use crate::db::models::{AppSettings, AppStatus, PlatformSettings, SubAccount, User};
use crate::db::repo;
use crate::orchestrator::DownloadOrchestrator;
use crate::sidecar::SidecarManager;
use crate::AppState;
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> AppStatus {
    AppStatus {
        sidecar_running: state.sidecar.is_running(),
        orchestrator_running: true,
        queue_paused: state.orchestrator.is_paused(),
        data_root: state.db.lock().get_data_root(),
        last_discover_minutes_ago: state.orchestrator.minutes_since_discover(),
    }
}

#[tauri::command]
pub fn list_users(state: State<'_, AppState>) -> Result<Vec<User>, String> {
    repo::list_users(&state.db.lock()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_user(
    state: State<'_, AppState>,
    display_name: String,
    note: Option<String>,
) -> Result<User, String> {
    repo::create_user(&state.db.lock(), &display_name, note.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_user(
    state: State<'_, AppState>,
    id: String,
    display_name: String,
    note: Option<String>,
) -> Result<(), String> {
    repo::update_user(&state.db.lock(), &id, &display_name, note.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_user(state: State<'_, AppState>, id: String) -> Result<(), String> {
    repo::delete_user(&state.db.lock(), &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_sub_accounts(state: State<'_, AppState>, platform: String) -> Result<Vec<SubAccount>, String> {
    repo::list_sub_accounts(&state.db.lock(), &platform).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_sub_account(
    state: State<'_, AppState>,
    user_id: String,
    platform: String,
    external_id: String,
    display_name: String,
    metadata: Value,
    valve_channel: Option<String>,
) -> Result<SubAccount, String> {
    let mut meta = metadata;
    if let Some(ch) = valve_channel {
        meta["valve_channel"] = Value::String(ch);
    }
    let account = repo::add_sub_account(
        &state.db.lock(),
        &user_id,
        &platform,
        &external_id,
        &display_name,
        meta,
    )
    .map_err(|e| e.to_string())?;
    let _ = state.db.lock().conn.execute(
        "UPDATE platform_settings SET enabled = TRUE, updated_at = CURRENT_TIMESTAMP WHERE platform = ?",
        duckdb::params![platform],
    );
    Ok(account)
}

#[tauri::command]
pub fn update_sub_account(
    state: State<'_, AppState>,
    id: String,
    user_id: String,
) -> Result<(), String> {
    repo::update_sub_account_user(&state.db.lock(), &id, &user_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_sub_account_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<(), String> {
    repo::set_sub_account_status(&state.db.lock(), &id, &status).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_download_jobs(state: State<'_, AppState>) -> Result<Vec<crate::db::models::DownloadJob>, String> {
    repo::list_download_jobs(&state.db.lock()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    repo::get_settings(&state.db.lock()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    repo::update_settings(&state.db.lock(), &settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_platform_settings(state: State<'_, AppState>) -> Result<Vec<PlatformSettings>, String> {
    repo::list_platform_settings(&state.db.lock()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_platform_settings(
    state: State<'_, AppState>,
    platform: PlatformSettings,
) -> Result<(), String> {
    repo::update_platform_settings(&state.db.lock(), &platform).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn discover_now(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state
        .orchestrator
        .discover_all(&app)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_queue_pause(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.orchestrator.toggle_pause())
}

#[tauri::command]
pub fn retry_download_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    state
        .orchestrator
        .retry_job(&job_id, &app)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_download_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    state
        .orchestrator
        .cancel_job(&job_id, &app)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())
}
