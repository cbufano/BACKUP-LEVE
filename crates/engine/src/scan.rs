//! Reads a folder tree: relative path, size and modification time of each file.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use crate::decision::Snapshot;
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

#[derive(Debug, Default)]
pub struct ScanResult {
    /// Keyed by [`paths::fold_key`].
    pub files: BTreeMap<String, ScannedFile>,
    /// Leftovers from an interrupted copy.
    pub partials: Vec<PathBuf>,
    pub errors: Vec<ScanError>,
}

/// Scans `root`. Symbolic links and junctions are not followed.
///
/// A file that cannot be read goes to `errors`; the scan continues.
pub fn scan(root: &Path) -> ScanResult {
    let mut result = ScanResult::default();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_excluded(entry.file_name()));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                result.errors.push(ScanError {
                    path: err
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    message: err.to_string(),
                });
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .file_name()
            .to_string_lossy()
            .ends_with(PARTIAL_SUFFIX)
        {
            result.partials.push(entry.path().to_path_buf());
            continue;
        }
        let Some(relative) = paths::to_relative(root, entry.path()) else {
            result.errors.push(ScanError {
                path: entry.path().display().to_string(),
                message: "nome de arquivo com caracteres inválidos".into(),
            });
            continue;
        };
        match entry.metadata() {
            Ok(metadata) => {
                let snapshot = Snapshot::new(metadata.len(), mtime_ms(&metadata));
                result.files.insert(
                    paths::fold_key(&relative),
                    ScannedFile {
                        path: relative,
                        snapshot,
                    },
                );
            }
            Err(err) => result.errors.push(ScanError {
                path: relative,
                message: err.to_string(),
            }),
        }
    }
    result
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
    use std::fs;

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
    }
}
