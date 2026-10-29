//! One run: scan both sides, decide each file, copy, update the index, write the report.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{Local, Utc};

use crate::copy::copy_file;
use crate::decision::{decide, Action, IndexEntry, Mode, Snapshot};
use crate::error::{Error, Result};
use crate::index::{Index, META_SOURCE};
use crate::paths::{self, REPLACED_DIR, REPORTS_DIR, STATE_DIR};
use crate::report::{ConflictItem, ErrorItem, FileItem, Report};
use crate::scan::{scan, ScanResult};

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub mode: Mode,
    /// Decide and report, but write nothing.
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Event<'a> {
    Scanning,
    Planned {
        files: u64,
        bytes: u64,
    },
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
    let source_id = comparable(source)?;
    let destination_id = comparable(destination)?;
    if source_id.starts_with(&destination_id) || destination_id.starts_with(&source_id) {
        return Err(Error::NestedFolders(
            source.to_path_buf(),
            destination.to_path_buf(),
        ));
    }
    prepare_destination(destination, options.dry_run)?;

    let index = if options.dry_run {
        Index::open_existing(destination)?
    } else {
        Some(Index::open(destination)?)
    };
    let source_id = source_id.to_string_lossy().into_owned();
    if let Some(index) = &index {
        match index.meta(META_SOURCE)? {
            Some(stored) if stored != source_id => {
                return Err(Error::IndexBelongsToOtherSource(stored))
            }
            Some(_) => {}
            None if !options.dry_run => index.set_meta(META_SOURCE, &source_id)?,
            None => {}
        }
    }
    let writable = if options.dry_run {
        None
    } else {
        index.as_ref()
    };

    let mut report = Report::new(source, destination, options.mode, options.dry_run, started);

    on_event(Event::Scanning);
    let source_scan = scan(source);
    let destination_scan = if destination.is_dir() {
        scan(destination)
    } else {
        ScanResult::default()
    };
    for err in source_scan.errors.iter().chain(&destination_scan.errors) {
        report.errors.push(ErrorItem::new(&err.path, &err.message));
    }
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
    let stored = match &index {
        Some(index) => index.load_all()?,
        None => Default::default(),
    };

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
    on_event(Event::Planned {
        files: total_files,
        bytes: total_bytes,
    });

    let replaced_root = destination.join(STATE_DIR).join(REPLACED_DIR).join(&stamp);
    let (mut done_files, mut done_bytes) = (0, 0);

    for step in &plan {
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
                    match copy_file(
                        &paths::join_relative(source, &step.path),
                        &target,
                        src.mtime_ms,
                        replaced.as_deref(),
                    ) {
                        Ok(copied) => {
                            let entry = IndexEntry {
                                source: src,
                                destination: copied,
                            };
                            index.upsert(&step.key, &step.path, &entry, now_ms())?;
                            report.bytes_copied += src.size;
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
        if let Err(err) = index.save_previous_copy(destination) {
            report.errors.push(ErrorItem::new(
                &Index::path(destination).display().to_string(),
                &format!("cópia de segurança do índice: {err}"),
            ));
        }
    }
    report.finished_at = Local::now().to_rfc3339();
    if writable.is_some() {
        report.save(&destination.join(STATE_DIR).join(REPORTS_DIR), &stamp)?;
    }
    Ok(report)
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

/// Absolute path, folded like index keys, to compare folders.
fn comparable(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    Ok(PathBuf::from(paths::fold_key(&absolute.to_string_lossy())))
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
