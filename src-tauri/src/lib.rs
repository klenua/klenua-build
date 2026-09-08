mod account;
mod backup;
mod git;
mod scanner;
mod writers;

use account::{Account, AccountState, SyncProject, SyncStatus};
use scanner::{AppSettings, ProjectScan, RecentProject};
use std::path::Path;
use writers::Preview;

#[tauri::command]
fn scan_project(path: String, app: tauri::AppHandle) -> Result<ProjectScan, String> {
    let root = Path::new(&path);
    let scan = scanner::scan(root)?;
    if scanner::load_settings(&app)?.save_projects {
        scanner::store_recent(&app, root)?;
    }
    Ok(scan)
}

#[tauri::command]
fn open_project_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Open project folder")
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
fn preview_changes(path: String, version: String, build: String) -> Result<Preview, String> {
    writers::preview(Path::new(&path), &version, &build)
}

#[tauri::command]
fn apply_changes(
    path: String,
    version: String,
    build: String,
    remember: bool,
) -> Result<String, String> {
    let root = Path::new(&path);
    let preview = writers::preview(root, &version, &build)?;
    if preview.changes.is_empty() {
        return Err("No safe changes were found. Manual review required.".into());
    }
    writers::apply(root, preview)?;
    if remember {
        scanner::save_project_config(root)?;
    }
    Ok("Changes applied safely. A backup is available through Restore Last Change.".into())
}

#[tauri::command]
fn restore_last_change(path: String) -> Result<String, String> {
    backup::restore(Path::new(&path))
}

#[tauri::command]
fn get_recent_projects(app: tauri::AppHandle) -> Result<Vec<RecentProject>, String> {
    scanner::read_recents(&app)
}

#[tauri::command]
fn remove_recent_project(path: String, app: tauri::AppHandle) -> Result<(), String> {
    scanner::remove_recent(&app, &path)
}

#[tauri::command]
fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    scanner::load_settings(&app)
}

#[tauri::command]
fn save_app_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    scanner::save_settings(&app, &settings)
}

#[tauri::command]
fn login_klenua(email: String, password: String, stay_signed_in: bool, state: tauri::State<AccountState>) -> Result<Account, String> {
    account::login(email, password, stay_signed_in, &state)
}

#[tauri::command]
fn get_klenua_account(state: tauri::State<AccountState>) -> Result<Option<Account>, String> {
    account::restore(&state)
}

#[tauri::command]
fn logout_klenua(state: tauri::State<AccountState>) {
    account::logout(&state);
}

#[tauri::command]
fn sync_klenua_project(project: SyncProject, state: tauri::State<AccountState>) -> Result<String, String> {
    account::sync(project, &state)
}

#[tauri::command]
fn get_klenua_sync_status(root: String, state: tauri::State<AccountState>) -> Result<Option<SyncStatus>, String> {
    account::sync_status(root, &state)
}

#[tauri::command]
fn upload_klenua_backups(root: String, state: tauri::State<AccountState>) -> Result<String, String> {
    account::upload_backups(root, &state)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AccountState::default())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            open_project_folder,
            scan_project,
            preview_changes,
            apply_changes,
            restore_last_change,
            get_recent_projects,
            remove_recent_project,
            get_app_settings,
            save_app_settings,
            login_klenua,
            get_klenua_account,
            logout_klenua,
            sync_klenua_project,
            get_klenua_sync_status,
            upload_klenua_backups
        ])
        .run(tauri::generate_context!())
        .expect("error while running klenua build");
}
