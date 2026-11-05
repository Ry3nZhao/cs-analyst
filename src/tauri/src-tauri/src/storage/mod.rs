//! 存储与数据（W3-D，迁移 009）：
//! - 原始 demo 的存储策略（保留 / 压缩为 `.dem.zst` / 删除只留数据），解析成功后自动执行；
//! - 按天数或总容量清理原始 demo（收藏的对局永不清理，执行前必须先预览）；
//! - 存储占用统计、CSV / JSON 导出、整库备份与恢复。
//!
//! **安全约定**（所有不可逆操作都遵守）：
//! 1. 只处理应用数据目录 `demos/` 下的文件，用户自己目录里的 demo 永远不动；
//! 2. 压缩 / 删除前确认该对局已用当前解析器版本解析完成并入库（见 [`check_demo_safe`]）；
//! 3. 压缩先写临时文件并完整解压校验，与原文件逐字节一致后才替换；
//! 4. 前端发起的删除 / 清理 / 恢复必须带上预览时下发的一次性令牌或显式确认。

pub mod backup;
pub mod cleanup;
pub mod commands;
pub mod demo;
pub mod export;
pub mod usage;

#[cfg(test)]
mod tests;

use crate::database::{Database, DbError, Result};
use parking_lot::Mutex;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, OnceLock};

pub(crate) fn log(msg: impl AsRef<str>) {
    eprintln!("[storage] {}", msg.as_ref());
}

// ---------------------------------------------------------------- settings

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DemoPolicy {
    Keep,
    Compress,
    Delete,
}

