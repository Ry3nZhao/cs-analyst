//! 设置页「下载模块」区块用到的命令。

use super::DownloaderStatus;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_downloader_status(state: State<'_, AppState>) -> DownloaderStatus {
    state.sidecar.status()
}

/// 手动重启（也用于从「连续崩溃已停止」或「协议不兼容」状态恢复）。
#[tauri::command]
pub async fn restart_downloader(state: State<'_, AppState>) -> Result<DownloaderStatus, String> {
    let sidecar = state.sidecar.clone();
    let orchestrator = state.orchestrator.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = sidecar.restart();
        // 重启后按数据库重新下发 adapter 开关
        orchestrator.sync_adapter_switches();
        result.map(|_| sidecar.status()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_downloader_logs(state: State<'_, AppState>, limit: Option<usize>) -> Vec<String> {
    state.sidecar.logs(limit.unwrap_or(200).min(500))
}
