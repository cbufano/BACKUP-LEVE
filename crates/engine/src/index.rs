//! The index: what each file looked like after the last successful sync.
//!
//! Stored in SQLite at `<destination>/.backup-leve/indice.sqlite` (ADR 0003).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::decision::{IndexEntry, Snapshot};
use crate::error::{Error, Result};
use crate::paths::STATE_DIR;

pub const INDEX_FILE: &str = "indice.sqlite";
pub const PREVIOUS_INDEX_FILE: &str = "indice.anterior.sqlite";
/// Meta key with the source folder this destination belongs to.
pub const META_SOURCE: &str = "source";
const META_SCHEMA_VERSION: &str = "schema_version";
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS files (
        key          TEXT PRIMARY KEY,
        path         TEXT NOT NULL,
        src_size     INTEGER NOT NULL,
        src_mtime_ms INTEGER NOT NULL,
        dst_size     INTEGER NOT NULL,
        dst_mtime_ms INTEGER NOT NULL,
        synced_at_ms INTEGER NOT NULL
    );
";

#[derive(Debug, Clone)]
pub struct StoredEntry {
    pub path: String,
    pub entry: IndexEntry,
}

pub struct Index {
    conn: Connection,
}

impl Index {
    pub fn path(destination: &Path) -> PathBuf {
        destination.join(STATE_DIR).join(INDEX_FILE)
    }

