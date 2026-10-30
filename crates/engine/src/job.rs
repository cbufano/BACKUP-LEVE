//! One run: scan both sides, decide each file, copy, update the index, write the report.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Local, Utc};

use crate::copy::copy_file_with_progress;
use crate::decision::{decide, Action, IndexEntry, Mode, Snapshot};
use crate::error::{Error, Result};
use crate::exclude::Exclusions;
use crate::index::{Index, META_SOURCE};
use crate::paths::{self, REPLACED_DIR, REPORTS_DIR, STATE_DIR};
use crate::report::{ConflictItem, ErrorItem, FileItem, LeftOutItem, Report};
use crate::scan::{scan_with_progress, Filter, ScanResult};

/// Longest time a copied file waits to enter the index.
const COMMIT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub mode: Mode,
    /// Decide and report, but write nothing.
    pub dry_run: bool,
    /// When set to `true`, the run stops after the current file and saves the report.
    pub cancel: Option<Arc<AtomicBool>>,
    /// What stays out of the backup. The default leaves out nothing.
    pub exclusions: Exclusions,
}

/// Folder being read before the copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Source,
    Destination,
}

#[derive(Debug, Clone, Copy)]
pub enum Event<'a> {
    /// The scan of one side starts. The source comes first.
    Scanning {
        side: Side,
        /// Files the index expects on this side. Zero when unknown.
        expected: u64,
    },
    /// One more file was found on `side`.
    ScanProgress {
        side: Side,
        found: u64,
        path: &'a str,
    },
    Planned {
        files: u64,
        bytes: u64,
        /// Files already in the backup and unchanged: not copied again.
        already: u64,
    },
    /// Part of a large file was copied. `done_bytes` counts that part too.
    FileProgress { path: &'a str, done_bytes: u64 },
    FileDone {
        path: &'a str,
        done_files: u64,
        done_bytes: u64,
    },
}

struct Step {
    key: String,
    path: String,
    action: Action,
    source: Option<Snapshot>,
    destination: Option<Snapshot>,
    /// Name in the destination, with the case found there.
    destination_path: Option<String>,
}

