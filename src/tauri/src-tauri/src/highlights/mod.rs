//! 高光 / 失误集锦（W3-A，迁移 010，docs/highlights.md）。
//!
//! - 规则在 `crates/highlights`（纯函数），这里负责从事件表读数据、写 `highlights`、查询和设置；
//! - 解析 worker 在 stats / aim 都入库之后调用 [`on_match_analyzed`]；
//! - 启动后台回填：没算过、规则版本变了、stats 重算过、或枪法数据后来才有的对局都会重算；
//! - [`play`]：在 CS2 中播放（复制命令、启动 CS2、实验性的 `-netconport` 自动跳转）。

pub mod commands;
pub mod play;
pub mod repo;

#[cfg(test)]
mod tests;

use crate::database::Database;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) fn log(msg: impl AsRef<str>) {
    eprintln!("[highlights] {}", msg.as_ref());
}

/// 用库里的事件重算一场。没有统计（stats 还没算）时返回 `Ok(None)`，等回填。
pub fn recompute(db: &Database, match_id: &str) -> Result<Option<usize>, String> {
    if repo::stats_version(db, match_id).map_err(|e| e.to_string())?.is_none() {
        return Ok(None);
    }
    let Some(input) = repo::load_input(db, match_id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let rows = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| highlights::detect(&input)))
        .map_err(|_| "集锦规则崩溃".to_string())?;
    repo::replace_highlights(db, match_id, &input, &rows).map_err(|e| e.to_string())?;
    Ok(Some(rows.len()))
}

fn recompute_logged(db: &Database, match_id: &str) {
    let started = Instant::now();
    match recompute(db, match_id) {
        Ok(Some(n)) => log(format!("done {match_id}: {n} moment(s) in {} ms", started.elapsed().as_millis())),
        Ok(None) => {}
        Err(e) => {
            log(format!("failed {match_id}: {e}"));
            let _ = repo::mark_failed(db, match_id, &e);
        }
    }
}

/// 解析 worker 在一场完整入库（事件 + stats + aim）之后调用。
pub fn on_match_analyzed(db: &Arc<Mutex<Database>>, match_id: &str) {
    recompute_logged(&db.lock(), match_id);
}

/// 启动回填，之后每 2 分钟检查一次（stats / aim 回填是异步的，算完之后这里跟着补）。
pub fn start_backfill(db: Arc<Mutex<Database>>) {
    std::thread::Builder::new()
        .name("highlights-backfill".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(15));
            loop {
                let ids = repo::list_matches_needing(&db.lock()).unwrap_or_default();
                if !ids.is_empty() {
                    log(format!("backfill: {} match(es)", ids.len()));
                }
                for id in ids {
                    recompute_logged(&db.lock(), &id);
                }
                std::thread::sleep(Duration::from_secs(120));
            }
        })
        .expect("spawn highlights backfill");
}
