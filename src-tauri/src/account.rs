use crate::backup;
use reqwest::{blocking::{multipart::{Form, Part}, Client}, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Mutex;

const API_BASE: &str = "https://klenua.com/api/v1";
const KEYRING_SERVICE: &str = "com.versionpilot.desktop";
const KEYRING_ACCOUNT: &str = "klenua-build-session";
const PERSISTENT_SESSION_SECONDS: i64 = 30 * 24 * 60 * 60;
const MAX_BACKUP_FILES: usize = 50;
const MAX_BACKUP_BYTES: u64 = 10 * 1024 * 1024;
const MAX_BACKUP_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Default)]
pub struct AccountState(pub Mutex<Option<AccountSession>>);

#[derive(Serialize, Deserialize, Clone)]
pub struct AccountSession {
    access_token: String,
    refresh_token: String,
    account: Account,
    persistent_until: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub synced_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProject {
    pub root: String,
    pub project_name: String,
    pub project_type: String,
    pub version: Option<String>,
    pub build: Option<String>,
    pub platforms: Vec<String>,
    pub source_of_truth: Option<String>,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

#[derive(Deserialize)]
struct UserData {
    display_name: String,
    email: String,
}

#[derive(Deserialize)]
struct LoginData {
    access_token: String,
    refresh_token: String,
    user: UserData,
}

#[derive(Deserialize)]
struct SyncRecord {
    local_id: String,
    synced_at: String,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<ApiError>,
}

fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("Could not initialize the klenua connection: {error}"))
}

fn api_error(status: StatusCode, body: &str) -> String {
    serde_json::from_str::<ApiResponse<Value>>(body)
        .ok()
        .and_then(|response| response.error.map(|error| error.message))
        .unwrap_or_else(|| format!("klenua.com returned {}.", status.as_u16()))
}

fn session_from_login(data: LoginData) -> AccountSession {
    AccountSession {
        access_token: data.access_token,
        refresh_token: data.refresh_token,
        account: Account { name: data.user.display_name, email: data.user.email },
        persistent_until: None,
    }
}

fn session_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|error| format!("Could not access the system keychain: {error}"))
}

fn forget_persisted_session() {
    if let Ok(entry) = session_entry() { let _ = entry.delete_credential(); }
}

fn persist_session(session: &AccountSession) -> Result<(), String> {
    if session.persistent_until.is_none_or(|until| until <= now()) {
        forget_persisted_session();
        return Ok(());
    }
    let serialized = serde_json::to_string(session)
        .map_err(|error| format!("Could not prepare the secure session: {error}"))?;
    session_entry()?.set_password(&serialized)
        .map_err(|error| format!("Could not save the session in the system keychain: {error}"))
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64).unwrap_or_default()
}

fn refresh(session: &AccountSession) -> Result<AccountSession, String> {
    let response = client()?
        .post(format!("{API_BASE}/auth/refresh"))
        .json(&json!({ "refresh_token": session.refresh_token }))
        .send()
        .map_err(|error| format!("Could not refresh your klenua session: {error}"))?;
    let status = response.status();
    let body = response.text().map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(api_error(status, &body));
    }
    let payload: ApiResponse<LoginData> = serde_json::from_str(&body)
        .map_err(|_| "klenua.com returned an invalid session response.".to_string())?;
    let mut refreshed = payload
        .data
        .filter(|_| payload.success)
        .map(session_from_login)
        .ok_or_else(|| "Could not refresh your klenua session.".to_string())?;
    refreshed.persistent_until = session.persistent_until;
    persist_session(&refreshed)?;
    Ok(refreshed)
}

pub fn login(email: String, password: String, stay_signed_in: bool, state: &AccountState) -> Result<Account, String> {
    if email.trim().is_empty() || password.is_empty() {
        return Err("Enter your klenua email address and password.".into());
    }
    let response = client()?
        .post(format!("{API_BASE}/auth/login"))
        .json(&json!({ "email": email.trim(), "password": password, "stay_signed_in": stay_signed_in, "client_name": "klenua build" }))
        .send()
        .map_err(|error| format!("Could not reach klenua.com: {error}"))?;
    let status = response.status();
    let body = response.text().map_err(|error| format!("Could not read the klenua response: {error}"))?;
    if !status.is_success() {
        return Err(api_error(status, &body));
    }
    let payload: ApiResponse<LoginData> = serde_json::from_str(&body)
        .map_err(|_| "klenua.com returned an invalid sign-in response.".to_string())?;
    let mut session = payload
        .data
        .filter(|_| payload.success)
        .map(session_from_login)
        .ok_or_else(|| "Could not sign in to klenua.com.".to_string())?;
    session.persistent_until = stay_signed_in.then(|| now() + PERSISTENT_SESSION_SECONDS);
    if stay_signed_in { persist_session(&session)?; } else { forget_persisted_session(); }
    let account = session.account.clone();
    *state.0.lock().map_err(|_| "Account session is unavailable.".to_string())? = Some(session);
    Ok(account)
}

