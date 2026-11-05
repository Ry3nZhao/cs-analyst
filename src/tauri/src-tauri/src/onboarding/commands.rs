//! W3-C 的 Tauri 命令。

use super::main_player::{self, MainPlayer, PlayerCandidate};
use super::{data_root, settings};
use crate::watch::{self, ReplayWatcher, WatchStatus};
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

fn err(e: impl ToString) -> String {
    e.to_string()
}

#[tauri::command]
pub fn get_main_player(state: State<'_, AppState>) -> Result<MainPlayer, String> {
    main_player::get(&state.db.lock().conn).map_err(err)
}

/// `steamid = None` 清除主玩家。接受 Steam64 / 个人主页链接 / `[U:1:n]` / `STEAM_X:Y:Z`。
/// 成功后广播 `main_player_changed`（载荷同返回值）。
#[tauri::command]
pub fn set_main_player(app: AppHandle, state: State<'_, AppState>, steamid: Option<String>) -> Result<MainPlayer, String> {
    let parsed = match steamid.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(main_player::parse_steam_input(s).ok_or("不是有效的 SteamID64")?),
        None => None,
    };
    let player = {
        let db = state.db.lock();
        main_player::set_steamid(&db.conn, parsed).map_err(err)?;
        main_player::get(&db.conn).map_err(err)?
    };
    let _ = app.emit("main_player_changed", &player);
    Ok(player)
}

#[tauri::command]
pub fn list_main_player_candidates(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<PlayerCandidate>, String> {
    main_player::list_candidates(&state.db.lock().conn, query.as_deref(), limit.unwrap_or(30).clamp(1, 200)).map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingState {
    /// 应该自动进入向导：没走过向导，且没有主玩家、账号、对局
    pub needs_onboarding: bool,
    pub completed: bool,
    pub main_player_set: bool,
    pub account_count: i64,
    pub match_count: i64,
    pub analyzed_count: i64,
    pub data_root: String,
    pub default_data_root: String,
    /// 由环境变量指定，不能在应用里改
    pub data_root_locked: bool,
}

#[tauri::command]
pub fn get_onboarding_state(app: AppHandle, state: State<'_, AppState>) -> Result<OnboardingState, String> {
    let db = state.db.lock();
    let c = &db.conn;
    let completed = settings::onboarding_completed(c).map_err(err)?;
    let main_player_set = main_player::get_steamid(c).map_err(err)?.is_some();
    let account_count: i64 = c.query_row("SELECT COUNT(*) FROM sub_accounts", [], |r| r.get(0)).map_err(err)?;
    let match_count: i64 = c.query_row("SELECT COUNT(*) FROM matches", [], |r| r.get(0)).map_err(err)?;
    let analyzed_count: i64 = c
        .query_row("SELECT COUNT(*) FROM matches WHERE analysis_status = 'done'", [], |r| r.get(0))
        .map_err(err)?;
    Ok(OnboardingState {
        needs_onboarding: !completed && !main_player_set && account_count == 0 && match_count == 0,
        completed,
        main_player_set,
        account_count,
        match_count,
        analyzed_count,
        data_root: db.get_data_root(),
        default_data_root: data_root::default_root(&app).to_string_lossy().to_string(),
        data_root_locked: data_root::env_override().is_some(),
    })
}

#[tauri::command]
pub fn complete_onboarding(state: State<'_, AppState>) -> Result<(), String> {
    settings::set_onboarding_completed(&state.db.lock().conn, true).map_err(err)
}

/// 切换数据目录（`None` = 恢复默认），然后重启应用。旧目录的数据不会搬走。
#[tauri::command]
pub fn set_data_root(app: AppHandle, path: Option<String>) -> Result<(), String> {
    let target = path.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(PathBuf::from);
    if let Some(t) = &target {
        if !t.is_absolute() {
            return Err("请选择一个完整路径".into());
        }
    }
    data_root::write_pointer(&app, target.as_deref())?;
    app.restart();
}

#[tauri::command]
pub fn get_experience_settings(state: State<'_, AppState>) -> Result<settings::ExperienceSettings, String> {
    settings::get(&state.db.lock().conn).map_err(err)
}

#[tauri::command]
pub fn update_experience_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    watcher: State<'_, Arc<ReplayWatcher>>,
    settings: settings::ExperienceSettings,
) -> Result<WatchStatus, String> {
    if let Some(dir) = settings.replays_dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !PathBuf::from(dir).is_dir() {
            return Err("目录不存在".into());
        }
    }
    super::settings::update(&state.db.lock().conn, &settings).map_err(err)?;
    watcher.reconfigure(&app);
    Ok(watcher.status())
}

#[tauri::command]
pub fn get_watch_status(watcher: State<'_, Arc<ReplayWatcher>>) -> WatchStatus {
    watcher.status()
}

#[tauri::command]
pub fn detect_replays_dir() -> Option<String> {
    watch::detect::detect_replays_dir().map(|p| p.to_string_lossy().to_string())
}

/// 设置页「发一条测试通知」
#[tauri::command]
pub fn send_test_notification(app: AppHandle) {
    super::notify::show(&app, "CS Analyst", "通知已开启：新对局解析完成时会在这里提醒你", None);
}
