//! 新对局解析完成时弹系统通知，点击打开 `/matches/<id>`。
//!
//! 监听解析队列已有的 `analysis_updated` 事件（status = done），不改解析队列。
//! 每场只通知一次（表 `match_notifications`，迁移 008 把存量对局都标为已通知）。

use super::settings;
use crate::database::Result;
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::{AppHandle, Listener, Manager};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisUpdated {
    match_id: String,
    status: String,
}

fn log(msg: impl AsRef<str>) {
    eprintln!("[notify] {}", msg.as_ref());
}

pub fn start(app: &AppHandle) {
    let handle = app.clone();
    app.listen_any("analysis_updated", move |event| {
        let Ok(payload) = serde_json::from_str::<AnalysisUpdated>(event.payload()) else { return };
        if payload.status != "done" {
            return;
        }
        let handle = handle.clone();
        // 不阻塞事件线程
        std::thread::spawn(move || on_match_done(&handle, &payload.match_id));
    });
}

fn on_match_done(app: &AppHandle, match_id: &str) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let (enabled, text) = {
        let db = state.db.lock();
        let enabled = settings::get(&db.conn).map(|s| s.notify_new_match).unwrap_or(false);
        // 不管开没开都标记，避免关掉再打开时补弹一堆旧对局
        let first = mark_notified(&db.conn, match_id).unwrap_or(false);
        let text = if first { summary(&db.conn, match_id).ok().flatten() } else { None };
        (enabled, text)
    };
    if let (true, Some(body)) = (enabled, text) {
        show(app, "新对局已解析", &body, Some(format!("/matches/{match_id}")));
    }
}

/// 第一次标记时返回 true。
pub fn mark_notified(c: &Connection, match_id: &str) -> Result<bool> {
    Ok(c.execute("INSERT OR IGNORE INTO match_notifications (match_id) VALUES (?)", params![match_id])? > 0)
}

/// 「de_dust2 13:9」。有主玩家且在场时按主玩家视角写比分并附上胜负。
pub fn summary(c: &Connection, match_id: &str) -> Result<Option<String>> {
    let map: Option<Option<String>> = c
        .query_row("SELECT map_name FROM matches WHERE id = ?", params![match_id], |r| r.get(0))
        .optional()?;
    let Some(map) = map else { return Ok(None) };
    let map = map.filter(|m| !m.is_empty()).unwrap_or_else(|| "未知地图".into());

    let me = super::main_player::get_steamid(c)?;
    let mine: Option<(i64, i64, i64)> = match me {
        Some(id) => c
            .query_row(
                "SELECT rounds_won, rounds_lost, result FROM h2h_player_results WHERE match_id = ? AND steamid = ?",
                params![match_id, id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?,
        None => None,
    };
    if let Some((won, lost, result)) = mine {
        let verdict = match result {
            1 => "胜",
            -1 => "负",
            _ => "平",
        };
        return Ok(Some(format!("{map} {won}:{lost} · {verdict}")));
    }

    let mut st = c.prepare("SELECT team, MAX(score) FROM team_round_stats WHERE match_id = ? GROUP BY team ORDER BY team")?;
    let scores: Vec<i64> = st
        .query_map(params![match_id], |r| r.get::<_, i64>(1))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(match scores.as_slice() {
        [a, b] => format!("{map} {a}:{b}"),
        _ => map,
    }))
}

/// 显示窗口并跳转到前端路由。
pub fn open_route(app: &AppHandle, route: &str) {
    super::tray::show_main_window(app);
    use tauri::Emitter;
    let _ = app.emit("app_navigate", route);
}

#[cfg(windows)]
pub fn show(app: &AppHandle, title: &str, body: &str, route: Option<String>) {
    use tauri_winrt_notification::Toast;
    // 与 tauri-plugin-notification 相同：未安装（target/debug|release 下运行）时借用 PowerShell 的 AppUserModelID
    let app_id = if running_from_target_dir() {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    };
    let handle = app.clone();
    let result = Toast::new(&app_id)
        .title(title)
        .text1(body)
        .on_activated(move |_| {
            if let Some(route) = &route {
                open_route(&handle, route);
            } else {
                super::tray::show_main_window(&handle);
            }
            Ok(())
        })
        .show();
    match result {
        Ok(()) => log(format!("shown: {body}")),
        Err(e) => {
            log(format!("winrt toast failed ({e}), falling back to plugin"));
            plugin_fallback(app, title, body);
        }
    }
}

#[cfg(not(windows))]
pub fn show(app: &AppHandle, title: &str, body: &str, _route: Option<String>) {
    plugin_fallback(app, title, body);
}

fn plugin_fallback(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log(format!("plugin notification failed: {e}"));
    }
}

#[cfg(windows)]
fn running_from_target_dir() -> bool {
    let Ok(exe) = std::env::current_exe() else { return true };
    let Some(dir) = exe.parent() else { return true };
    let s = dir.to_string_lossy().to_lowercase().replace('/', "\\");
    s.ends_with("\\target\\debug") || s.ends_with("\\target\\release")
}
