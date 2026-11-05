//! Preview-then-apply plans for bulk demo operations:
//! - **cleanup**: delete raw demos older than N days / beyond a total size (favorites never);
//! - **policy**: apply "compress" or "delete" to demos that already exist.
//!
//! Every preview stores a one-time token together with the exact match ids it listed; apply only
//! accepts that token and only touches those ids (each one re-checked right before it is changed).

use super::demo::{compress_match, delete_match_demo_locked};
use super::{check_demo_safe, get_settings, is_managed_demo_path, match_label, DemoAction};
use crate::database::{Database, Result};
use parking_lot::Mutex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRules {
    pub max_age_days: Option<i64>,
    pub max_total_gb: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub id: String,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
    pub platform: Option<String>,
    pub demo_storage: String,
    pub bytes: u64,
    pub favorite: bool,
    /// 会被处理；为 false 时 `reason` 说明为什么跳过
    pub eligible: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// 执行时原样传回；只能使用一次
    pub token: String,
    /// `cleanup` / `compress` / `delete`
    pub action: String,
    pub items: Vec<PlanItem>,
    pub eligible_count: usize,
    /// 会被处理的 demo 当前占用
    pub eligible_bytes: u64,
    pub skipped_favorites: usize,
    /// 所有原始 demo 当前总占用
    pub total_demo_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanResult {
    pub action: String,
    pub processed: usize,
    pub deleted: usize,
    pub bytes_freed: u64,
    pub failed: Vec<PlanFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFailure {
    pub id: String,
    pub reason: String,
}

struct Pending {
    token: String,
    action: String,
    ids: Vec<String>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

struct DemoRow {
    id: String,
    path: String,
    storage: String,
    favorite: bool,
    age_days: f64,
}

/// All matches that still have a demo file, oldest first.
fn demo_rows(db: &Database) -> Result<Vec<DemoRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, file_path, demo_storage, favorite,
                COALESCE(julianday('now') - julianday(COALESCE(played_at, discovered_at)), 0)
         FROM matches
         WHERE file_path IS NOT NULL AND file_path <> '' AND demo_storage <> 'deleted'
         ORDER BY COALESCE(played_at, discovered_at) ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(DemoRow {
            id: r.get(0)?,
            path: r.get(1)?,
            storage: r.get(2)?,
            favorite: r.get::<_, i64>(3)? != 0,
            age_days: r.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn file_size(db: &Database, rel: &str) -> u64 {
    std::fs::metadata(db.resolve_path(rel)).map(|m| m.len()).unwrap_or(0)
}

fn item(db: &Database, row: &DemoRow, bytes: u64, check: std::result::Result<(), String>) -> PlanItem {
    let (map_name, played_at, platform) = match_label(db, &row.id);
    PlanItem {
        id: row.id.clone(),
        map_name,
        played_at,
        platform,
        demo_storage: row.storage.clone(),
        bytes,
        favorite: row.favorite,
        eligible: check.is_ok(),
        reason: check.err(),
    }
}

fn finish(action: &str, items: Vec<PlanItem>, total_demo_bytes: u64) -> Plan {
    let mut items = items;
    // 会被处理的排前面
    items.sort_by_key(|i| !i.eligible);
    let eligible: Vec<&PlanItem> = items.iter().filter(|i| i.eligible).collect();
    let token = uuid::Uuid::new_v4().to_string();
    let ids: Vec<String> = eligible.iter().map(|i| i.id.clone()).collect();
    let plan = Plan {
        token: token.clone(),
        action: action.into(),
        eligible_count: ids.len(),
        eligible_bytes: eligible.iter().map(|i| i.bytes).sum(),
        skipped_favorites: items.iter().filter(|i| i.favorite && !i.eligible).count(),
        items,
        total_demo_bytes,
    };
    *PENDING.lock() = Some(Pending { token, action: action.into(), ids });
    plan
}

/// Which demos the cleanup rules would delete. Favorites are listed but never selected.
pub fn preview_cleanup(db: &Database, rules: &CleanupRules) -> Result<Plan> {
    let rows = demo_rows(db)?;
    let sizes: Vec<u64> = rows.iter().map(|r| file_size(db, &r.path)).collect();
    let total: u64 = sizes.iter().sum();
    let days = rules.max_age_days.filter(|d| *d > 0);
    let cap = rules.max_total_gb.filter(|g| g.is_finite() && *g > 0.0).map(|g| (g * 1024f64.powi(3)) as u64);

    let mut items = vec![];
    let mut selected = vec![false; rows.len()];
    let mut remaining = total;
    let consider = |i: usize, items: &mut Vec<PlanItem>, remaining: &mut u64| {
        let row = &rows[i];
        let check = check_demo_safe(db, &row.id, DemoAction::Delete).map(|_| ());
        if check.is_ok() {
            *remaining = remaining.saturating_sub(sizes[i]);
        }
        items.push(item(db, row, sizes[i], check));
    };
    if let Some(days) = days {
        for (i, row) in rows.iter().enumerate() {
            if row.age_days > days as f64 {
                selected[i] = true;
                consider(i, &mut items, &mut remaining);
            }
        }
    }
    if let Some(cap) = cap {
        // 从最旧的开始删，直到预计占用不超过上限；收藏和不满足条件的不计入释放量
        for i in 0..rows.len() {
            if remaining <= cap {
                break;
            }
            if selected[i] || rows[i].favorite {
                continue;
            }
            selected[i] = true;
            consider(i, &mut items, &mut remaining);
        }
    }
    Ok(finish("cleanup", items, total))
}

/// Apply the demo policy (`compress` / `delete`) to existing demos, optionally only `ids`.
pub fn preview_policy(db: &Database, action: DemoAction, ids: Option<&[String]>) -> Result<Plan> {
    let rows = demo_rows(db)?;
    let mut total = 0;
    let mut items = vec![];
    for row in &rows {
        let size = file_size(db, &row.path);
        total += size;
        if let Some(ids) = ids {
            if !ids.contains(&row.id) {
                continue;
            }
        }
        if action == DemoAction::Compress && row.storage == "compressed" {
            continue;
        }
        // 用户自己目录里的 demo 不出现在列表里（永远不处理）
        if !is_managed_demo_path(db, &db.resolve_path(&row.path)) {
            continue;
        }
        let check = check_demo_safe(db, &row.id, action).map(|_| ());
        items.push(item(db, row, size, check));
    }
    let name = if action == DemoAction::Compress { "compress" } else { "delete" };
    Ok(finish(name, items, total))
}

fn take_pending(token: &str) -> std::result::Result<Pending, String> {
    let mut slot = PENDING.lock();
    match slot.take() {
        Some(p) if p.token == token => Ok(p),
        other => {
            *slot = other;
            Err("预览已过期，请重新预览后再执行".into())
        }
    }
}

/// Execute a previewed plan. `progress(done, total)` is called after each match.
pub fn apply_plan(
    db: &Arc<Mutex<Database>>,
    token: &str,
    progress: &dyn Fn(usize, usize),
) -> std::result::Result<PlanResult, String> {
    let pending = take_pending(token)?;
    let mut result = PlanResult { action: pending.action.clone(), ..Default::default() };
    let total = pending.ids.len();
    for (i, id) in pending.ids.iter().enumerate() {
        let outcome = if pending.action == "compress" {
            compress_match(db, id)
        } else {
            delete_match_demo_locked(&db.lock(), id)
        };
        match outcome {
            Ok(o) => {
                result.processed += 1;
                if o.bytes_after == 0 {
                    result.deleted += 1;
                }
                result.bytes_freed += o.bytes_before.saturating_sub(o.bytes_after);
            }
            Err(reason) => result.failed.push(PlanFailure { id: id.clone(), reason }),
        }
        progress(i + 1, total);
    }
    if pending.action == "cleanup" {
        record_cleanup(&db.lock(), &result);
    }
    Ok(result)
}

fn record_cleanup(db: &Database, r: &PlanResult) {
    let gb = r.bytes_freed as f64 / 1024f64.powi(3);
    let freed = if gb >= 1.0 { format!("{gb:.1} GB") } else { format!("{:.0} MB", r.bytes_freed as f64 / 1024f64.powi(2)) };
    let summary = format!(
        "删除 {} 个 demo，释放 {freed}{}",
        r.deleted,
        if r.failed.is_empty() { String::new() } else { format!("，{} 个跳过", r.failed.len()) }
    );
    let _ = db.conn.execute(
        "UPDATE storage_settings SET last_cleanup_at = datetime('now'), last_cleanup_summary = ?1 WHERE id = 1",
        params![summary],
    );
}

/// Startup cleanup with the saved rules (the user enabled it after seeing a preview).
pub fn run_auto(db: &Arc<Mutex<Database>>) -> std::result::Result<PlanResult, String> {
    let token = {
        let guard = db.lock();
        let s = get_settings(&guard).map_err(|e| e.to_string())?;
        let rules = CleanupRules { max_age_days: s.cleanup_max_age_days, max_total_gb: s.cleanup_max_total_gb };
        if rules.max_age_days.is_none() && rules.max_total_gb.is_none() {
            return Ok(PlanResult::default());
        }
        preview_cleanup(&guard, &rules).map_err(|e| e.to_string())?.token
    };
    apply_plan(db, &token, &|_, _| {})
}