pub fn current(state: &AccountState) -> Option<Account> {
    state.0.lock().ok().and_then(|session| session.as_ref().map(|account| account.account.clone()))
}

pub fn restore(state: &AccountState) -> Result<Option<Account>, String> {
    if let Some(account) = current(state) { return Ok(Some(account)); }
    let saved = match session_entry()?.get_password() {
        Ok(value) => value,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(error) => return Err(format!("Could not read the system keychain: {error}")),
    };
    let session: AccountSession = serde_json::from_str(&saved)
        .map_err(|_| "The saved klenua session could not be read. Please sign in again.".to_string())?;
    if session.persistent_until.is_none_or(|until| until <= now()) {
        forget_persisted_session();
        return Ok(None);
    }
    let account = session.account.clone();
    *state.0.lock().map_err(|_| "Account session is unavailable.".to_string())? = Some(session);
    Ok(Some(account))
}

pub fn logout(state: &AccountState) {
    let active_session = state.0.lock().ok().and_then(|session| session.clone());
    if let Some(session) = active_session {
        let _ = client().and_then(|client| {
            client.post(format!("{API_BASE}/auth/logout"))
                .json(&json!({ "refresh_token": session.refresh_token }))
                .send()
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
    }
    if let Ok(mut session) = state.0.lock() { *session = None; }
    forget_persisted_session();
}

fn send_sync(session: &AccountSession, project: &Value) -> Result<(StatusCode, String), String> {
    let response = client()?
        .post(format!("{API_BASE}/build/projects"))
        .bearer_auth(&session.access_token)
        .json(project)
        .send()
        .map_err(|error| format!("Could not sync to klenua.com: {error}"))?;
    let status = response.status();
    let body = response.text().map_err(|error| format!("Could not read the klenua response: {error}"))?;
    Ok((status, body))
}

fn fetch_sync_records(session: &AccountSession) -> Result<(StatusCode, String), String> {
    let response = client()?
        .get(format!("{API_BASE}/build/projects"))
        .bearer_auth(&session.access_token)
        .send()
        .map_err(|error| format!("Could not check sync status: {error}"))?;
    let status = response.status();
    let body = response.text().map_err(|error| format!("Could not read the klenua response: {error}"))?;
    Ok((status, body))
}

fn send_backup(
    session: &AccountSession,
    local_id: &str,
    relative_path: &str,
    file: &std::path::Path,
) -> Result<(StatusCode, String), String> {
    let bytes = std::fs::read(file)
        .map_err(|error| format!("Could not read backup file {}: {error}", file.display()))?;
    let file_name = file.file_name().and_then(|name| name.to_str()).unwrap_or("backup.txt");
    let form = Form::new()
        .text("local_id", local_id.to_owned())
        .text("relative_path", relative_path.to_owned())
        .part("backup", Part::bytes(bytes).file_name(file_name.to_owned()));
    let response = client()?
        .post(format!("{API_BASE}/build/projects/backups"))
        .bearer_auth(&session.access_token)
        .multipart(form)
        .send()
        .map_err(|error| format!("Could not upload backup files to klenua.com: {error}"))?;
    let status = response.status();
    let body = response.text().map_err(|error| format!("Could not read the klenua response: {error}"))?;
    Ok((status, body))
}

pub fn sync(scan: SyncProject, state: &AccountState) -> Result<String, String> {
    let session = state.0.lock().map_err(|_| "Account session is unavailable.".to_string())?
        .clone().ok_or_else(|| "Sign in to your klenua account before syncing.".to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(scan.root.as_bytes());
    let project = json!({
        "local_id": format!("{:x}", hasher.finalize()),
        "project_name": scan.project_name,
        "project_type": scan.project_type,
        "version": scan.version,
        "build": scan.build,
        "platforms": scan.platforms,
        "source_of_truth": scan.source_of_truth,
        "app_name": scan.app_name,
        "bundle_id": scan.bundle_id,
    });
    let (mut status, mut body) = send_sync(&session, &project)?;
    if status == StatusCode::UNAUTHORIZED {
        let refreshed = refresh(&session)?;
        (status, body) = send_sync(&refreshed, &project)?;
        if status.is_success() {
            if let Ok(mut stored) = state.0.lock() { *stored = Some(refreshed); }
        }
    }
    if !status.is_success() { return Err(api_error(status, &body)); }
    let payload: ApiResponse<Value> = serde_json::from_str(&body)
        .map_err(|_| "klenua.com returned an invalid sync response.".to_string())?;
    if !payload.success { return Err(payload.error.map(|error| error.message).unwrap_or_else(|| "Could not sync the project.".into())); }
    Ok(payload.data.and_then(|data| data.get("message").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "Project synced to your klenua account.".into()))
}

pub fn sync_status(root: String, state: &AccountState) -> Result<Option<SyncStatus>, String> {
    let session = state.0.lock().map_err(|_| "Account session is unavailable.".to_string())?
        .clone().ok_or_else(|| "Sign in to your klenua account before checking sync status.".to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(root.as_bytes());
    let local_id = format!("{:x}", hasher.finalize());
    let (mut status, mut body) = fetch_sync_records(&session)?;
    if status == StatusCode::UNAUTHORIZED {
        let refreshed = refresh(&session)?;
        (status, body) = fetch_sync_records(&refreshed)?;
        if status.is_success() {
            if let Ok(mut stored) = state.0.lock() { *stored = Some(refreshed); }
        }
    }
    if !status.is_success() { return Err(api_error(status, &body)); }
    let payload: ApiResponse<Vec<SyncRecord>> = serde_json::from_str(&body)
        .map_err(|_| "klenua.com returned an invalid sync status response.".to_string())?;
    if !payload.success { return Err(payload.error.map(|error| error.message).unwrap_or_else(|| "Could not check sync status.".into())); }
    Ok(payload.data.unwrap_or_default().into_iter().find(|record| record.local_id == local_id)
        .map(|record| SyncStatus { synced_at: record.synced_at }))
}

pub fn upload_backups(root: String, state: &AccountState) -> Result<String, String> {
    let session = state.0.lock().map_err(|_| "Account session is unavailable.".to_string())?
        .clone().ok_or_else(|| "Sign in to your klenua account before uploading backups.".to_string())?;
    let files = backup::latest_files(std::path::Path::new(&root))?;
    if files.len() > MAX_BACKUP_FILES {
        return Err(format!("Backup upload is limited to {MAX_BACKUP_FILES} files at a time."));
    }
    let mut total_bytes = 0u64;
    for (_, file) in &files {
        let size = std::fs::metadata(file).map_err(|error| error.to_string())?.len();
        if size > MAX_BACKUP_FILE_BYTES {
            return Err("Each backup file must be 1 MB or smaller before upload.".into());
        }
        total_bytes = total_bytes.saturating_add(size);
    }
    if total_bytes > MAX_BACKUP_BYTES {
        return Err("Backup uploads are limited to 10 MB per Apply Changes action.".into());
    }
    let mut hasher = Sha256::new();
    hasher.update(root.as_bytes());
    let local_id = format!("{:x}", hasher.finalize());
    let mut active_session = session;
    for (relative_path, file) in &files {
        let (mut status, mut body) = send_backup(&active_session, &local_id, relative_path, file)?;
        if status == StatusCode::UNAUTHORIZED {
            active_session = refresh(&active_session)?;
            (status, body) = send_backup(&active_session, &local_id, relative_path, file)?;
        }
        if !status.is_success() {
            return Err(api_error(status, &body));
        }
        let payload: ApiResponse<Value> = serde_json::from_str(&body)
            .map_err(|_| "klenua.com returned an invalid backup upload response.".to_string())?;
        if !payload.success {
            return Err(payload.error.map(|error| error.message).unwrap_or_else(|| "Could not upload backup files.".into()));
        }
    }
    if let Ok(mut stored) = state.0.lock() { *stored = Some(active_session); }
    Ok(format!("Uploaded {} backup file{} to klenua.", files.len(), if files.len() == 1 { "" } else { "s" }))
}
