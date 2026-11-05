//! 身份与对位（W1-C，docs/roadmap.md 2.7）。
//!
//! - `link`：自动关联（demo steamid、sub_accounts、平台账号）
//! - `manage`：合并 / 撤销 / 拆分 / 备注标签
//! - `h2h`：每场的对位事实与 h2h_stats 维护、全量重建
//! - `query`：读接口与 DTO
//! - `commands`：Tauri 命令
//!
//! 入口：
//! - `init`：应用启动时调用，后台线程做一次性存量回填（之后只同步 sub_accounts）
//! - `on_match_analyzed`：解析 worker 每场入库后调用，增量更新

pub mod commands;
pub mod h2h;
pub mod link;
pub mod manage;
pub mod query;

#[cfg(test)]
mod tests;

use crate::database::{Database, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Arc;

/// 回填逻辑变化时加 1，启动时会重新回填一次
pub const BACKFILL_VERSION: i64 = 1;

fn log(msg: impl AsRef<str>) {
    eprintln!("[identity] {}", msg.as_ref());
}

/// 一场解析完成后：生成身份、同步子账号、按 steamid 关联、重算这一场的对位。单个事务。
pub fn update_for_match(c: &Connection, match_id: &str) -> Result<()> {
    let tx = c.unchecked_transaction()?;
    link::ensure_match_identities(&tx, match_id)?;
    link::sync_sub_accounts(&tx)?;
    link::link_by_steamid(&tx)?;
    h2h::recompute_match(&tx, match_id)?;
    tx.commit()?;
    Ok(())
}

/// 解析 worker 的挂载点：出错只记日志，不影响解析结果。
pub fn on_match_analyzed(db: &Database, match_id: &str) {
    if let Err(e) = update_for_match(&db.conn, match_id) {
        log(format!("update failed {match_id}: {e}"));
    }
}

fn backfilled_version(c: &Connection) -> Result<i64> {
    Ok(c
        .query_row("SELECT value FROM identity_meta WHERE key = 'backfill_version'", [], |r| r.get::<_, String>(0))
        .optional()?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

/// 存量回填：所有已解析对局生成身份、sub_accounts 迁移、关联、对位全量重建。返回对局数。
pub fn backfill(c: &Connection) -> Result<usize> {
    let tx = c.unchecked_transaction()?;
    let ids: Vec<String> = {
        let mut st = tx.prepare("SELECT id FROM matches WHERE analysis_status = 'done' ORDER BY id")?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for id in &ids {
        link::ensure_match_identities(&tx, id)?;
    }
    link::sync_sub_accounts(&tx)?;
    link::link_by_steamid(&tx)?;
    tx.commit()?;
    let n = h2h::rebuild_all(c)?;
    c.execute(
        "INSERT INTO identity_meta (key, value) VALUES ('backfill_version', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![BACKFILL_VERSION.to_string()],
    )?;
    Ok(n)
}

/// 应用启动：后台线程里回填（只做一次），否则只同步 sub_accounts。
pub fn init(db: Arc<Mutex<Database>>) {
    std::thread::Builder::new()
        .name("identity-init".into())
        .spawn(move || {
            let db = db.lock();
            let result = match backfilled_version(&db.conn) {
                Ok(v) if v >= BACKFILL_VERSION => {
                    let tx = db.conn.unchecked_transaction();
                    tx.map_err(Into::into).and_then(|tx| {
                        link::sync_sub_accounts(&tx)?;
                        link::link_by_steamid(&tx)?;
                        tx.commit()?;
                        Ok(())
                    })
                }
                Ok(_) => backfill(&db.conn).map(|n| log(format!("backfilled {n} match(es)"))),
                Err(e) => Err(e),
            };
            if let Err(e) = result {
                log(format!("init failed: {e}"));
            }
        })
        .expect("spawn identity init");
}
