//! 集锦的 Tauri 命令。JSON 结构见 src/shared-types/src/highlights.ts。

use super::play::{self, LaunchResult, PlaybackInfo};
use super::repo::{self, HighlightFilters, HighlightList, HighlightSettings};
use crate::AppState;
use tauri::{AppHandle, State};

type R<T> = Result<T, String>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> R<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_highlights(state: State<'_, AppState>, filters: Option<HighlightFilters>) -> R<HighlightList> {
    repo::list_highlights(&state.db.lock(), &filters.unwrap_or_default())
}

#[tauri::command]
pub fn get_highlight_settings(state: State<'_, AppState>) -> R<HighlightSettings> {
    repo::get_settings(&state.db.lock()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_highlight_settings(state: State<'_, AppState>, settings: HighlightSettings) -> R<HighlightSettings> {
    let db = state.db.lock();
    repo::update_settings(&db, &settings).map_err(|e| e.to_string())?;
    repo::get_settings(&db).map_err(|e| e.to_string())
}

/// 重算集锦：`matchIds` 不传 = 所有已解析的对局。返回算出结果的对局数。
#[tauri::command]
pub async fn recompute_highlights(state: State<'_, AppState>, match_ids: Option<Vec<String>>) -> R<usize> {
    let db = state.db.clone();
    blocking(move || {
        let ids = match match_ids {
            Some(ids) => ids,
            None => {
                let g = db.lock();
                let mut st = g
                    .conn
                    .prepare("SELECT id FROM matches WHERE analysis_status = 'done'")
                    .map_err(|e| e.to_string())?;
                let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
                rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?
            }
        };
        let mut n = 0;
        for id in ids {
            if super::recompute(&db.lock(), &id)?.is_some() {
                n += 1;
            }
        }
        Ok(n)
    })
    .await
}

/// 播放前准备：定位 demo（压缩的先解压到临时目录），返回控制台命令。
#[tauri::command]
pub async fn prepare_highlight_playback(state: State<'_, AppState>, id: i64) -> R<PlaybackInfo> {
    let db = state.db.clone();
    blocking(move || play::prepare(&db, id)).await
}

/// 用 CS2 打开这条集锦的 demo（实验开关打开时会尝试自动跳转）。
#[tauri::command]
pub async fn launch_highlight_in_cs2(app: AppHandle, state: State<'_, AppState>, id: i64) -> R<LaunchResult> {
    let db = state.db.clone();
    blocking(move || play::launch(&db, Some(app), id)).await
}