impl DemoPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            DemoPolicy::Keep => "keep",
            DemoPolicy::Compress => "compress",
            DemoPolicy::Delete => "delete",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "keep" => Some(DemoPolicy::Keep),
            "compress" => Some(DemoPolicy::Compress),
            "delete" => Some(DemoPolicy::Delete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSettings {
    pub demo_policy: DemoPolicy,
    pub auto_cleanup: bool,
    pub cleanup_max_age_days: Option<i64>,
    pub cleanup_max_total_gb: Option<f64>,
    #[serde(default)]
    pub last_cleanup_at: Option<String>,
    #[serde(default)]
    pub last_cleanup_summary: Option<String>,
}

pub fn get_settings(db: &Database) -> Result<StorageSettings> {
    Ok(db.conn.query_row(
        "SELECT demo_policy, auto_cleanup, cleanup_max_age_days, cleanup_max_total_gb,
                last_cleanup_at, last_cleanup_summary
         FROM storage_settings WHERE id = 1",
        [],
        |r| {
            let policy: String = r.get(0)?;
            Ok(StorageSettings {
                demo_policy: DemoPolicy::parse(&policy).unwrap_or(DemoPolicy::Keep),
                auto_cleanup: r.get::<_, i64>(1)? != 0,
                cleanup_max_age_days: r.get(2)?,
                cleanup_max_total_gb: r.get(3)?,
                last_cleanup_at: r.get(4)?,
                last_cleanup_summary: r.get(5)?,
            })
        },
    )?)
}

pub fn update_settings(db: &Database, s: &StorageSettings) -> Result<()> {
    let days = s.cleanup_max_age_days.filter(|d| *d > 0);
    let gb = s.cleanup_max_total_gb.filter(|g| g.is_finite() && *g > 0.0);
    if s.auto_cleanup && days.is_none() && gb.is_none() {
        return Err(DbError::Message("开启自动清理前至少设置一条规则（天数或总容量）".into()));
    }
    db.conn.execute(
        "UPDATE storage_settings SET demo_policy = ?1, auto_cleanup = ?2, cleanup_max_age_days = ?3,
           cleanup_max_total_gb = ?4, updated_at = datetime('now')
         WHERE id = 1",
        params![s.demo_policy.as_str(), s.auto_cleanup, days, gb],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- favorites

pub fn set_favorite(db: &Database, ids: &[String], favorite: bool) -> Result<usize> {
    let tx = db.conn.unchecked_transaction()?;
    let mut n = 0;
    for id in ids {
        n += tx.execute("UPDATE matches SET favorite = ?2 WHERE id = ?1", params![id, favorite])?;
    }
    tx.commit()?;
    Ok(n)
}

/// 对局列表需要的存储字段（不改动 `list_matches`，前端按 id 合并）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchStorageInfo {
    pub id: String,
    pub favorite: bool,
    pub demo_storage: String,
    pub file_size: Option<i64>,
    pub demo_original_size: Option<i64>,
}

pub fn list_match_storage(db: &Database) -> Result<Vec<MatchStorageInfo>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, favorite, demo_storage, file_size, demo_original_size FROM matches",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(MatchStorageInfo {
            id: r.get(0)?,
            favorite: r.get::<_, i64>(1)? != 0,
            demo_storage: r.get(2)?,
            file_size: r.get(3)?,
            demo_original_size: r.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ---------------------------------------------------------------- safety checks

/// What is about to happen to a demo file; the stricter checks apply to `Delete`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoAction {
    Compress,
    Delete,
}

/// A demo that passed [`check_demo_safe`].
#[derive(Debug, Clone)]
pub struct SafeDemo {
    pub rel_path: String,
    pub abs_path: PathBuf,
    pub storage: String,
    pub size: u64,
}

/// `demos/` under the data root, canonicalised when it exists.
fn demos_root(db: &Database) -> PathBuf {
    let root = db.data_root.join("demos");
    std::fs::canonicalize(&root).unwrap_or(root)
}

/// True when `path` resolves to a file inside the app's `demos/` directory.
pub fn is_managed_demo_path(db: &Database, path: &Path) -> bool {
    let Ok(canon) = std::fs::canonicalize(path) else { return false };
    canon.starts_with(demos_root(db))
}

/// Everything that must hold before a demo file may be compressed or deleted. Returns a
/// user-facing reason when it may not.
pub fn check_demo_safe(db: &Database, id: &str, action: DemoAction) -> std::result::Result<SafeDemo, String> {
    let row: Option<(String, Option<String>, Option<String>, Option<i64>, String)> = db
        .conn
        .query_row(
            "SELECT status, file_path, analysis_status, analyzer_version, demo_storage FROM matches WHERE id = ?",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((status, file_path, analysis_status, analyzer_version, storage)) = row else {
        return Err("对局不存在".into());
    };
    let rel = file_path.filter(|p| !p.trim().is_empty());
    let Some(rel) = rel else { return Err("没有原始 demo 文件".into()) };
    if storage == "deleted" {
        return Err("原始 demo 已删除".into());
    }
    if action == DemoAction::Compress && storage == "compressed" {
        return Err("已经压缩过".into());
    }
    if status != "completed" {
        return Err("下载未完成".into());
    }
    if analysis_status.as_deref() != Some("done") {
        return Err("分析尚未完成".into());
    }
    if analyzer_version.unwrap_or(0) < demo_parse::PARSER_VERSION as i64 {
        return Err("分析结果不是当前解析器版本，需先重新解析".into());
    }
    let has = |sql: &str| -> std::result::Result<bool, String> {
        db.conn.query_row(sql, params![id], |r| r.get::<_, bool>(0)).map_err(|e| e.to_string())
    };
    if !has("SELECT EXISTS(SELECT 1 FROM rounds WHERE match_id = ?)")?
        || !has("SELECT EXISTS(SELECT 1 FROM match_players WHERE match_id = ?)")?
    {
        return Err("数据库里没有这场对局的回合或玩家数据".into());
    }
    if action == DemoAction::Delete {
        if has("SELECT favorite <> 0 FROM matches WHERE id = ?")? {
            return Err("已收藏，不删除原始 demo".into());
        }
        // 删除后无法再从 demo 补算，派生数据也必须已入库
        if !has("SELECT EXISTS(SELECT 1 FROM player_match_stats WHERE match_id = ?)")? {
            return Err("基础指标尚未入库".into());
        }
        let aim_ok: bool = db
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM match_aim WHERE match_id = ?1 AND aim_version >= ?2)",
                params![id, ::aim::AIM_VERSION],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !aim_ok {
            return Err("枪法分析尚未完成".into());
        }
    }
    let abs = db.resolve_path(&rel);
    if !abs.exists() {
        return Err("demo 文件不存在".into());
    }
    if !is_managed_demo_path(db, &abs) {
        return Err("demo 不在应用数据目录的 demos/ 下，不做处理".into());
    }
    let size = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
    Ok(SafeDemo { rel_path: rel, abs_path: abs, storage, size })
}

/// Played-at (or discovered-at) of a match, for display.
pub(crate) fn match_label(db: &Database, id: &str) -> (Option<String>, Option<String>, Option<String>) {
    db.conn
        .query_row(
            "SELECT map_name, COALESCE(played_at, discovered_at), platform FROM matches WHERE id = ?",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((None, None, None))
}

// ---------------------------------------------------------------- background worker

static WORKER: OnceLock<Sender<String>> = OnceLock::new();

/// Start the storage worker (applies the demo policy after each analysis) and the optional
/// startup cleanup. Call once from app setup.
pub fn init(db: Arc<Mutex<Database>>) {
    let (tx, rx) = channel::<String>();
    if WORKER.set(tx).is_err() {
        return;
    }
    let worker_db = db.clone();
    std::thread::Builder::new()
        .name("storage-policy".into())
        .spawn(move || {
            for id in rx {
                apply_policy_after_analysis(&worker_db, &id);
            }
        })
        .expect("spawn storage worker");

    std::thread::Builder::new()
        .name("storage-auto-cleanup".into())
        .spawn(move || {
            // 等回填和解析队列先跑起来；清理只处理已经安全的对局
            std::thread::sleep(std::time::Duration::from_secs(60));
            let enabled = get_settings(&db.lock()).map(|s| s.auto_cleanup).unwrap_or(false);
            if enabled {
                match cleanup::run_auto(&db) {
                    Ok(r) => log(format!("auto cleanup: {} demo(s), {} bytes freed", r.deleted, r.bytes_freed)),
                    Err(e) => log(format!("auto cleanup failed: {e}")),
                }
            }
        })
        .expect("spawn storage cleanup");
}

/// Called by the analysis worker after a match is fully analysed (stats + aim stored).
pub fn on_match_analyzed(match_id: &str) {
    if let Some(tx) = WORKER.get() {
        let _ = tx.send(match_id.to_string());
    }
}

fn apply_policy_after_analysis(db: &Arc<Mutex<Database>>, id: &str) {
    let policy = match get_settings(&db.lock()) {
        Ok(s) => s.demo_policy,
        Err(_) => return,
    };
    let result = match policy {
        DemoPolicy::Keep => return,
        DemoPolicy::Compress => demo::compress_match(db, id).map(|_| ()),
        DemoPolicy::Delete => demo::delete_match_demo(db, id).map(|_| ()),
    };
    if let Err(e) = result {
        log(format!("policy {} skipped {id}: {e}", policy.as_str()));
    }
}

pub(crate) fn now_stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// A directory name that does not exist yet under `parent` (`name`, `name-2`, …).
pub(crate) fn unique_dir(parent: &Path, name: &str) -> PathBuf {
    let mut p = parent.join(name);
    let mut i = 2;
    while p.exists() {
        p = parent.join(format!("{name}-{i}"));
        i += 1;
    }
    p
}

pub(crate) fn dir_size(path: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(path) else { return 0 };
    rd.filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}