/// Synchronizes `source` into `destination`. See `docs/REGRAS-DE-SINCRONISMO.md`.
///
/// Problems with single files go to the report. Only problems that make the whole
/// run unsafe return an error.
pub fn run(
    source: &Path,
    destination: &Path,
    options: &Options,
    mut on_event: impl FnMut(Event<'_>),
) -> Result<Report> {
    let started = Local::now();
    let stamp = started.format("%Y-%m-%d_%H-%M-%S").to_string();

    if !source.is_dir() {
        return Err(Error::SourceNotFound(source.to_path_buf()));
    }
    let source_id = check_not_nested(source, destination)?;
    prepare_destination(destination, options.dry_run)?;

    let index = if options.dry_run {
        Index::open_existing(destination)?
    } else {
        Some(Index::open(destination)?)
    };
    if let Some(index) = &index {
        check_index_source(index, &source_id, !options.dry_run)?;
    }
    let writable = if options.dry_run {
        None
    } else {
        index.as_ref()
    };

    let mut report = Report::new(source, destination, options.mode, options.dry_run, started);

    let exclusions = &options.exclusions;
    let mut scan_side = |side: Side, root: &Path, filter: Filter<'_>, expected: u64| {
        on_event(Event::Scanning { side, expected });
        scan_with_progress(root, filter, &mut |found, path| {
            on_event(Event::ScanProgress { side, found, path });
            !is_cancelled(options)
        })
    };
    let source_filter = Filter {
        exclusions,
        folders: None,
    };
    let mut source_scan = scan_side(Side::Source, source, source_filter, 0);

    // The source decides which folders stay out. The destination does not read them,
    // and their records leave the index: the copies already in the destination stay
    // there, outside the program.
    let mut left_out_folders: HashSet<String> = source_scan
        .left_out
        .iter()
        .map(|folder| paths::fold_key(&folder.path))
        .collect();
    let mut stored = match &index {
        Some(index) if !source_scan.stopped => index.load_all()?,
        _ => Default::default(),
    };
    let mut forget_left_out = Vec::new();
    if !exclusions.is_empty() {
        stored.retain(|key, _| {
            let out = exclusions.covers(key, &left_out_folders);
            if out {
                forget_left_out.push(key.clone());
            }
            !out
        });
    }

    let destination_scan = if source_scan.stopped {
        ScanResult::default()
    } else if destination.is_dir() {
        let filter = Filter {
            exclusions,
            folders: Some(&left_out_folders),
        };
        scan_side(Side::Destination, destination, filter, stored.len() as u64)
    } else {
        on_event(Event::Scanning {
            side: Side::Destination,
            expected: 0,
        });
        ScanResult::default()
    };
    // An incomplete scan would make files look removed. Stop before any decision.
    if source_scan.stopped || destination_scan.stopped || is_cancelled(options) {
        report.cancelled = true;
        return finish(report, writable, destination, &stamp);
    }
    for err in source_scan.errors.iter().chain(&destination_scan.errors) {
        report.errors.push(ErrorItem::new(&err.path, &err.message));
    }
    // A folder that only the destination rules leave out stays out on both sides.
    if !destination_scan.left_out.is_empty() {
        let only_destination: HashSet<String> = destination_scan
            .left_out
            .iter()
            .map(|folder| paths::fold_key(&folder.path))
            .collect();
        source_scan
            .files
            .retain(|key, _| !exclusions.covers(key, &only_destination));
        stored.retain(|key, _| {
            let out = exclusions.covers(key, &only_destination);
            if out {
                forget_left_out.push(key.clone());
            }
            !out
        });
        left_out_folders.extend(only_destination);
    }
    report.left_out = source_scan
        .left_out
        .iter()
        .chain(&destination_scan.left_out)
        .map(|folder| LeftOutItem {
            path: folder.path.clone(),
            reason: folder.reason.to_owned(),
        })
        .collect();
    report.left_out_forgotten = forget_left_out.len() as u64;
    if writable.is_some() {
        for partial in &destination_scan.partials {
            match fs::remove_file(partial) {
                Ok(()) => report.partials_removed += 1,
                Err(err) => report.errors.push(ErrorItem::new(
                    &partial.display().to_string(),
                    &err.to_string(),
                )),
            }
        }
    }
    let keys: BTreeSet<&String> = source_scan
        .files
        .keys()
        .chain(destination_scan.files.keys())
        .chain(stored.keys())
        .collect();
    let mut plan = Vec::with_capacity(keys.len());
    for key in keys {
        let src = source_scan.files.get(key);
        let dst = destination_scan.files.get(key);
        let entry = stored.get(key);
        let action = decide(
            options.mode,
            src.map(|f| f.snapshot),
            dst.map(|f| f.snapshot),
            entry.map(|e| &e.entry),
        );
        let path = src
            .map(|f| f.path.clone())
            .or_else(|| dst.map(|f| f.path.clone()))
            .or_else(|| entry.map(|e| e.path.clone()))
            .unwrap_or_else(|| key.clone());
        plan.push(Step {
            key: key.clone(),
            path,
            action,
            source: src.map(|f| f.snapshot),
            destination: dst.map(|f| f.snapshot),
            destination_path: dst.map(|f| f.path.clone()),
        });
    }

    let (total_files, total_bytes) = plan
        .iter()
        .filter(|step| matches!(step.action, Action::CopyNew | Action::Update))
        .fold((0, 0), |(files, bytes), step| {
            (files + 1, bytes + step.source.map_or(0, |s| s.size))
        });
    let already = plan
        .iter()
        .filter(|step| step.action == Action::Skip && step.source.is_some())
        .count() as u64;
    on_event(Event::Planned {
        files: total_files,
        bytes: total_bytes,
        already,
    });

    let replaced_root = destination.join(STATE_DIR).join(REPLACED_DIR).join(&stamp);
    let (mut done_files, mut done_bytes) = (0, 0);

    // Files enter the index in groups. If the program stops abruptly, the files of
    // the last group are in the destination but not in the index; the next run finds
    // them identical and records them without copying ("Adopt").
    if let Some(index) = writable {
        index.begin()?;
        for key in &forget_left_out {
            index.remove(key)?;
        }
    }
    let mut last_commit = Instant::now();

    for step in &plan {
        if let Some(index) = writable {
            if last_commit.elapsed() >= COMMIT_INTERVAL {
                index.commit()?;
                index.begin()?;
                last_commit = Instant::now();
            }
        }
        if is_cancelled(options) {
            report.cancelled = true;
            break;
        }
        match step.action {
            Action::Skip => {
                if step.source.is_some() {
                    report.unchanged += 1;
                }
            }
            Action::CopyNew | Action::Update => {
                let src = step.source.expect("copy actions always have a source");
                let item = FileItem::new(&step.path, src.size);
                let is_update = step.action == Action::Update;
                if let Some(index) = writable {
                    let target_relative = step.destination_path.as_deref().unwrap_or(&step.path);
                    let target = paths::join_relative(destination, target_relative);
                    let replaced =
                        is_update.then(|| paths::join_relative(&replaced_root, target_relative));
                    let bytes_before = done_bytes;
                    let copy = copy_file_with_progress(
                        &paths::join_relative(source, &step.path),
                        &target,
                        src.mtime_ms,
                        replaced.as_deref(),
                        &mut |copied| {
                            on_event(Event::FileProgress {
                                path: &step.path,
                                done_bytes: bytes_before + copied,
                            });
                            !is_cancelled(options)
                        },
                    );
                    match copy {
                        Ok(copied) => {
                            let entry = IndexEntry {
                                source: src,
                                destination: copied,
                            };
                            index.upsert(&step.key, &step.path, &entry, now_ms())?;
                            report.bytes_copied += src.size;
                        }
                        Err(_) if is_cancelled(options) => {
                            report.cancelled = true;
                            break;
                        }
                        Err(err) => {
                            report
                                .errors
                                .push(ErrorItem::new(&step.path, &err.to_string()));
                            continue;
                        }
                    }
                    done_files += 1;
                    done_bytes += src.size;
                    on_event(Event::FileDone {
                        path: &step.path,
                        done_files,
                        done_bytes,
                    });
                }
                if is_update {
                    report.updated.push(item);
                } else {
                    report.copied.push(item);
                }
            }
            Action::Adopt => {
                if let (Some(index), Some(src), Some(dst)) =
                    (writable, step.source, step.destination)
                {
                    let entry = IndexEntry {
                        source: src,
                        destination: dst,
                    };
                    index.upsert(&step.key, &step.path, &entry, now_ms())?;
                }
                report
                    .adopted
                    .push(FileItem::new(&step.path, step.source.map_or(0, |s| s.size)));
            }
            Action::ReportChangedInSource => report
                .changed_in_source
                .push(FileItem::new(&step.path, step.source.map_or(0, |s| s.size))),
            Action::Conflict(kind) => report.conflicts.push(ConflictItem {
                path: step.path.clone(),
                kind,
                source: step.source,
                destination: step.destination,
            }),
            Action::KeepRemovedFromSource => report.removed_from_source.push(FileItem::new(
                &step.path,
                step.destination.map_or(0, |s| s.size),
            )),
            Action::ForgetEntry => {
                if let Some(index) = writable {
                    index.remove(&step.key)?;
                }
                report.forgotten += 1;
            }
        }
    }
    if let Some(index) = writable {
        index.commit()?;
    }

    finish(report, writable, destination, &stamp)
}

