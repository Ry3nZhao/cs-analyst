//! 上手体验相关的设置（迁移 008 在 app_settings 里加的列）。

use crate::database::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// 托盘 / 通知 / 监听目录开关。主玩家单独由 `main_player` 管理。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceSettings {
    pub close_to_tray: bool,
    pub notify_new_match: bool,
    pub watch_replays: bool,
    /// 手动指定的 replays 目录；None = 自动检测
    pub replays_dir: Option<String>,
}

impl Default for ExperienceSettings {
    fn default() -> Self {
        Self { close_to_tray: true, notify_new_match: true, watch_replays: true, replays_dir: None }
    }
}

pub fn get(c: &Connection) -> Result<ExperienceSettings> {
    Ok(c.query_row(
        "SELECT close_to_tray, notify_new_match, watch_replays, replays_dir FROM app_settings WHERE id = 1",
        [],
        |r| {
            Ok(ExperienceSettings {
                close_to_tray: r.get(0)?,
                notify_new_match: r.get(1)?,
                watch_replays: r.get(2)?,
                replays_dir: r.get(3)?,
            })
        },
    )?)
}

pub fn update(c: &Connection, s: &ExperienceSettings) -> Result<()> {
    let dir = s.replays_dir.as_deref().map(str::trim).filter(|d| !d.is_empty());
    c.execute(
        "UPDATE app_settings SET close_to_tray = ?1, notify_new_match = ?2, watch_replays = ?3, replays_dir = ?4,
           updated_at = datetime('now') WHERE id = 1",
        params![s.close_to_tray, s.notify_new_match, s.watch_replays, dir],
    )?;
    Ok(())
}

pub fn onboarding_completed(c: &Connection) -> Result<bool> {
    Ok(c.query_row(
        "SELECT onboarding_completed_at IS NOT NULL FROM app_settings WHERE id = 1",
        [],
        |r| r.get(0),
    )?)
}

pub fn set_onboarding_completed(c: &Connection, done: bool) -> Result<()> {
    c.execute(
        "UPDATE app_settings SET onboarding_completed_at = CASE WHEN ?1 THEN datetime('now') ELSE NULL END WHERE id = 1",
        params![done],
    )?;
    Ok(())
}
