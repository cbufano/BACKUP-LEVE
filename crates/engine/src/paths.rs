//! Relative paths as stored in the index, and the names the engine reserves.

use std::path::{Component, Path, PathBuf};

/// Folder in the destination that holds the index, reports and replaced versions.
pub const STATE_DIR: &str = ".backup-leve";
/// Suffix of a copy in progress. Only a complete copy gets the final name.
pub const PARTIAL_SUFFIX: &str = ".backup-leve-parcial";
pub const REPLACED_DIR: &str = "substituidos";
pub const REPORTS_DIR: &str = "relatorios";

/// Path of `path` relative to `root`, with `/` separators.
///
/// Returns `None` when `path` is not under `root` or its name is not valid UTF-8.
pub fn to_relative(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Inverse of [`to_relative`].
pub fn join_relative(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
    }
    path
}

/// Key that identifies a file. Windows and macOS ignore case in file names.
pub fn fold_key(relative: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        relative.to_lowercase()
    } else {
        relative.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_round_trip() {
        let root = Path::new("raiz");
        let path = root.join("pasta").join("arquivo.txt");
        let relative = to_relative(root, &path).unwrap();
        assert_eq!(relative, "pasta/arquivo.txt");
        assert_eq!(join_relative(root, &relative), path);
    }

    #[test]
    fn root_itself_has_no_relative_path() {
        assert_eq!(to_relative(Path::new("raiz"), Path::new("raiz")), None);
    }

    #[test]
    fn path_outside_root_has_no_relative_path() {
        assert_eq!(
            to_relative(Path::new("raiz"), Path::new("outra/a.txt")),
            None
        );
    }
}
