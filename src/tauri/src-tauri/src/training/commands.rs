//! 训练命令。JSON 结构见 `src/shared-types/src/training.ts`。

use super::cs2cfg::{self, Cs2SensitivityDetect};
use super::repo::{self, SessionInput, SessionRow, TrialRow};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn save_training_result(state: State<'_, AppState>, result: SessionInput) -> Result<String, String> {
    let db = state.db.lock();
    repo::save_session(&db, &result).map_err(|e| e.to_string())
}

/// `range`：`7d` / `30d` / `90d` / `365d` / `all`。`variant` 只对压枪练习有意义（武器名）。
#[tauri::command]
pub fn list_training_results(state: State<'_, AppState>, mode: String, range: Option<String>, variant: Option<String>) -> Result<Vec<SessionRow>, String> {
    let db = state.db.lock();
    repo::list_sessions(&db, &mode, variant.as_deref(), range.as_deref(), chrono::Utc::now()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_training_trials(state: State<'_, AppState>, session_id: String) -> Result<Vec<TrialRow>, String> {
    let db = state.db.lock();
    repo::get_trials(&db, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_training_result(state: State<'_, AppState>, session_id: String) -> Result<bool, String> {
    let db = state.db.lock();
    repo::delete_session(&db, &session_id).map_err(|e| e.to_string())
}

/// 自动查找本机 CS2 配置（不需要数据库）。
#[tauri::command]
pub async fn detect_cs2_sensitivity() -> Result<Cs2SensitivityDetect, String> {
    tauri::async_runtime::spawn_blocking(cs2cfg::detect).await.map_err(|e| e.to_string())
}

/// 用户手动选择的 vcfg / cfg 文件。
#[tauri::command]
pub fn read_cs2_sensitivity(path: String) -> Result<f64, String> {
    cs2cfg::read_file(std::path::Path::new(&path))
}
