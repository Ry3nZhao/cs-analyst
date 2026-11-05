//! 存储与数据的 Tauri 命令。JSON 结构见 src/shared-types/src/storage.ts。
//!
//! 耗时操作（批量压缩 / 删除、导出、备份、恢复）是 async 命令，在阻塞线程池里执行，进度通过
//! `storage_progress` 事件推送。不可逆操作必须带预览下发的令牌（`apply_storage_plan`）或
//! 显式确认（`restore_backup` 的 `confirm`）。

use super::backup::{self, BackupInfo, RestoreResult};
use super::cleanup::{self, CleanupRules, Plan, PlanResult};
use super::export::{self, ExportResult, Format};
use super::usage::{self, StorageUsage};
use super::{DemoAction, MatchStorageInfo, StorageSettings};
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

type R<T> = Result<T, String>;

fn e(err: crate::database::DbError) -> String {
    err.to_string()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    op: &'a str,
    done: usize,
    total: usize,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> R<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_storage_settings(state: State<'_, AppState>) -> R<StorageSettings> {
    super::get_settings(&state.db.lock()).map_err(e)
}

#[tauri::command]
pub fn update_storage_settings(state: State<'_, AppState>, settings: StorageSettings) -> R<StorageSettings> {
    let db = state.db.lock();
    super::update_settings(&db, &settings).map_err(e)?;
    super::get_settings(&db).map_err(e)
}

#[tauri::command]
pub async fn get_storage_usage(state: State<'_, AppState>) -> R<StorageUsage> {
    let db = state.db.clone();
    blocking(move || usage::compute(&db.lock()).map_err(e)).await
}

#[tauri::command]
pub fn list_match_storage(state: State<'_, AppState>) -> R<Vec<MatchStorageInfo>> {
    super::list_match_storage(&state.db.lock()).map_err(e)
}

#[tauri::command]
pub fn set_match_favorite(state: State<'_, AppState>, ids: Vec<String>, favorite: bool) -> R<usize> {
    super::set_favorite(&state.db.lock(), &ids, favorite).map_err(e)
}

/// 预览：按规则清理（`kind = "cleanup"`，`rules` 省略时用已保存的规则），或对已有 demo 执行
/// 压缩 / 删除（`kind = "compress" | "delete"`，`ids` 省略表示全部）。
#[tauri::command]
pub async fn preview_storage_plan(
    state: State<'_, AppState>,
    kind: String,
    rules: Option<CleanupRules>,
    ids: Option<Vec<String>>,
) -> R<Plan> {
    let db = state.db.clone();
    blocking(move || {
        let db = db.lock();
        match kind.as_str() {
            "cleanup" => {
                let rules = match rules {
                    Some(r) => r,
                    None => {
                        let s = super::get_settings(&db).map_err(e)?;
                        CleanupRules { max_age_days: s.cleanup_max_age_days, max_total_gb: s.cleanup_max_total_gb }
                    }
                };
                if rules.max_age_days.filter(|d| *d > 0).is_none()
                    && rules.max_total_gb.filter(|g| *g > 0.0).is_none()
                {
                    return Err("请至少设置一条清理规则".into());
                }
                cleanup::preview_cleanup(&db, &rules).map_err(e)
            }
            "compress" => cleanup::preview_policy(&db, DemoAction::Compress, ids.as_deref()).map_err(e),
            "delete" => cleanup::preview_policy(&db, DemoAction::Delete, ids.as_deref()).map_err(e),
            other => Err(format!("未知操作: {other}")),
        }
    })
    .await
}

/// 执行预览过的计划。`token` 来自 `preview_storage_plan`，只能用一次。
#[tauri::command]
pub async fn apply_storage_plan(app: AppHandle, state: State<'_, AppState>, token: String) -> R<PlanResult> {
    let db = state.db.clone();
    blocking(move || {
        cleanup::apply_plan(&db, &token, &|done, total| {
            let _ = app.emit("storage_progress", Progress { op: "plan", done, total });
        })
    })
    .await
}

#[tauri::command]
pub async fn export_matches(state: State<'_, AppState>, ids: Option<Vec<String>>, format: String) -> R<ExportResult> {
    let db = state.db.clone();
    blocking(move || {
        let fmt = Format::parse(&format).ok_or_else(|| format!("未知格式: {format}"))?;
        let db = db.lock();
        let out = db.data_root.join("exports");
        export::export(&db, ids.as_deref(), fmt, &out).map_err(e)
    })
    .await
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>, include_replays: bool) -> R<BackupInfo> {
    let db = state.db.clone();
    blocking(move || {
        // 只为取路径短暂持锁；快照用独立的只读连接，不阻塞应用
        let data_root = db.lock().data_root.clone();
        backup::create_backup(&data_root, include_replays, "manual").map_err(e)
    })
    .await
}

#[tauri::command]
pub fn list_backups(state: State<'_, AppState>) -> R<Vec<BackupInfo>> {
    backup::list_backups(&state.db.lock()).map_err(e)
}

/// 恢复备份。`confirm` 必须为 `"RESTORE"`（前端二次确认后传入）。恢复前会自动备份当前库。
#[tauri::command]
pub async fn restore_backup(app: AppHandle, state: State<'_, AppState>, path: String, confirm: String) -> R<RestoreResult> {
    if confirm != "RESTORE" {
        return Err("恢复需要二次确认".into());
    }
    let db = state.db.clone();
    let result = blocking(move || {
        let mut guard = db.lock();
        backup::restore_backup(&mut guard, &PathBuf::from(path)).map_err(e)
    })
    .await?;
    let _ = app.emit("storage_restored", &result);
    Ok(result)
}

/// 只读校验备份（恢复前给用户看）。
#[tauri::command]
pub fn inspect_backup(state: State<'_, AppState>, path: String) -> R<i64> {
    backup::validate_backup(&state.db.lock(), &PathBuf::from(path)).map(|(_, _, v)| v).map_err(e)
}