    /// Opens the index, creating it when it does not exist.
    pub fn open(destination: &Path) -> Result<Self> {
        fs::create_dir_all(destination.join(STATE_DIR))?;
        let conn = Connection::open(Self::path(destination))?;
        // One program writes the index at a time. The exclusive lock is taken once,
        // and WAL then needs no shared-memory file. On a network drive, this saves
        // several round trips to the server on each commit.
        conn.execute_batch("PRAGMA locking_mode = EXCLUSIVE;")?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })?;
        conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(SCHEMA)?;
        let index = Self { conn };
        index.check_schema_version(true)?;
        Ok(index)
    }

    /// Opens an index that already exists, without creating anything. For dry runs.
    pub fn open_existing(destination: &Path) -> Result<Option<Self>> {
        let path = Self::path(destination);
        if !path.is_file() {
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let index = Self { conn };
        index.check_schema_version(false)?;
        Ok(Some(index))
    }

    fn check_schema_version(&self, write_if_missing: bool) -> Result<()> {
        match self.meta(META_SCHEMA_VERSION)? {
            Some(version) => {
                let version: i64 = version.parse().unwrap_or(i64::MAX);
                if version > SCHEMA_VERSION {
                    return Err(Error::UnsupportedIndexVersion(version));
                }
            }
            None if write_if_missing => {
                self.set_meta(META_SCHEMA_VERSION, &SCHEMA_VERSION.to_string())?
            }
            None => {}
        }
        Ok(())
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// All entries, keyed by [`crate::paths::fold_key`].
    pub fn load_all(&self) -> Result<HashMap<String, StoredEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, path, src_size, src_mtime_ms, dst_size, dst_mtime_ms FROM files",
        )?;
        let rows = stmt.query_map([], |row| {
            let source = Snapshot::new(row.get::<_, i64>(2)? as u64, row.get(3)?);
            let destination = Snapshot::new(row.get::<_, i64>(4)? as u64, row.get(5)?);
            Ok((
                row.get::<_, String>(0)?,
                StoredEntry {
                    path: row.get(1)?,
                    entry: IndexEntry {
                        source,
                        destination,
                    },
                },
            ))
        })?;
        let mut entries = HashMap::new();
        for row in rows {
            let (key, entry) = row?;
            entries.insert(key, entry);
        }
        Ok(entries)
    }

    pub fn upsert(
        &self,
        key: &str,
        path: &str,
        entry: &IndexEntry,
        synced_at_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO files (key, path, src_size, src_mtime_ms, dst_size, dst_mtime_ms, synced_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (key) DO UPDATE SET
                path = excluded.path,
                src_size = excluded.src_size,
                src_mtime_ms = excluded.src_mtime_ms,
                dst_size = excluded.dst_size,
                dst_mtime_ms = excluded.dst_mtime_ms,
                synced_at_ms = excluded.synced_at_ms",
            params![
                key,
                path,
                entry.source.size as i64,
                entry.source.mtime_ms,
                entry.destination.size as i64,
                entry.destination.mtime_ms,
                synced_at_ms,
            ],
        )?;
        Ok(())
    }

    pub fn remove(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM files WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Number of files in the index.
    pub fn count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Starts a group of changes. The changes are saved together by
    /// [`Index::commit`]. A change that was not committed is lost.
    pub fn begin(&self) -> Result<()> {
        if self.conn.is_autocommit() {
            self.conn.execute_batch("BEGIN")?;
        }
        Ok(())
    }

    /// Saves the group of changes started by [`Index::begin`].
    pub fn commit(&self) -> Result<()> {
        if !self.conn.is_autocommit() {
            self.conn.execute_batch("COMMIT")?;
        }
        Ok(())
    }

    /// Writes a copy of the index to `indice.anterior.sqlite`.
    pub fn save_previous_copy(&self, destination: &Path) -> Result<()> {
        let target = destination.join(STATE_DIR).join(PREVIOUS_INDEX_FILE);
        if target.exists() {
            fs::remove_file(&target)?;
        }
        let target = target.to_string_lossy().replace('\'', "''");
        self.conn
            .execute_batch(&format!("VACUUM INTO '{target}'"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(size: u64) -> IndexEntry {
        IndexEntry {
            source: Snapshot::new(size, 1_000),
            destination: Snapshot::new(size, 1_000),
        }
    }

    #[test]
    fn stores_updates_and_removes_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let index = Index::open(tmp.path()).unwrap();

        index.upsert("a.txt", "A.txt", &entry(3), 5).unwrap();
        index.upsert("a.txt", "A.txt", &entry(4), 6).unwrap();
        index.upsert("b.txt", "b.txt", &entry(1), 6).unwrap();
        index.remove("b.txt").unwrap();

        let all = index.load_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all["a.txt"].path, "A.txt");
        assert_eq!(all["a.txt"].entry, entry(4));
    }

    #[test]
    fn keeps_data_between_openings() {
        let tmp = tempfile::tempdir().unwrap();
        {
            let index = Index::open(tmp.path()).unwrap();
            index.set_meta(META_SOURCE, "d:\\docs").unwrap();
            index.upsert("a.txt", "a.txt", &entry(3), 5).unwrap();
        }
        let index = Index::open_existing(tmp.path()).unwrap().unwrap();
        assert_eq!(
            index.meta(META_SOURCE).unwrap().as_deref(),
            Some("d:\\docs")
        );
        assert_eq!(index.load_all().unwrap().len(), 1);
    }

    #[test]
    fn open_existing_does_not_create_anything() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(Index::open_existing(tmp.path()).unwrap().is_none());
        assert!(!tmp.path().join(STATE_DIR).exists());
    }

    #[test]
    fn saves_previous_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let index = Index::open(tmp.path()).unwrap();
        index.upsert("a.txt", "a.txt", &entry(3), 5).unwrap();
        index.save_previous_copy(tmp.path()).unwrap();
        index.save_previous_copy(tmp.path()).unwrap();
        assert!(tmp
            .path()
            .join(STATE_DIR)
            .join(PREVIOUS_INDEX_FILE)
            .is_file());
    }

    #[test]
    fn groups_changes_and_loses_only_what_was_not_committed() {
        let tmp = tempfile::tempdir().unwrap();
        {
            let index = Index::open(tmp.path()).unwrap();
            index.begin().unwrap();
            index.upsert("a.txt", "a.txt", &entry(1), 5).unwrap();
            index.upsert("b.txt", "b.txt", &entry(2), 5).unwrap();
            index.commit().unwrap();
            index.begin().unwrap();
            index.upsert("c.txt", "c.txt", &entry(3), 6).unwrap();
            // Closed without commit, like a program that stops abruptly.
        }
        let index = Index::open(tmp.path()).unwrap();
        assert_eq!(index.count().unwrap(), 2);
        assert!(!index.load_all().unwrap().contains_key("c.txt"));
    }

    #[test]
    fn a_second_program_cannot_write_while_the_index_is_open() {
        let tmp = tempfile::tempdir().unwrap();
        let first = Index::open(tmp.path()).unwrap();
        first.upsert("a.txt", "a.txt", &entry(1), 5).unwrap();
        assert!(Index::open(tmp.path()).is_err());
        drop(first);
        assert_eq!(Index::open(tmp.path()).unwrap().count().unwrap(), 1);
    }

    #[test]
    fn refuses_newer_schema() {
        let tmp = tempfile::tempdir().unwrap();
        {
            let index = Index::open(tmp.path()).unwrap();
            index.set_meta(META_SCHEMA_VERSION, "99").unwrap();
        }
        assert!(matches!(
            Index::open(tmp.path()),
            Err(Error::UnsupportedIndexVersion(99))
        ));
    }
}
