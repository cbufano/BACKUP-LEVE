//! Decision table for a single file.
//!
//! Pure logic, no I/O. Implements the tables in `docs/REGRAS-DE-SINCRONISMO.md`.

use serde::{Deserialize, Serialize};

/// FAT32 stores modification times with 2-second precision.
pub const MTIME_TOLERANCE_MS: i64 = 2_000;

/// Size and modification time of a file at one moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub mtime_ms: i64,
}

impl Snapshot {
    pub fn new(size: u64, mtime_ms: i64) -> Self {
        Self { size, mtime_ms }
    }

    /// Same size and modification times within [`MTIME_TOLERANCE_MS`].
    pub fn matches(&self, other: &Snapshot) -> bool {
        self.size == other.size && (self.mtime_ms - other.mtime_ms).abs() <= MTIME_TOLERANCE_MS
    }
}

/// What the index remembers from the last successful sync of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexEntry {
    pub source: Snapshot,
    pub destination: Snapshot,
}

/// Serialized with the Portuguese names the user sees in `perfis.toml`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Copy new files only. Files changed in the source are reported, not copied.
    #[serde(rename = "somente-novos")]
    AddOnly,
    /// Copy new files and update files changed in the source.
    #[default]
    #[serde(rename = "sincronizar")]
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// Both sides have the file, the index does not, and they differ.
    ExistsWithDifferentContent,
    ChangedInDestination,
    ChangedInBoth,
    DeletedInDestination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing to do.
    Skip,
    /// Copy a file that does not exist in the destination.
    CopyNew,
    /// Both sides already match: record them in the index without copying.
    Adopt,
    /// Replace the destination with the newer source version.
    Update,
    /// `AddOnly` mode: the source changed, report it and leave the destination as is.
    ReportChangedInSource,
    /// Do not touch the file. The user decides at the end of the run.
    Conflict(ConflictKind),
    /// The source file was removed: keep the backup copy and report it.
    KeepRemovedFromSource,
    /// The file is gone from both sides: drop the index entry.
    ForgetEntry,
}

/// Decides what to do with one relative path.
///
/// `None` means the file does not exist on that side (or has no index entry).
pub fn decide(
    mode: Mode,
    source: Option<Snapshot>,
    destination: Option<Snapshot>,
    index: Option<&IndexEntry>,
) -> Action {
    match (source, destination, index) {
        (None, None, None) => Action::Skip,
        (None, None, Some(_)) => Action::ForgetEntry,
        // Only in the destination and not ours.
        (None, Some(_), None) => Action::Skip,
        (None, Some(_), Some(_)) => Action::KeepRemovedFromSource,
        (Some(_), None, None) => Action::CopyNew,
        (Some(_), None, Some(_)) => Action::Conflict(ConflictKind::DeletedInDestination),
        (Some(src), Some(dst), None) => {
            if src.matches(&dst) {
                Action::Adopt
            } else {
                Action::Conflict(ConflictKind::ExistsWithDifferentContent)
            }
        }
        (Some(src), Some(dst), Some(entry)) => {
            let source_changed = !src.matches(&entry.source);
            let destination_changed = !dst.matches(&entry.destination);
            match (source_changed, destination_changed) {
                (false, false) => Action::Skip,
                (true, false) => match mode {
                    Mode::Sync => Action::Update,
                    Mode::AddOnly => Action::ReportChangedInSource,
                },
                (false, true) => Action::Conflict(ConflictKind::ChangedInDestination),
                (true, true) if src.matches(&dst) => Action::Adopt,
                (true, true) => Action::Conflict(ConflictKind::ChangedInBoth),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_760_000_000_000;

    fn snap(size: u64, mtime_ms: i64) -> Option<Snapshot> {
        Some(Snapshot::new(size, mtime_ms))
    }

    fn entry(size: u64, mtime_ms: i64) -> IndexEntry {
        IndexEntry {
            source: Snapshot::new(size, mtime_ms),
            destination: Snapshot::new(size, mtime_ms),
        }
    }

    #[test]
    fn new_file_is_copied() {
        assert_eq!(
            decide(Mode::Sync, snap(10, T0), None, None),
            Action::CopyNew
        );
    }

    #[test]
    fn identical_file_without_index_is_adopted() {
        assert_eq!(
            decide(Mode::Sync, snap(10, T0), snap(10, T0 + 1_500), None),
            Action::Adopt
        );
    }

    #[test]
    fn different_file_without_index_is_a_conflict() {
        assert_eq!(
            decide(Mode::Sync, snap(10, T0), snap(11, T0), None),
            Action::Conflict(ConflictKind::ExistsWithDifferentContent)
        );
    }

    #[test]
    fn unchanged_file_is_skipped() {
        let e = entry(10, T0);
        assert_eq!(
            decide(Mode::Sync, snap(10, T0), snap(10, T0), Some(&e)),
            Action::Skip
        );
    }

    #[test]
    fn source_change_updates_in_sync_mode() {
        let e = entry(10, T0);
        assert_eq!(
            decide(Mode::Sync, snap(12, T0 + 60_000), snap(10, T0), Some(&e)),
            Action::Update
        );
    }

    #[test]
    fn source_change_is_only_reported_in_add_only_mode() {
        let e = entry(10, T0);
        assert_eq!(
            decide(Mode::AddOnly, snap(12, T0 + 60_000), snap(10, T0), Some(&e)),
            Action::ReportChangedInSource
        );
    }

    #[test]
    fn destination_change_is_a_conflict() {
        let e = entry(10, T0);
        assert_eq!(
            decide(Mode::Sync, snap(10, T0), snap(10, T0 + 60_000), Some(&e)),
            Action::Conflict(ConflictKind::ChangedInDestination)
        );
    }

    #[test]
    fn change_on_both_sides_is_a_conflict() {
        let e = entry(10, T0);
        assert_eq!(
            decide(
                Mode::Sync,
                snap(11, T0 + 1_000_000),
                snap(12, T0 + 60_000),
                Some(&e)
            ),
            Action::Conflict(ConflictKind::ChangedInBoth)
        );
    }

    #[test]
    fn same_change_on_both_sides_is_adopted() {
        let e = entry(10, T0);
        assert_eq!(
            decide(
                Mode::Sync,
                snap(11, T0 + 60_000),
                snap(11, T0 + 60_000),
                Some(&e)
            ),
            Action::Adopt
        );
    }

    #[test]
    fn deleted_in_destination_is_a_conflict() {
        let e = entry(10, T0);
        assert_eq!(
            decide(Mode::Sync, snap(10, T0), None, Some(&e)),
            Action::Conflict(ConflictKind::DeletedInDestination)
        );
    }

    #[test]
    fn removed_from_source_keeps_the_backup() {
        let e = entry(10, T0);
        assert_eq!(
            decide(Mode::Sync, None, snap(10, T0), Some(&e)),
            Action::KeepRemovedFromSource
        );
    }

    #[test]
    fn gone_from_both_sides_forgets_the_entry() {
        let e = entry(10, T0);
        assert_eq!(
            decide(Mode::Sync, None, None, Some(&e)),
            Action::ForgetEntry
        );
    }

    #[test]
    fn foreign_destination_file_is_ignored() {
        assert_eq!(decide(Mode::Sync, None, snap(10, T0), None), Action::Skip);
    }

    #[test]
    fn mtime_beyond_tolerance_is_a_change() {
        let a = Snapshot::new(10, T0);
        assert!(a.matches(&Snapshot::new(10, T0 + MTIME_TOLERANCE_MS)));
        assert!(!a.matches(&Snapshot::new(10, T0 + MTIME_TOLERANCE_MS + 1)));
    }
}
