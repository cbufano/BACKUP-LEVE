//! A short summary of a source folder: the largest folder names and file endings.
//!
//! The window sends this summary to an AI service, which suggests names to skip.
//! The summary has names and sizes only. It never has the content of a file.
//! What is already left out by the profile is not in the summary.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::exclude::Exclusions;

/// Entries read at most, so that a very large disk does not stop the window.
pub const MAX_ENTRIES: usize = 300_000;
/// Lines kept in each list of the summary.
pub const TOP: usize = 60;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Item {
    /// Folder name or file ending (`.log`), in lowercase.
    #[serde(rename = "nome")]
    pub name: String,
    /// Folders with this name, or files with this ending.
    #[serde(rename = "quantidade")]
    pub count: u64,
    /// Files inside these folders, or zero for an ending.
    #[serde(rename = "arquivos")]
    pub files: u64,
    #[serde(rename = "bytes")]
    pub bytes: u64,
    /// Names of the files next to one of these folders (for example `package.json`).
    #[serde(rename = "ao_lado", skip_serializing_if = "Vec::is_empty")]
    pub siblings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Summary {
    #[serde(rename = "pastas")]
    pub folders: Vec<Item>,
    #[serde(rename = "terminacoes")]
    pub endings: Vec<Item>,
    #[serde(rename = "arquivos_lidos")]
    pub files_read: u64,
    /// True when [`MAX_ENTRIES`] stopped the reading.
    #[serde(rename = "incompleto")]
    pub truncated: bool,
}

/// Reads `root` and returns its summary. Unreadable folders are ignored.
pub fn summarize(root: &Path, exclusions: &Exclusions) -> Summary {
    let mut folders: HashMap<String, Item> = HashMap::new();
    let mut endings: HashMap<String, Item> = HashMap::new();
    let mut summary = Summary::default();
    let mut entries = 0usize;
    walk(
        root,
        exclusions,
        &mut Vec::new(),
        &mut folders,
        &mut endings,
        &mut summary,
        &mut entries,
    );
    summary.folders = top(folders);
    summary.endings = top(endings);
    summary
}

/// Walks `dir`. `open` holds the names of the folders above, to add the sizes to them.
fn walk(
    dir: &Path,
    exclusions: &Exclusions,
    open: &mut Vec<String>,
    folders: &mut HashMap<String, Item>,
    endings: &mut HashMap<String, Item>,
    summary: &mut Summary,
    entries: &mut usize,
) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    let children: Vec<_> = read.filter_map(|entry| entry.ok()).collect();
    let names: HashSet<String> = children
        .iter()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
        .collect();
    for entry in children {
        *entries += 1;
        if *entries > MAX_ENTRIES {
            summary.truncated = true;
            return;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if exclusions.skips_name(&name) {
            continue;
        }
        if kind.is_dir() {
            if exclusions.rebuildable_rule(&name, &names).is_some() {
                continue;
            }
            let item = folders.entry(name.clone()).or_default();
            item.count += 1;
            if item.siblings.is_empty() {
                let mut next: Vec<String> = names.iter().take(8).cloned().collect();
                next.sort();
                item.siblings = next;
            }
            open.push(name);
            walk(
                &entry.path(),
                exclusions,
                open,
                folders,
                endings,
                summary,
                entries,
            );
            open.pop();
        } else if kind.is_file() {
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            summary.files_read += 1;
            let unique: HashSet<&String> = open.iter().collect();
            for folder in unique {
                if let Some(item) = folders.get_mut(folder) {
                    item.files += 1;
                    item.bytes += size;
                }
            }
            if let Some(dot) = name.rfind('.').filter(|&dot| dot > 0) {
                let item = endings.entry(name[dot..].to_owned()).or_default();
                item.count += 1;
                item.bytes += size;
            }
        }
    }
}

/// The [`TOP`] items with the most bytes, then the most files.
fn top(map: HashMap<String, Item>) -> Vec<Item> {
    let mut items: Vec<Item> = map
        .into_iter()
        .map(|(name, item)| Item { name, ..item })
        .collect();
    items.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then(b.files.cmp(&a.files))
            .then(a.name.cmp(&b.name))
    });
    items.truncate(TOP);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_has_folders_and_endings_but_not_what_is_left_out() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("site/node_modules/react")).unwrap();
        fs::write(root.join("site/package.json"), "{}").unwrap();
        fs::write(
            root.join("site/node_modules/react/index.js"),
            "x".repeat(500),
        )
        .unwrap();
        fs::create_dir_all(root.join("jogo/Library")).unwrap();
        fs::write(root.join("jogo/Library/a.asset"), "x".repeat(300)).unwrap();
        fs::write(root.join("jogo/rascunho.tmp"), "x").unwrap();

        let rules = Exclusions::new(true, &[]).with_temporary(true);
        let summary = summarize(root, &rules);

        let names: Vec<&str> = summary.folders.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"library"));
        assert!(!names.contains(&"node_modules"));
        let library = summary
            .folders
            .iter()
            .find(|i| i.name == "library")
            .unwrap();
        assert_eq!((library.files, library.bytes), (1, 300));
        let endings: Vec<&str> = summary.endings.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(endings, vec![".asset", ".json"]);
        assert_eq!(summary.files_read, 2);
        assert!(!summary.truncated);
    }
}
