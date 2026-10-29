//! End-to-end runs on real folders. One test per rule in `docs/REGRAS-DE-SINCRONISMO.md`.

use std::fs;
use std::path::{Path, PathBuf};

use backup_leve_engine::decision::{ConflictKind, Mode};
use backup_leve_engine::paths::{PARTIAL_SUFFIX, REPLACED_DIR, STATE_DIR};
use backup_leve_engine::report::FileItem;
use backup_leve_engine::{run, Error, Options, Report};
use filetime::FileTime;
use tempfile::TempDir;

const T0: i64 = 1_700_000_000;

struct Folders {
    _tmp: TempDir,
    source: PathBuf,
    destination: PathBuf,
}

fn folders() -> Folders {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("origem");
    let destination = tmp.path().join("destino");
    fs::create_dir_all(&source).unwrap();
    Folders {
        _tmp: tmp,
        source,
        destination,
    }
}

fn write(path: &Path, content: &str, mtime_secs: i64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
    filetime::set_file_mtime(path, FileTime::from_unix_time(mtime_secs, 0)).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn sync(f: &Folders, mode: Mode) -> Report {
    try_sync(f, mode, false).unwrap()
}

fn try_sync(f: &Folders, mode: Mode, dry_run: bool) -> backup_leve_engine::Result<Report> {
    run(
        &f.source,
        &f.destination,
        &Options { mode, dry_run },
        |_| {},
    )
}

fn names(items: &[FileItem]) -> Vec<&str> {
    items.iter().map(|i| i.path.as_str()).collect()
}

/// First run with `a.txt` and `pasta/b.txt`.
fn synced() -> Folders {
    let f = folders();
    write(&f.source.join("a.txt"), "versão 1", T0);
    write(&f.source.join("pasta").join("b.txt"), "bê", T0);
    sync(&f, Mode::Sync);
    f
}

#[test]
fn first_run_copies_everything() {
    let f = folders();
    write(&f.source.join("a.txt"), "versão 1", T0);
    write(&f.source.join("pasta").join("b.txt"), "bê", T0);

    let report = sync(&f, Mode::Sync);

    assert_eq!(names(&report.copied), ["a.txt", "pasta/b.txt"]);
    assert_eq!(read(&f.destination.join("pasta").join("b.txt")), "bê");
    let mtime =
        FileTime::from_last_modification_time(&fs::metadata(f.destination.join("a.txt")).unwrap());
    assert_eq!(mtime.unix_seconds(), T0);
    assert!(report.errors.is_empty());
    assert!(report.saved_to.as_ref().unwrap().is_file());
}

#[test]
fn second_run_copies_nothing() {
    let f = synced();

    let report = sync(&f, Mode::Sync);

    assert!(report.copied.is_empty());
    assert_eq!(report.unchanged, 2);
}

#[test]
fn source_change_updates_and_keeps_the_old_version() {
    let f = synced();
    write(&f.source.join("a.txt"), "versão 2, mais longa", T0 + 100);

    let report = sync(&f, Mode::Sync);

    assert_eq!(names(&report.updated), ["a.txt"]);
    assert_eq!(read(&f.destination.join("a.txt")), "versão 2, mais longa");
    let replaced_root = f.destination.join(STATE_DIR).join(REPLACED_DIR);
    let run_folder = fs::read_dir(&replaced_root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(read(&run_folder.path().join("a.txt")), "versão 1");
}

#[test]
fn add_only_mode_reports_source_change_without_copying() {
    let f = synced();
    write(&f.source.join("a.txt"), "versão 2, mais longa", T0 + 100);

    let report = sync(&f, Mode::AddOnly);

    assert_eq!(names(&report.changed_in_source), ["a.txt"]);
    assert_eq!(read(&f.destination.join("a.txt")), "versão 1");
}

#[test]
fn destination_change_is_a_conflict_and_nothing_is_touched() {
    let f = synced();
    write(&f.destination.join("a.txt"), "mexeram no backup", T0 + 50);

    let report = sync(&f, Mode::Sync);

    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0].kind, ConflictKind::ChangedInDestination);
    assert_eq!(read(&f.destination.join("a.txt")), "mexeram no backup");
    assert_eq!(read(&f.source.join("a.txt")), "versão 1");
}

#[test]
fn change_on_both_sides_is_a_conflict() {
    let f = synced();
    write(&f.source.join("a.txt"), "origem nova", T0 + 100);
    write(&f.destination.join("a.txt"), "destino novo!", T0 + 50);

    let report = sync(&f, Mode::Sync);

    assert_eq!(report.conflicts[0].kind, ConflictKind::ChangedInBoth);
    assert_eq!(read(&f.destination.join("a.txt")), "destino novo!");
}

