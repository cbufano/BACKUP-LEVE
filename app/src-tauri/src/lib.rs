//! Commands the window calls. All decisions about files stay in `backup-leve-engine`.

mod ai;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use backup_leve_engine::exclude::{Rule, REBUILDABLE, TEMPORARY};
use backup_leve_engine::paths::{REPORTS_DIR, STATE_DIR};
use backup_leve_engine::profile::{self, folder_label, Profile};
use backup_leve_engine::resolve::{self, Decision, Outcome};
use backup_leve_engine::settings::{self, Settings};
use backup_leve_engine::{run as run_job, Event, Options, Report, Side};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

/// Progress events are sent at most this often.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Finished files sent with one progress event. The window shows the newest ones.
const RECENT_PER_EVENT: usize = 100;
/// Reports listed per source folder.
const REPORTS_PER_TARGET: usize = 15;

#[derive(Default)]
struct RunState {
    cancel: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    target: usize,
    targets: usize,
    label: String,
    stage: &'static str,
    /// Folder being read: "origem" or "destino". Empty while copying.
    side: &'static str,
    /// Files found so far in the source and in the destination.
    source_found: u64,
    destination_found: u64,
    /// Files the index expects in the destination. Zero when unknown.
    destination_expected: u64,
    /// Files already in the backup and unchanged: not copied again.
    already_done: u64,
    files_done: u64,
    files_total: u64,
    bytes_done: u64,
    bytes_total: u64,
    /// File being read or copied now.
    path: String,
    /// Files finished since the previous event, oldest first.
    recent: Vec<RecentFile>,
}

#[derive(Clone, Serialize)]
struct RecentFile {
    path: String,
    size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetResult {
    label: String,
    source: PathBuf,
    destination: PathBuf,
    report: Option<Report>,
    report_path: Option<PathBuf>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportEntry {
    label: String,
    path: PathBuf,
    stamp: String,
}

fn profiles_path() -> Result<PathBuf, String> {
    profile::default_path()
        .ok_or_else(|| "a pasta de configuração do usuário não foi encontrada".into())
}

fn find_profile(name: &str) -> Result<Profile, String> {
    profile::load(&profiles_path()?)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|profile| profile.name == name)
        .ok_or_else(|| format!("o perfil \"{name}\" não existe"))
}

#[tauri::command]
fn list_profiles() -> Result<Vec<Profile>, String> {
    profile::load(&profiles_path()?).map_err(|err| err.to_string())
}

#[tauri::command]
fn save_profiles(profiles: Vec<Profile>) -> Result<(), String> {
    profile::save(&profiles_path()?, &profiles).map_err(|err| err.to_string())
}

#[tauri::command]
async fn run_profile(
    app: AppHandle,
    state: State<'_, RunState>,
    name: String,
    dry_run: bool,
) -> Result<Vec<TargetResult>, String> {
    let profile = find_profile(&name)?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.cancel.lock().map_err(|err| err.to_string())?;
        if slot.is_some() {
            return Err("já existe uma sincronização em andamento".into());
        }
        *slot = Some(cancel.clone());
    }
    let result =
        tauri::async_runtime::spawn_blocking(move || run_targets(&app, &profile, dry_run, &cancel))
            .await;
    if let Ok(mut slot) = state.cancel.lock() {
        *slot = None;
    }
    result.map_err(|err| err.to_string())
}

fn run_targets(
    app: &AppHandle,
    profile: &Profile,
    dry_run: bool,
    cancel: &Arc<AtomicBool>,
) -> Vec<TargetResult> {
    let targets = profile.targets();
    let mut results = Vec::with_capacity(targets.len());
    for (number, target) in targets.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let label = folder_label(&target.source);
        let options = Options {
            mode: profile.mode,
            dry_run,
            cancel: Some(cancel.clone()),
            exclusions: profile.exclusions(),
        };
        let mut progress = Progress {
            target: number + 1,
            targets: targets.len(),
            label: label.clone(),
            stage: "lendo",
            side: "origem",
            source_found: 0,
            destination_found: 0,
            destination_expected: 0,
            already_done: 0,
            files_done: 0,
            files_total: 0,
            bytes_done: 0,
            bytes_total: 0,
            path: String::new(),
            recent: Vec::new(),
        };
        let mut last_sent: Option<Instant> = None;
        let mut bytes_at_last_file = 0;
        let outcome = run_job(&target.source, &target.destination, &options, |event| {
            let due = last_sent.is_none_or(|last| last.elapsed() >= PROGRESS_INTERVAL);
            match event {
                Event::Scanning { side, expected } => {
                    progress.stage = "lendo";
                    progress.side = side_name(side);
                    if side == Side::Destination {
                        progress.destination_expected = expected;
                    }
                    progress.path.clear();
                }
                Event::ScanProgress { side, found, path } => {
                    match side {
                        Side::Source => progress.source_found = found,
                        Side::Destination => progress.destination_found = found,
                    }
                    if !due {
                        return;
                    }
                    progress.path = path.to_owned();
                }
                Event::Planned {
                    files,
                    bytes,
                    already,
                } => {
                    progress.stage = "copiando";
                    progress.already_done = already;
                    progress.side = "";
                    progress.path.clear();
                    progress.files_total = files;
                    progress.bytes_total = bytes;
                }
                Event::FileProgress { path, done_bytes } => {
                    progress.bytes_done = done_bytes;
                    progress.path = path.to_owned();
                    if !due {
                        return;
                    }
                }
                Event::FileDone {
                    path,
                    done_files,
                    done_bytes,
                } => {
                    if progress.recent.len() == RECENT_PER_EVENT {
                        progress.recent.remove(0);
                    }
                    progress.recent.push(RecentFile {
                        path: path.to_owned(),
                        size: done_bytes - bytes_at_last_file,
                    });
                    bytes_at_last_file = done_bytes;
                    progress.files_done = done_files;
                    progress.bytes_done = done_bytes;
                    progress.path = path.to_owned();
                    if !due && done_files < progress.files_total {
                        return;
                    }
                }
            }
            last_sent = Some(Instant::now());
            let _ = app.emit("progresso", progress.clone());
            progress.recent.clear();
        });
        results.push(match outcome {
            Ok(report) => TargetResult {
                label,
                source: target.source.clone(),
                destination: target.destination.clone(),
                report_path: report.saved_to.clone(),
                report: Some(report),
                error: None,
            },
            Err(err) => TargetResult {
                label,
                source: target.source.clone(),
                destination: target.destination.clone(),
                report: None,
                report_path: None,
                error: Some(err.to_string()),
            },
        });
    }
    results
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Source => "origem",
        Side::Destination => "destino",
    }
}