/// Saves a copy of the index and the report.
fn finish(
    mut report: Report,
    writable: Option<&Index>,
    destination: &Path,
    stamp: &str,
) -> Result<Report> {
    if let Some(index) = writable {
        if let Err(err) = index.save_previous_copy(destination) {
            report.errors.push(ErrorItem::new(
                &Index::path(destination).display().to_string(),
                &format!("cópia de segurança do índice: {err}"),
            ));
        }
    }
    report.finished_at = Local::now().to_rfc3339();
    if writable.is_some() {
        report.save(&destination.join(STATE_DIR).join(REPORTS_DIR), stamp)?;
    }
    Ok(report)
}

fn is_cancelled(options: &Options) -> bool {
    options
        .cancel
        .as_ref()
        .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
}

/// The destination drive must be connected. A missing folder on it is created.
fn prepare_destination(destination: &Path, dry_run: bool) -> Result<()> {
    if destination.is_dir() {
        return Ok(());
    }
    let reachable = destination
        .ancestors()
        .skip(1)
        .any(|ancestor| ancestor.as_os_str().is_empty() || ancestor.is_dir());
    if !reachable {
        return Err(Error::DestinationUnavailable(destination.to_path_buf()));
    }
    if !dry_run {
        fs::create_dir_all(destination)?;
    }
    Ok(())
}

/// Refuses a source and a destination inside each other. Returns the source id.
pub(crate) fn check_not_nested(source: &Path, destination: &Path) -> Result<String> {
    let source_id = comparable(source)?;
    let destination_id = comparable(destination)?;
    if source_id.starts_with(&destination_id) || destination_id.starts_with(&source_id) {
        return Err(Error::NestedFolders(
            source.to_path_buf(),
            destination.to_path_buf(),
        ));
    }
    Ok(source_id.to_string_lossy().into_owned())
}

/// Refuses an index that belongs to another source. Records the source when missing.
pub(crate) fn check_index_source(index: &Index, source_id: &str, record: bool) -> Result<()> {
    match index.meta(META_SOURCE)? {
        Some(stored) if stored != source_id => Err(Error::IndexBelongsToOtherSource(stored)),
        Some(_) => Ok(()),
        None if record => index.set_meta(META_SOURCE, source_id),
        None => Ok(()),
    }
}

/// Absolute path, folded like index keys, to compare folders.
fn comparable(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    Ok(PathBuf::from(paths::fold_key(&absolute.to_string_lossy())))
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
