//! Reads a folder tree: relative path, size and modification time of each file.

use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::fs::{self, FileType, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Condvar, Mutex};
use std::thread;
use std::time::UNIX_EPOCH;

use crate::decision::Snapshot;
use crate::exclude::{Exclusions, CACHE_TAG, CACHE_TAG_SIGNATURE};
use crate::paths::{self, PARTIAL_SUFFIX, STATE_DIR};

/// Names never copied, compared without case.
const EXCLUDED_NAMES: &[&str] = &[
    STATE_DIR,
    "thumbs.db",
    "desktop.ini",
    ".ds_store",
    "$recycle.bin",
    "system volume information",
];

/// Folders read at the same time. On a network drive, each folder costs a round
/// trip to the server; reading several at once hides most of that wait.
const SCAN_THREADS: usize = 8;

#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Relative path with `/` separators, with the case found on disk.
    pub path: String,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone)]
pub struct ScanError {
    pub path: String,
    pub message: String,
}

impl ScanError {
    fn new(path: &Path, err: std::io::Error) -> Self {
        Self {
            path: path.display().to_string(),
            message: err.to_string(),
        }
    }
}

/// A folder left out by a rebuildable rule.
#[derive(Debug, Clone)]
pub struct LeftOut {
    /// Relative path with `/` separators, with the case found on disk.
    pub path: String,
    /// Who creates the folder again, in Portuguese.
    pub reason: &'static str,
}

/// What a scan leaves out.
#[derive(Debug, Clone, Copy)]
pub struct Filter<'a> {
    pub exclusions: &'a Exclusions,
    /// Folders to leave out, as keys from [`paths::fold_key`]. The destination
    /// uses the folders that the source left out.
    pub folders: Option<&'a HashSet<String>>,
}

static NO_EXCLUSIONS: Exclusions = Exclusions::none();

