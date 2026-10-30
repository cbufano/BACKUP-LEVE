//! Applies the user's choices for conflicts. See "Conflitos" in `docs/REGRAS-DE-SINCRONISMO.md`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::copy::copy_file;
use crate::decision::{IndexEntry, Snapshot};
use crate::error::Result;
use crate::index::Index;
use crate::job::{check_index_source, check_not_nested};
use crate::paths::{self, REPLACED_DIR, REPORTS_DIR, STATE_DIR};
use crate::scan::mtime_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    /// The source is copied to the destination. The old destination goes to `substituidos`.
    SourceWins,
    /// The destination is copied to the source. The old source goes to the trash.
    DestinationWins,
    /// The destination version gets its date in the name; the source is copied next to it.
    KeepBoth,
    /// Only for files deleted in the destination: the source goes to the trash.
    DeleteInSource,
}

/// A choice for one conflict, with the snapshots the user saw.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub path: String,
    pub choice: Choice,
    pub source: Option<Snapshot>,
    pub destination: Option<Snapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub path: String,
    pub choice: Choice,
    pub ok: bool,
    /// What happened, in Portuguese.
    pub message: String,
}

/// Moves a file to the system trash. The application passes [`system_trash`].
pub type Trash<'a> = &'a dyn Fn(&Path) -> io::Result<()>;

pub fn system_trash(path: &Path) -> io::Result<()> {
    trash::delete(path).map_err(io::Error::other)
}

/// Applies each decision. A decision that fails does not stop the others.
///
/// A file that changed after the user saw the conflict is not touched.
/// The outcomes are also saved in `relatorios/decisoes_<date>.json`.
pub fn resolve(
    source_root: &Path,
    destination_root: &Path,
    decisions: &[Decision],
    trash: Trash<'_>,
) -> Result<Vec<Outcome>> {
    let source_id = check_not_nested(source_root, destination_root)?;
    let index = Index::open(destination_root)?;
    check_index_source(&index, &source_id, false)?;

    let now = Local::now();
    let stamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();
    let replaced_root = destination_root
        .join(STATE_DIR)
        .join(REPLACED_DIR)
        .join(&stamp);

    let outcomes: Vec<Outcome> = decisions
        .iter()
        .map(|decision| {
            let result = apply(
                source_root,
                destination_root,
                &replaced_root,
                &index,
                decision,
                trash,
            );
            let (ok, message) = match result {
                Ok(message) => (true, message),
                Err(err) => (false, err.to_string()),
            };
            Outcome {
                path: decision.path.clone(),
                choice: decision.choice,
                ok,
                message,
            }
        })
        .collect();

    let log_dir = destination_root.join(STATE_DIR).join(REPORTS_DIR);
    fs::create_dir_all(&log_dir)?;
    fs::write(
        log_dir.join(format!("decisoes_{stamp}.json")),
        serde_json::to_vec_pretty(&outcomes)?,
    )?;
    Ok(outcomes)
}

fn apply(
    source_root: &Path,
    destination_root: &Path,
    replaced_root: &Path,
    index: &Index,
    decision: &Decision,
    trash: Trash<'_>,
) -> io::Result<String> {
    let key = paths::fold_key(&decision.path);
    let source = paths::join_relative(source_root, &decision.path);
    let destination = paths::join_relative(destination_root, &decision.path);
    let source_now = snapshot(&source)?;
    let destination_now = snapshot(&destination)?;

    if !same(source_now, decision.source) || !same(destination_now, decision.destination) {
        return Err(refused(
            "o arquivo mudou depois da verificação; sincronize de novo para ver o estado atual",
        ));
    }

    match (decision.choice, source_now, destination_now) {
        (Choice::SourceWins, Some(src), dst) => {
            let replaced = dst.map(|_| paths::join_relative(replaced_root, &decision.path));
            let copied = copy_file(&source, &destination, src.mtime_ms, replaced.as_deref())?;
            record(index, &key, &decision.path, src, copied)?;
            Ok(if dst.is_some() {
                "a origem foi copiada; a versão do destino está em .backup-leve/substituidos".into()
            } else {
                "a origem foi copiada de novo para o destino".into()
            })
        }
        (Choice::DestinationWins, src, Some(dst)) => {
            if src.is_some() {
                trash(&source)?;
            }
            let copied = copy_file(&destination, &source, dst.mtime_ms, None).map_err(|err| {
                io::Error::other(format!(
                    "{err}; a versão anterior da origem está na Lixeira"
                ))
            })?;
            record(index, &key, &decision.path, copied, dst)?;
            Ok("o destino foi copiado para a origem; a versão da origem está na Lixeira".into())
        }
        (Choice::KeepBoth, Some(src), Some(dst)) => {
            let kept = dated_name(&destination, "destino", dst.mtime_ms);
            fs::rename(&destination, &kept)?;
            let copied = copy_file(&source, &destination, src.mtime_ms, None)?;
            record(index, &key, &decision.path, src, copied)?;
            Ok(format!(
                "a versão do destino agora se chama \"{}\"",
                kept.file_name().unwrap_or_default().to_string_lossy()
            ))
        }
        (Choice::DeleteInSource, Some(_), None) => {
            trash(&source)?;
            index.remove(&key).map_err(io::Error::other)?;
            Ok("o arquivo da origem está na Lixeira".into())
        }
        _ => Err(refused("esta opção não vale para este conflito")),
    }
}

fn record(
    index: &Index,
    key: &str,
    path: &str,
    source: Snapshot,
    destination: Snapshot,
) -> io::Result<()> {
    let entry = IndexEntry {
        source,
        destination,
    };
    index
        .upsert(key, path, &entry, Utc::now().timestamp_millis())
        .map_err(io::Error::other)
}

fn snapshot(path: &Path) -> io::Result<Option<Snapshot>> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(Snapshot::new(metadata.len(), mtime_ms(&metadata)))),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn same(now: Option<Snapshot>, seen: Option<Snapshot>) -> bool {
    match (now, seen) {
        (Some(now), Some(seen)) => now.matches(&seen),
        (None, None) => true,
        _ => false,
    }
}

fn refused(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}

/// `relatorio.docx` → `relatorio (destino 2026-10-29 09-12).docx`, unique in its folder.
pub fn dated_name(path: &Path, side: &str, mtime_ms: i64) -> PathBuf {
    let date = DateTime::from_timestamp_millis(mtime_ms)
        .map(|time| {
            time.with_timezone(&Local)
                .format("%Y-%m-%d %H-%M")
                .to_string()
        })
        .unwrap_or_default();
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    let mut counter = 1;
    loop {
        let suffix = if counter == 1 {
            String::new()
        } else {
            format!(" {counter}")
        };
        let candidate = path.with_file_name(format!("{stem} ({side} {date}{suffix}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}