#[test]
fn deleted_in_destination_is_a_conflict_and_is_not_recreated() {
    let f = synced();
    fs::remove_file(f.destination.join("a.txt")).unwrap();

    let report = sync(&f, Mode::Sync);

    assert_eq!(report.conflicts[0].kind, ConflictKind::DeletedInDestination);
    assert!(!f.destination.join("a.txt").exists());
}

#[test]
fn removed_from_source_keeps_the_backup() {
    let f = synced();
    fs::remove_file(f.source.join("a.txt")).unwrap();

    let report = sync(&f, Mode::Sync);

    assert_eq!(names(&report.removed_from_source), ["a.txt"]);
    assert_eq!(read(&f.destination.join("a.txt")), "versão 1");
}

#[test]
fn gone_from_both_sides_leaves_the_index() {
    let f = synced();
    fs::remove_file(f.source.join("a.txt")).unwrap();
    fs::remove_file(f.destination.join("a.txt")).unwrap();

    let first = sync(&f, Mode::Sync);
    let second = sync(&f, Mode::Sync);

    assert_eq!(first.forgotten, 1);
    assert_eq!(second.forgotten, 0);
}

#[test]
fn identical_files_already_in_destination_are_adopted() {
    let f = folders();
    write(&f.source.join("a.txt"), "igual", T0);
    write(&f.destination.join("a.txt"), "igual", T0);

    let report = sync(&f, Mode::Sync);

    assert_eq!(names(&report.adopted), ["a.txt"]);
    assert!(report.copied.is_empty());
    assert_eq!(sync(&f, Mode::Sync).unchanged, 1);
}

#[test]
fn different_file_already_in_destination_is_a_conflict() {
    let f = folders();
    write(&f.source.join("a.txt"), "da origem", T0);
    write(&f.destination.join("a.txt"), "outro conteúdo", T0 + 10);

    let report = sync(&f, Mode::Sync);

    assert_eq!(
        report.conflicts[0].kind,
        ConflictKind::ExistsWithDifferentContent
    );
    assert_eq!(read(&f.destination.join("a.txt")), "outro conteúdo");
}

#[test]
fn foreign_files_in_destination_are_ignored() {
    let f = synced();
    write(&f.destination.join("outro.txt"), "não é do backup", T0);

    let report = sync(&f, Mode::Sync);

    assert!(report.conflicts.is_empty());
    assert!(report.removed_from_source.is_empty());
    assert!(f.destination.join("outro.txt").exists());
}

#[test]
fn dry_run_writes_nothing() {
    let f = folders();
    write(&f.source.join("a.txt"), "x", T0);

    let report = try_sync(&f, Mode::Sync, true).unwrap();

    assert_eq!(names(&report.copied), ["a.txt"]);
    assert!(!f.destination.exists());
    assert!(report.saved_to.is_none());
}

#[test]
fn excluded_names_and_leftover_partials() {
    let f = folders();
    write(&f.source.join("a.txt"), "x", T0);
    write(&f.source.join("Thumbs.db"), "x", T0);
    write(
        &f.destination.join(format!("a.txt{PARTIAL_SUFFIX}")),
        "pela metade",
        T0,
    );

    let report = sync(&f, Mode::Sync);

    assert_eq!(names(&report.copied), ["a.txt"]);
    assert_eq!(report.partials_removed, 1);
    assert!(!f.destination.join("Thumbs.db").exists());
    assert!(!f
        .destination
        .join(format!("a.txt{PARTIAL_SUFFIX}"))
        .exists());
}

#[test]
fn destination_inside_source_is_refused() {
    let f = folders();
    let destination = f.source.join("backup");

    let result = run(
        &f.source,
        &destination,
        &Options {
            mode: Mode::Sync,
            dry_run: false,
        },
        |_| {},
    );

    assert!(matches!(result, Err(Error::NestedFolders(..))));
    assert!(!destination.exists());
}

#[test]
fn destination_of_another_source_is_refused() {
    let f = synced();
    let other = f.source.parent().unwrap().join("outra-origem");
    fs::create_dir_all(&other).unwrap();

    let result = run(
        &other,
        &f.destination,
        &Options {
            mode: Mode::Sync,
            dry_run: false,
        },
        |_| {},
    );

    assert!(matches!(result, Err(Error::IndexBelongsToOtherSource(_))));
}

#[test]
fn missing_source_is_an_error() {
    let f = folders();
    fs::remove_dir(&f.source).unwrap();

    assert!(matches!(
        try_sync(&f, Mode::Sync, false),
        Err(Error::SourceNotFound(_))
    ));
}
