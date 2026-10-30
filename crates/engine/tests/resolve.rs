//! Conflict choices on real folders. A temporary folder stands in for the system trash.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use backup_leve_engine::decision::Mode;
use backup_leve_engine::report::ConflictItem;
use backup_leve_engine::resolve::{resolve, Choice, Decision, Outcome};
use backup_leve_engine::{run, Options, Report};
use filetime::FileTime;
use tempfile::TempDir;

const T0: i64 = 1_700_000_000;

struct Folders {
    _tmp: TempDir,
    source: PathBuf,
    destination: PathBuf,
    trash: PathBuf,
}

fn write(path: &Path, content: &str, mtime_secs: i64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
    filetime::set_file_mtime(path, FileTime::from_unix_time(mtime_secs, 0)).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn sync(f: &Folders) -> Report {
    run(&f.source, &f.destination, &Options::default(), |_| {}).unwrap()
}

/// Synced `a.txt`, then the destination copy was changed by someone.
fn changed_in_destination() -> (Folders, ConflictItem) {
    let tmp = tempfile::tempdir().unwrap();
    let f = Folders {
        source: tmp.path().join("origem"),
        destination: tmp.path().join("destino"),
        trash: tmp.path().join("lixeira"),
        _tmp: tmp,
    };
    fs::create_dir_all(&f.trash).unwrap();
    write(&f.source.join("a.txt"), "da origem", T0);
    sync(&f);
    write(
        &f.destination.join("a.txt"),
        "mexido no destino",
        T0 + 3_600,
    );
    let conflict = sync(&f).conflicts.remove(0);
    (f, conflict)
}

fn decide(f: &Folders, conflict: &ConflictItem, choice: Choice) -> Outcome {
    let trash_dir = f.trash.clone();
    let trash = move |path: &Path| -> io::Result<()> {
        fs::rename(path, trash_dir.join(path.file_name().unwrap()))
    };
    let decision = Decision {
        path: conflict.path.clone(),
        choice,
        source: conflict.source,
        destination: conflict.destination,
    };
    resolve(&f.source, &f.destination, &[decision], &trash)
        .unwrap()
        .remove(0)
}

#[test]
fn source_wins_keeps_the_destination_version_in_replaced() {
    let (f, conflict) = changed_in_destination();

    let outcome = decide(&f, &conflict, Choice::SourceWins);

    assert!(outcome.ok, "{}", outcome.message);
    assert_eq!(read(&f.destination.join("a.txt")), "da origem");
    assert!(sync(&f).conflicts.is_empty());
}

#[test]
fn destination_wins_sends_the_source_version_to_the_trash() {
    let (f, conflict) = changed_in_destination();

    let outcome = decide(&f, &conflict, Choice::DestinationWins);

    assert!(outcome.ok, "{}", outcome.message);
    assert_eq!(read(&f.source.join("a.txt")), "mexido no destino");
    assert_eq!(read(&f.trash.join("a.txt")), "da origem");
    assert!(sync(&f).conflicts.is_empty());
}

#[test]
fn keep_both_renames_the_destination_version_with_its_date() {
    let (f, conflict) = changed_in_destination();

    let outcome = decide(&f, &conflict, Choice::KeepBoth);

    assert!(outcome.ok, "{}", outcome.message);
    assert_eq!(read(&f.destination.join("a.txt")), "da origem");
    let kept: Vec<String> = fs::read_dir(&f.destination)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("a (destino "))
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(read(&f.destination.join(&kept[0])), "mexido no destino");
    assert_eq!(read(&f.source.join("a.txt")), "da origem");
    let next = sync(&f);
    assert!(next.conflicts.is_empty());
    assert!(next.copied.is_empty());
}

#[test]
fn file_changed_after_the_check_is_not_touched() {
    let (f, conflict) = changed_in_destination();
    write(
        &f.destination.join("a.txt"),
        "mexido de novo, outra vez",
        T0 + 7_200,
    );

    let outcome = decide(&f, &conflict, Choice::SourceWins);

    assert!(!outcome.ok);
    assert_eq!(
        read(&f.destination.join("a.txt")),
        "mexido de novo, outra vez"
    );
}

#[test]
fn deleted_in_destination_can_be_copied_again_or_deleted_in_source() {
    let (f, _) = changed_in_destination();
    decide(&f, &sync(&f).conflicts.remove(0), Choice::SourceWins);
    write(&f.source.join("b.txt"), "bê", T0);
    sync(&f);
    fs::remove_file(f.destination.join("a.txt")).unwrap();
    fs::remove_file(f.destination.join("b.txt")).unwrap();
    let conflicts = sync(&f).conflicts;
    assert_eq!(conflicts.len(), 2);

    let again = decide(&f, &conflicts[0], Choice::SourceWins);
    let deleted = decide(&f, &conflicts[1], Choice::DeleteInSource);

    assert!(again.ok && deleted.ok);
    assert_eq!(read(&f.destination.join("a.txt")), "da origem");
    assert!(!f.source.join("b.txt").exists());
    assert_eq!(read(&f.trash.join("b.txt")), "bê");
    assert!(sync(&f).conflicts.is_empty());
}

#[test]
fn option_that_does_not_fit_the_conflict_is_refused() {
    let (f, conflict) = changed_in_destination();

    let outcome = decide(&f, &conflict, Choice::DeleteInSource);

    assert!(!outcome.ok);
    assert!(f.source.join("a.txt").exists());
}

#[test]
fn cancelled_run_stops_and_still_saves_the_report() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("origem");
    let destination = tmp.path().join("destino");
    write(&source.join("a.txt"), "x", T0);
    let options = Options {
        mode: Mode::Sync,
        dry_run: false,
        cancel: Some(Arc::new(AtomicBool::new(true))),
        ..Default::default()
    };

    let report = run(&source, &destination, &options, |_| {}).unwrap();

    assert!(report.cancelled);
    assert!(report.copied.is_empty());
    assert!(report.saved_to.unwrap().is_file());
}