impl Filter<'_> {
    /// Leaves out only the names the program never copies.
    pub fn none() -> Filter<'static> {
        Filter {
            exclusions: &NO_EXCLUSIONS,
            folders: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct ScanResult {
    /// Keyed by [`paths::fold_key`].
    pub files: BTreeMap<String, ScannedFile>,
    /// Leftovers from an interrupted copy.
    pub partials: Vec<PathBuf>,
    pub errors: Vec<ScanError>,
    /// Folders left out by a rule. Folders in [`Filter::folders`] are not here.
    pub left_out: Vec<LeftOut>,
    /// The caller stopped the scan. `files` is then incomplete.
    pub stopped: bool,
}

/// Scans `root`. Symbolic links and junctions are not followed.
///
/// A file that cannot be read goes to `errors`; the scan continues.
pub fn scan(root: &Path) -> ScanResult {
    scan_with_progress(root, Filter::none(), &mut |_, _| true)
}

/// Like [`scan`], and calls `on_file(found, path)` after each file.
/// When `on_file` returns `false`, the scan stops and sets `stopped`.
///
/// Worker threads read the folders. `on_file` always runs on the calling thread.
pub fn scan_with_progress(
    root: &Path,
    filter: Filter<'_>,
    on_file: &mut dyn FnMut(u64, &str) -> bool,
) -> ScanResult {
    let mut result = ScanResult::default();
    let queue = FolderQueue::new(root.to_path_buf());
    let (sender, receiver) = mpsc::channel::<Vec<Found>>();

    thread::scope(|scope| {
        for _ in 0..SCAN_THREADS {
            let sender = sender.clone();
            let queue = &queue;
            scope.spawn(move || {
                while let Some(folder) = queue.next() {
                    let found = read_folder(root, &folder, filter, queue);
                    queue.finished();
                    if sender.send(found).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);

        // Ends when every worker has stopped.
        for batch in receiver {
            if result.stopped {
                continue;
            }
            for item in batch {
                match item {
                    Found::File(relative, snapshot) => {
                        let keep_going = on_file(result.files.len() as u64 + 1, &relative);
                        result.files.insert(
                            paths::fold_key(&relative),
                            ScannedFile {
                                path: relative,
                                snapshot,
                            },
                        );
                        if !keep_going {
                            result.stopped = true;
                            queue.stop();
                            break;
                        }
                    }
                    Found::Partial(path) => result.partials.push(path),
                    Found::Error(err) => result.errors.push(err),
                    Found::LeftOut(left_out) => result.left_out.push(left_out),
                }
            }
        }
    });
    result
}

enum Found {
    File(String, Snapshot),
    Partial(PathBuf),
    Error(ScanError),
    LeftOut(LeftOut),
}

/// Lists one folder. Its subfolders go back to the queue.
fn read_folder(root: &Path, folder: &Path, filter: Filter<'_>, queue: &FolderQueue) -> Vec<Found> {
    let listing = match fs::read_dir(folder) {
        Ok(listing) => listing,
        Err(err) => return vec![Found::Error(ScanError::new(folder, err))],
    };
    let mut found = Vec::new();
    let mut entries = Vec::new();
    for entry in listing {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                found.push(Found::Error(ScanError::new(folder, err)));
                continue;
            }
        };
        let name = entry.file_name();
        if is_excluded(&name) {
            continue;
        }
        // A link or a junction is neither a folder nor a file here: it is not followed.
        match entry.file_type() {
            Ok(file_type) => {
                let lower = name.to_string_lossy().to_lowercase();
                entries.push((entry, lower, file_type));
            }
            Err(err) => found.push(Found::Error(ScanError::new(&entry.path(), err))),
        }
    }

    let exclusions = filter.exclusions;
    if folder != root && is_tagged_cache(folder, &entries, exclusions) {
        if let Some(relative) = paths::to_relative(root, folder) {
            found.push(Found::LeftOut(LeftOut {
                path: relative,
                reason: "pasta marcada como cache (CACHEDIR.TAG)",
            }));
            return found;
        }
    }

    // Names of the files here, read only when a rebuildable rule needs them.
    let mut siblings: Option<HashSet<String>> = None;
    let mut subfolders = Vec::new();
    for (entry, lower, file_type) in &entries {
        if exclusions.skips_name(lower) {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            let relative = paths::to_relative(root, &path);
            if let (Some(folders), Some(relative)) = (filter.folders, &relative) {
                if folders.contains(&paths::fold_key(relative)) {
                    continue;
                }
            }
            if exclusions.may_skip_folder(lower) {
                let siblings = siblings.get_or_insert_with(|| file_names(&entries));
                if let (Some(rule), Some(relative)) =
                    (exclusions.rebuildable_rule(lower, siblings), relative)
                {
                    found.push(Found::LeftOut(LeftOut {
                        path: relative,
                        reason: rule.tool,
                    }));
                    continue;
                }
            }
            subfolders.push(path);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if lower.ends_with(PARTIAL_SUFFIX) {
            found.push(Found::Partial(path));
            continue;
        }
        let Some(relative) = paths::to_relative(root, &path) else {
            found.push(Found::Error(ScanError {
                path: path.display().to_string(),
                message: "nome de arquivo com caracteres inválidos".into(),
            }));
            continue;
        };
        // On Windows the size and the date come with the folder listing.
        match entry.metadata() {
            Ok(metadata) => found.push(Found::File(
                relative,
                Snapshot::new(metadata.len(), mtime_ms(&metadata)),
            )),
            Err(err) => found.push(Found::Error(ScanError {
                path: relative,
                message: err.to_string(),
            })),
        }
    }
    queue.add(subfolders);
    found
}

type Entry = (fs::DirEntry, String, FileType);

/// Names, in lowercase, of the files in a listing.
fn file_names(entries: &[Entry]) -> HashSet<String> {
    entries
        .iter()
        .filter(|(_, _, file_type)| file_type.is_file())
        .map(|(_, name, _)| name.clone())
        .collect()
}

/// Whether `folder` holds a valid cache tag and the rules leave such a folder out.
fn is_tagged_cache(folder: &Path, entries: &[Entry], exclusions: &Exclusions) -> bool {
    let tag = CACHE_TAG.to_lowercase();
    let tagged = entries
        .iter()
        .any(|(_, name, file_type)| file_type.is_file() && *name == tag);
    if !tagged {
        return false;
    }
    let name = folder
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !exclusions.skips_tagged_cache(&name) {
        return false;
    }
    let mut start = [0; CACHE_TAG_SIGNATURE.len()];
    fs::File::open(folder.join(CACHE_TAG))
        .and_then(|mut file| file.read_exact(&mut start))
        .is_ok_and(|()| start == CACHE_TAG_SIGNATURE)
}

/// Folders waiting to be read, shared by the worker threads.
struct FolderQueue {
    state: Mutex<QueueState>,
    changed: Condvar,
}

struct QueueState {
    waiting: Vec<PathBuf>,
    /// Folders being read now. Their subfolders can still arrive.
    reading: usize,
    stopped: bool,
}

impl FolderQueue {
    fn new(root: PathBuf) -> Self {
        Self {
            state: Mutex::new(QueueState {
                waiting: vec![root],
                reading: 0,
                stopped: false,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// The next folder, or `None` when every folder was read or the scan stopped.
    fn next(&self) -> Option<PathBuf> {
        let mut state = self.lock();
        loop {
            if state.stopped {
                return None;
            }
            if let Some(folder) = state.waiting.pop() {
                state.reading += 1;
                return Some(folder);
            }
            if state.reading == 0 {
                return None;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|err| err.into_inner());
        }
    }

    fn add(&self, folders: Vec<PathBuf>) {
        if folders.is_empty() {
            return;
        }
        self.lock().waiting.extend(folders);
        self.changed.notify_all();
    }

    fn finished(&self) {
        let mut state = self.lock();
        state.reading -= 1;
        if state.reading == 0 && state.waiting.is_empty() {
            self.changed.notify_all();
        }
    }

    fn stop(&self) {
        self.lock().stopped = true;
        self.changed.notify_all();
    }
}

/// Modification time in milliseconds since the Unix epoch.
pub fn mtime_ms(metadata: &Metadata) -> i64 {
    match metadata.modified() {
        Ok(time) => match time.duration_since(UNIX_EPOCH) {
            Ok(after) => after.as_millis() as i64,
            Err(before) => -(before.duration().as_millis() as i64),
        },
        Err(_) => 0,
    }
}

fn is_excluded(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_lowercase();
    EXCLUDED_NAMES.contains(&name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_files_and_skips_excluded_names() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("pasta")).unwrap();
        fs::create_dir_all(root.join(STATE_DIR)).unwrap();
        fs::write(root.join("a.txt"), "abc").unwrap();
        fs::write(root.join("pasta").join("b.txt"), "12345").unwrap();
        fs::write(root.join("Thumbs.db"), "x").unwrap();
        fs::write(root.join(STATE_DIR).join("indice.sqlite"), "x").unwrap();
        fs::write(root.join(format!("c.txt{PARTIAL_SUFFIX}")), "x").unwrap();

        let result = scan(root);

        let found: Vec<&str> = result.files.values().map(|f| f.path.as_str()).collect();
        assert_eq!(found, ["a.txt", "pasta/b.txt"]);
        assert_eq!(
            result.files[&paths::fold_key("pasta/b.txt")].snapshot.size,
            5
        );
        assert_eq!(result.partials.len(), 1);
        assert!(result.errors.is_empty());
        assert!(!result.stopped);
    }

    #[test]
    fn reads_every_folder_of_a_deep_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let mut expected = 0;
        for a in 0..6 {
            for b in 0..5 {
                let folder = tmp.path().join(format!("a{a}")).join(format!("b{b}"));
                fs::create_dir_all(folder.join("vazia")).unwrap();
                for c in 0..4 {
                    fs::write(folder.join(format!("{c}.txt")), "x").unwrap();
                    expected += 1;
                }
            }
        }

        for _ in 0..5 {
            let result = scan(tmp.path());
            assert_eq!(result.files.len(), expected);
            assert!(result.errors.is_empty());
        }
    }

    #[test]
    fn reports_each_file_and_can_be_stopped() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["a.txt", "b.txt", "c.txt"] {
            fs::write(tmp.path().join(name), "x").unwrap();
        }

        let mut seen = Vec::new();
        let result = scan_with_progress(tmp.path(), Filter::none(), &mut |found, path| {
            seen.push((found, path.to_owned()));
            found < 2
        });

        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, 1);
        assert_eq!(seen[1].0, 2);
        assert_eq!(result.files.len(), 2);
        assert!(result.stopped);
    }

    fn scan_leaving_out(root: &Path, exclusions: &Exclusions) -> ScanResult {
        let filter = Filter {
            exclusions,
            folders: None,
        };
        scan_with_progress(root, filter, &mut |_, _| true)
    }

    fn found(result: &ScanResult) -> Vec<&str> {
        result.files.values().map(|f| f.path.as_str()).collect()
    }

    #[test]
    fn leaves_out_rebuildable_folders_and_says_which() {
        let tmp = tempfile::tempdir().unwrap();
        let site = tmp.path().join("site");
        fs::create_dir_all(site.join("node_modules").join("react")).unwrap();
        fs::write(site.join("package.json"), "{}").unwrap();
        fs::write(
            site.join("node_modules").join("react").join("index.js"),
            "x",
        )
        .unwrap();
        // Without a package.json next to it, the folder is the user's.
        fs::create_dir_all(tmp.path().join("notas").join("node_modules")).unwrap();
        fs::write(
            tmp.path().join("notas").join("node_modules").join("a.txt"),
            "x",
        )
        .unwrap();

        let result = scan_leaving_out(tmp.path(), &Exclusions::new(true, &[]));

        assert_eq!(
            found(&result),
            ["notas/node_modules/a.txt", "site/package.json"]
        );
        assert_eq!(result.left_out.len(), 1);
        assert_eq!(result.left_out[0].path, "site/node_modules");
        assert_eq!(result.left_out[0].reason, "npm, yarn ou pnpm (JavaScript)");
    }

    #[test]
    fn leaves_out_a_folder_tagged_as_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("compilado");
        let fake = tmp.path().join("falso");
        let venv = tmp.path().join(".venv");
        for folder in [&cache, &fake, &venv] {
            fs::create_dir_all(folder).unwrap();
            fs::write(folder.join("dado.bin"), "x").unwrap();
        }
        let tag = [CACHE_TAG_SIGNATURE, b"\n# cache".as_slice()].concat();
        fs::write(cache.join(CACHE_TAG), &tag).unwrap();
        fs::write(venv.join(CACHE_TAG), &tag).unwrap();
        fs::write(fake.join(CACHE_TAG), "texto qualquer").unwrap();

        let result = scan_leaving_out(tmp.path(), &Exclusions::new(true, &[]));

        // A virtual environment without a package list stays in the backup.
        assert_eq!(
            found(&result),
            [
                ".venv/CACHEDIR.TAG",
                ".venv/dado.bin",
                "falso/CACHEDIR.TAG",
                "falso/dado.bin"
            ]
        );
        assert_eq!(result.left_out.len(), 1);
        assert_eq!(result.left_out[0].path, "compilado");
    }

    #[test]
    fn skips_the_folders_it_is_given() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("site").join("dist")).unwrap();
        fs::write(tmp.path().join("site").join("dist").join("a.js"), "x").unwrap();
        fs::write(tmp.path().join("site").join("b.js"), "x").unwrap();
        let folders: HashSet<String> = [paths::fold_key("site/dist")].into_iter().collect();
        let exclusions = Exclusions::none();

        let result = scan_with_progress(
            tmp.path(),
            Filter {
                exclusions: &exclusions,
                folders: Some(&folders),
            },
            &mut |_, _| true,
        );

        assert_eq!(found(&result), ["site/b.js"]);
        assert!(result.left_out.is_empty());
    }
}
