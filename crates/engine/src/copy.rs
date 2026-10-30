//! Safe copy: partial file first, final name only when the copy is complete.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use filetime::FileTime;

use crate::decision::Snapshot;
use crate::paths::PARTIAL_SUFFIX;
use crate::scan::mtime_ms;

/// Files from this size up are copied in blocks, with progress after each block.
/// Smaller files use the system copy, which is faster for many small files.
pub const BLOCK_COPY_FROM: u64 = 32 * 1024 * 1024;
const BLOCK_SIZE: usize = 4 * 1024 * 1024;

/// Copies `source` to `target` and gives the copy the modification time `source_mtime_ms`.
///
/// If `target` exists, it is moved to `replaced` first. With `replaced` set to
/// `None`, an existing `target` is an error: the engine never overwrites silently.
///
/// Returns the snapshot of `target` after the copy.
pub fn copy_file(
    source: &Path,
    target: &Path,
    source_mtime_ms: i64,
    replaced: Option<&Path>,
) -> io::Result<Snapshot> {
    copy_file_with_progress(source, target, source_mtime_ms, replaced, &mut |_| true)
}

/// Like [`copy_file`]. For large files, `on_bytes` receives the bytes copied so far
/// after each block. When it returns `false`, the copy stops with `ErrorKind::Interrupted`.
pub fn copy_file_with_progress(
    source: &Path,
    target: &Path,
    source_mtime_ms: i64,
    replaced: Option<&Path>,
    on_bytes: &mut dyn FnMut(u64) -> bool,
) -> io::Result<Snapshot> {
    if replaced.is_none() && target.symlink_metadata().is_ok() {
        return Err(already_exists());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial = partial_path(target);
    let result = copy_through_partial(
        source,
        target,
        &partial,
        source_mtime_ms,
        replaced,
        on_bytes,
    );
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

fn already_exists() -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        "o arquivo já existe no destino",
    )
}

fn copy_through_partial(
    source: &Path,
    target: &Path,
    partial: &Path,
    source_mtime_ms: i64,
    replaced: Option<&Path>,
    on_bytes: &mut dyn FnMut(u64) -> bool,
) -> io::Result<Snapshot> {
    if fs::metadata(source)?.len() >= BLOCK_COPY_FROM {
        copy_in_blocks(source, partial, on_bytes)?;
    } else {
        fs::copy(source, partial)?;
    }
    set_mtime(partial, source_mtime_ms)?;

    if target.symlink_metadata().is_ok() {
        let Some(replaced) = replaced else {
            return Err(already_exists());
        };
        if let Some(parent) = replaced.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(target, replaced)?;
    }
    fs::rename(partial, target)?;

    let metadata = fs::metadata(target)?;
    Ok(Snapshot::new(metadata.len(), mtime_ms(&metadata)))
}

fn copy_in_blocks(
    source: &Path,
    partial: &Path,
    on_bytes: &mut dyn FnMut(u64) -> bool,
) -> io::Result<()> {
    let mut reader = File::open(source)?;
    let mut writer = File::create(partial)?;
    let mut block = vec![0u8; BLOCK_SIZE];
    let mut copied = 0u64;
    loop {
        let read = match reader.read(&mut block) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        };
        writer.write_all(&block[..read])?;
        copied += read as u64;
        if !on_bytes(copied) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "cópia interrompida pelo usuário",
            ));
        }
    }
    drop(writer);
    fs::set_permissions(partial, fs::metadata(source)?.permissions())
}

/// `<name>.backup-leve-parcial` next to `target`.
pub fn partial_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(PARTIAL_SUFFIX);
    target.with_file_name(name)
}

/// Sets the modification time, also on read-only files.
///
/// The read-only flag is cleared only for the change and set again right after.
#[allow(clippy::permissions_set_readonly_false)]
fn set_mtime(path: &Path, mtime_ms: i64) -> io::Result<()> {
    let time = FileTime::from_unix_time(
        mtime_ms.div_euclid(1_000),
        (mtime_ms.rem_euclid(1_000) * 1_000_000) as u32,
    );
    match filetime::set_file_mtime(path, time) {
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            let mut permissions = fs::metadata(path)?.permissions();
            if !permissions.readonly() {
                return Err(err);
            }
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions.clone())?;
            let result = filetime::set_file_mtime(path, time);
            permissions.set_readonly(true);
            fs::set_permissions(path, permissions)?;
            result
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MTIME_MS: i64 = 1_700_000_000_000;

    #[test]
    fn copies_content_and_modification_time() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("a.txt");
        let target = tmp.path().join("destino").join("sub").join("a.txt");
        fs::write(&source, "conteúdo").unwrap();

        let snapshot = copy_file(&source, &target, MTIME_MS, None).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "conteúdo");
        assert_eq!(snapshot.mtime_ms, MTIME_MS);
        assert_eq!(snapshot.size, "conteúdo".len() as u64);
        assert!(!partial_path(&target).exists());
    }

    #[test]
    fn never_overwrites_without_a_place_for_the_old_version() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("a.txt");
        let target = tmp.path().join("b.txt");
        fs::write(&source, "novo").unwrap();
        fs::write(&target, "antigo").unwrap();

        let err = copy_file(&source, &target, MTIME_MS, None).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&target).unwrap(), "antigo");
        assert!(!partial_path(&target).exists());
    }

    #[test]
    fn moves_the_old_version_before_replacing() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("a.txt");
        let target = tmp.path().join("b.txt");
        let replaced = tmp.path().join("substituidos").join("b.txt");
        fs::write(&source, "novo").unwrap();
        fs::write(&target, "antigo").unwrap();

        copy_file(&source, &target, MTIME_MS, Some(&replaced)).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "novo");
        assert_eq!(fs::read_to_string(&replaced).unwrap(), "antigo");
    }

    #[test]
    fn large_files_report_progress_and_can_be_stopped() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("grande.bin");
        let size = BLOCK_COPY_FROM + 1_000;
        fs::write(&source, vec![7u8; size as usize]).unwrap();

        let mut steps = Vec::new();
        let target = tmp.path().join("copia.bin");
        let snapshot = copy_file_with_progress(&source, &target, MTIME_MS, None, &mut |bytes| {
            steps.push(bytes);
            true
        })
        .unwrap();
        assert_eq!(snapshot.size, size);
        assert_eq!(*steps.last().unwrap(), size);
        assert!(steps.len() > 1);
        assert_eq!(fs::read(&target).unwrap(), fs::read(&source).unwrap());

        let stopped = tmp.path().join("parada.bin");
        let err =
            copy_file_with_progress(&source, &stopped, MTIME_MS, None, &mut |_| false).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        assert!(!stopped.exists());
        assert!(!partial_path(&stopped).exists());
    }

    #[test]
    fn copies_read_only_files() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("a.txt");
        let target = tmp.path().join("b.txt");
        fs::write(&source, "x").unwrap();
        let mut permissions = fs::metadata(&source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&source, permissions).unwrap();

        let snapshot = copy_file(&source, &target, MTIME_MS, None).unwrap();

        assert_eq!(snapshot.mtime_ms, MTIME_MS);
        // Lets the temporary folder be removed on Windows.
        for path in [&source, &target] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions).unwrap();
        }
    }
}
