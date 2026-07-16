use crate::database::models::{AppSettings, AppStatus, BackgroundTask, PlatformSettings, SubAccount, User, ValveBotConfig, ValveBotConfigUpdate};
use crate::database::repo;
use crate::AppState;
use std::path::{Path, PathBuf};
use serde_json::{json, Value};
use tauri::State;

fn validate_platform(platform: &str) -> Result<(), String> {
    match platform {
        "valve" | "renown" | "5eplay" => Ok(()),
        _ => Err("不支持的平台".into()),
    }
}

fn validate_status(status: &str) -> Result<(), String> {
    match status {
        "active" | "disabled" | "error" => Ok(()),
        _ => Err("不支持的账号状态".into()),
    }
}

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> AppStatus {
    AppStatus {
        sidecar_running: state.sidecar.is_running(),
        orchestrator_running: true,
        queue_paused: state.orchestrator.is_paused(),
        data_root: state.db.lock().get_data_root(),
        last_discover_minutes_ago: state.orchestrator.minutes_since_discover(),
        background_task: state.orchestrator.active_task(),
    }
}

#[tauri::command]
pub fn get_background_task(state: State<'_, AppState>) -> Option<BackgroundTask> {
    state.orchestrator.active_task()
}

#[tauri::command]
pub fn get_background_task_logs(state: State<'_, AppState>) -> Vec<String> {
    state.orchestrator.task_logs()
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
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err("显示名不能为空".into());
    }
    let note = note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    repo::create_user(&state.db.lock(), display_name, note).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_user(
    state: State<'_, AppState>,
    id: String,
    display_name: String,
    note: Option<String>,
) -> Result<(), String> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err("显示名不能为空".into());
    }
    let note = note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    repo::update_user(&state.db.lock(), &id, display_name, note).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_user(state: State<'_, AppState>, id: String) -> Result<(), String> {
    repo::delete_user(&state.db.lock(), &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_sub_accounts(state: State<'_, AppState>, platform: String) -> Result<Vec<SubAccount>, String> {
    let platform = platform.trim();
    validate_platform(platform)?;
    repo::list_sub_accounts(&state.db.lock(), platform).map_err(|e| e.to_string())
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
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Err("请选择所属用户".into());
    }
    let platform = platform.trim();
    validate_platform(platform)?;
    let external_id = external_id.trim();
    if external_id.is_empty() {
        return Err("External ID 不能为空".into());
    }
    let display_name = display_name.trim();
    let display_name = if display_name.is_empty() {
        external_id
    } else {
        display_name
    };
    let mut meta = metadata;
    let external_id = if platform == "5eplay" {
        let resolved = state
            .sidecar
            .call(
                "resolve_5eplay_player_id",
                json!({ "input": external_id }),
            )
            .map_err(|e| e.to_string())?;
        resolved
            .get("uuid")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "无法解析 5EPlay 玩家 ID".to_string())?
    } else {
        external_id.to_string()
    };
    if platform == "valve" {
        let channel = valve_channel
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("authcode");
        if channel != "authcode" {
            return Err("Valve Local 暂未实现，请使用 Auth Code".into());
        }
        let auth_code = meta
            .get("auth_code")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "Auth Code 不能为空".to_string())?;
        let oldest_share_code = meta
            .get("oldest_share_code")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "Oldest Share Code 不能为空".to_string())?;
        meta["source"] = Value::String("auth_code".into());
        meta["auth_code"] = Value::String(auth_code);
        meta["oldest_share_code"] = Value::String(oldest_share_code);
        meta["valve_channel"] = Value::String(channel.to_string());
    }
    let account = repo::add_sub_account(
        &state.db.lock(),
        user_id,
        platform,
        &external_id,
        display_name,
        meta,
    )
    .map_err(|e| e.to_string())?;
    let _ = state.db.lock().conn.execute(
        "UPDATE platform_settings SET enabled = 1, updated_at = datetime('now') WHERE platform = ?",
        rusqlite::params![platform],
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
    let status = status.trim();
    validate_status(status)?;
    repo::set_sub_account_status(&state.db.lock(), &id, status).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_sub_account(state: State<'_, AppState>, id: String) -> Result<(), String> {
    repo::delete_sub_account(&state.db.lock(), &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_sub_account_meta(
    state: State<'_, AppState>,
    id: String,
    display_name: String,
    metadata: Value,
) -> Result<(), String> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err("显示名不能为空".into());
    }
    repo::update_sub_account_meta(&state.db.lock(), &id, display_name, metadata)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_matches(state: State<'_, AppState>) -> Result<Vec<crate::database::models::Match>, String> {
    repo::list_matches(&state.db.lock()).map_err(|e| e.to_string())
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
    validate_platform(platform.platform.as_str())?;
    repo::update_platform_settings(&state.db.lock(), &platform).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_valve_bot_config(state: State<'_, AppState>) -> Result<ValveBotConfig, String> {
    repo::get_valve_bot_config(&state.db.lock()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_valve_bot_config(
    state: State<'_, AppState>,
    config: ValveBotConfigUpdate,
) -> Result<(), String> {
    repo::update_valve_bot_config(&state.db.lock(), &config).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn discover_now(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if state.orchestrator.start_discover_background(app, "发现 Demo") {
        Ok(())
    } else {
        Err("已有后台任务正在运行".into())
    }
}

#[tauri::command]
pub fn toggle_queue_pause(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let paused = state.orchestrator.toggle_pause();
    if !paused {
        let _ = state
            .orchestrator
            .start_queue_background(app, "恢复下载队列");
    }
    Ok(paused)
}

#[tauri::command]
pub fn retry_download_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    if state.orchestrator.start_retry_background(job_id, app) {
        Ok(())
    } else {
        Err("已有后台任务正在运行".into())
    }
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
pub fn import_demos(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    file_paths: Vec<String>,
) -> Result<(), String> {
    if file_paths.is_empty() {
        return Ok(());
    }
    if !state.orchestrator.start_import_background(file_paths, app) {
        Err("已有后台任务正在运行，请稍后再导入".into())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub fn refresh_match_metadata(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.orchestrator.start_refresh_metadata_background(app) {
        Ok(())
    } else {
        Err("已有后台任务正在运行，请稍后再试".into())
    }
}

#[tauri::command]
pub fn force_download_now(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    repo::requeue_all_failed(&state.db.lock()).map_err(|e| e.to_string())?;
    if state.orchestrator.is_paused() {
        state.orchestrator.toggle_pause();
    }
    if !state.orchestrator.start_queue_background(app, "立即下载") {
        return Err("已有后台任务正在运行".into());
    }
    Ok(())
}

#[tauri::command]
pub fn open_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let resolved = resolve_data_path(&state, Path::new(&path))?;
    open::that(resolved).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_data_subdirectory(state: State<'_, AppState>, subdir: String) -> Result<(), String> {
    let root = state.db.lock().data_root.clone();
    let path = if subdir.is_empty() {
        root
    } else {
        root.join(subdir)
    };
    open::that(path).map_err(|e| e.to_string())
}

fn resolve_data_path(state: &AppState, path: &Path) -> Result<PathBuf, String> {
    let root = state.db.lock().data_root.clone();
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let relative = path
            .to_string_lossy()
            .strip_prefix("./")
            .map(str::to_string)
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        root.join(relative)
    };
    let canonical_root = root.canonicalize().unwrap_or(root);
    let canonical_path = resolved.canonicalize().unwrap_or(resolved);
    if !canonical_path.starts_with(&canonical_root) {
        return Err("只能打开数据目录内的路径".into());
    }
    Ok(canonical_path)
}