#[tauri::command]
fn cancel_run(state: State<'_, RunState>) {
    if let Ok(slot) = state.cancel.lock() {
        if let Some(cancel) = slot.as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

#[tauri::command]
async fn resolve_conflicts(
    source: PathBuf,
    destination: PathBuf,
    decisions: Vec<Decision>,
) -> Result<Vec<Outcome>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        resolve::resolve(&source, &destination, &decisions, &resolve::system_trash)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_reports(name: String) -> Result<Vec<ReportEntry>, String> {
    let profile = find_profile(&name)?;
    let mut entries = Vec::new();
    for target in profile.targets() {
        let dir = target.destination.join(STATE_DIR).join(REPORTS_DIR);
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        let mut found: Vec<ReportEntry> = read
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let stamp = name.strip_prefix("relatorio_")?.strip_suffix(".html")?;
                Some(ReportEntry {
                    label: folder_label(&target.source),
                    path: entry.path(),
                    stamp: stamp.to_owned(),
                })
            })
            .collect();
        found.sort_by(|a, b| b.stamp.cmp(&a.stamp));
        found.truncate(REPORTS_PER_TARGET);
        entries.extend(found);
    }
    entries.sort_by(|a, b| b.stamp.cmp(&a.stamp));
    Ok(entries)
}

/// Opens an HTML report. Only files inside a `.backup-leve/relatorios` folder.
#[tauri::command]
fn open_report(path: PathBuf) -> Result<(), String> {
    let is_report = path.extension().is_some_and(|ext| ext == "html")
        && path
            .parent()
            .is_some_and(|dir| dir.file_name().is_some_and(|name| name == REPORTS_DIR))
        && path
            .parent()
            .and_then(Path::parent)
            .is_some_and(|dir| dir.file_name().is_some_and(|name| name == STATE_DIR));
    if !is_report || !path.is_file() {
        return Err("este arquivo não é um relatório do Backup Leve".into());
    }
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|err| err.to_string())
}

/// Opens a folder in the file manager.
#[tauri::command]
fn open_folder(path: PathBuf) -> Result<(), String> {
    if !path.is_dir() {
        return Err("a pasta não existe ou o disco não está conectado".into());
    }
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|err| err.to_string())
}

#[tauri::command]
fn folder_exists(path: PathBuf) -> bool {
    path.is_dir()
}

/// The folders that "pular_recriaveis" leaves out, for the profile screen.
#[tauri::command]
fn rebuildable_rules() -> &'static [Rule] {
    REBUILDABLE
}

/// The names that "pular_temporarios" leaves out, for the profile screen.
#[tauri::command]
fn temporary_names() -> &'static [&'static str] {
    TEMPORARY
}

fn settings_path() -> Result<PathBuf, String> {
    settings::default_path()
        .ok_or_else(|| "a pasta de configuração do usuário não foi encontrada".into())
}

#[tauri::command]
fn get_settings() -> Result<Settings, String> {
    settings::load(&settings_path()?).map_err(|err| err.to_string())
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings::save(&settings_path()?, &settings).map_err(|err| err.to_string())
}

/// Sends a short question with the settings on screen (saved or not).
#[tauri::command]
async fn test_ai(settings: Settings) -> Result<String, String> {
    let (provider, key, model) = settings
        .active()
        .ok_or("escolha um serviço de IA e escreva a chave")?;
    ai::test(provider, &key, &model).await
}

/// Asks the AI for names to skip in the profile's sources. The user decides.
#[tauri::command]
async fn suggest_skips(profile: Profile) -> Result<Vec<ai::Suggestion>, String> {
    let settings = settings::load(&settings_path()?).map_err(|err| err.to_string())?;
    let (provider, key, model) = settings
        .active()
        .ok_or("configure um serviço de IA na engrenagem, no alto da tela")?;
    if profile.sources.is_empty() {
        return Err("adicione pelo menos uma pasta de origem".into());
    }
    ai::suggest(
        provider,
        key,
        model,
        profile.sources.clone(),
        profile.exclusions(),
        profile.skip_names.clone(),
    )
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(RunState::default())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            save_profiles,
            run_profile,
            cancel_run,
            resolve_conflicts,
            list_reports,
            open_report,
            open_folder,
            folder_exists,
            rebuildable_rules,
            temporary_names,
            get_settings,
            save_settings,
            test_ai,
            suggest_skips,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
