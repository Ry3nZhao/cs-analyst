//! Compress or delete one match's raw demo. Both re-run [`check_demo_safe`] immediately before
//! touching the file, with the database lock held, so a re-analysis that started in between
//! (status → pending / running) aborts the operation.

use super::{check_demo_safe, log, DemoAction, SafeDemo};
use crate::database::Database;
use parking_lot::Mutex;
use rusqlite::params;
use serde::Serialize;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// zstd level for demos: level 3 compresses a 200 MB demo in a couple of seconds.
pub const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoOpOutcome {
    pub id: String,
    pub bytes_before: u64,
    pub bytes_after: u64,
}

fn zst_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".zst");
    PathBuf::from(s)
}

/// Stream-compress `src` into `dst` and verify by decompressing `dst` and comparing it byte for
/// byte with `src`. On any failure `dst` is removed. Returns the compressed size.
pub fn compress_file_verified(src: &Path, dst: &Path) -> Result<u64, String> {
    let run = || -> std::io::Result<()> {
        let mut input = BufReader::with_capacity(1 << 20, File::open(src)?);
        let out = BufWriter::with_capacity(1 << 20, File::create(dst)?);
        let mut enc = zstd::stream::write::Encoder::new(out, ZSTD_LEVEL)?;
        enc.include_checksum(true)?;
        std::io::copy(&mut input, &mut enc)?;
        let mut out = enc.finish()?;
        out.flush()?;
        out.get_ref().sync_all()?;
        Ok(())
    };
    if let Err(e) = run() {
        let _ = std::fs::remove_file(dst);
        return Err(format!("压缩失败: {e}"));
    }
    match same_content(src, dst) {
        Ok(true) => {}
        Ok(false) => {
            let _ = std::fs::remove_file(dst);
            return Err("压缩校验失败：解压结果与原文件不一致".into());
        }
        Err(e) => {
            let _ = std::fs::remove_file(dst);
            return Err(format!("压缩校验失败: {e}"));
        }
    }
    Ok(std::fs::metadata(dst).map(|m| m.len()).unwrap_or(0))
}

/// True when decompressing `zst` yields exactly the bytes of `raw`.
fn same_content(raw: &Path, zst: &Path) -> std::io::Result<bool> {
    let mut a = BufReader::with_capacity(1 << 20, File::open(raw)?);
    let mut b = zstd::stream::read::Decoder::new(File::open(zst)?)?;
    let mut buf_a = vec![0u8; 1 << 20];
    let mut buf_b = vec![0u8; 1 << 20];
    loop {
        let n = read_full(&mut a, &mut buf_a)?;
        let m = read_full(&mut b, &mut buf_b)?;
        if n != m || buf_a[..n] != buf_b[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

fn read_full(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = r.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

/// Compress a match's raw `.dem` into `.dem.zst` next to it, then remove the original.
pub fn compress_match(db: &Arc<Mutex<Database>>, id: &str) -> Result<DemoOpOutcome, String> {
    let safe = check_demo_safe(&db.lock(), id, DemoAction::Compress)?;
    let dst = zst_path(&safe.abs_path);
    let mut tmp = dst.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    if dst.exists() {
        return Err(format!("目标文件已存在: {}", dst.display()));
    }
    // 压缩耗时数秒，不持有数据库锁
    let compressed = compress_file_verified(&safe.abs_path, &tmp)?;
    if let Err(e) = std::fs::rename(&tmp, &dst) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("重命名失败: {e}"));
    }

    let guard = db.lock();
    let swap = || -> Result<(), String> {
        // 压缩期间对局可能被重新排队解析或文件被替换：重新检查
        let again: SafeDemo = check_demo_safe(&guard, id, DemoAction::Compress)?;
        if again.rel_path != safe.rel_path || again.size != safe.size {
            return Err("压缩期间 demo 文件发生了变化".into());
        }
        let new_rel = guard.relative_path(&dst);
        let file_name = dst.file_name().map(|s| s.to_string_lossy().to_string());
        let tx = guard.conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE matches SET file_path = ?2, file_name = ?3, file_size = ?4,
               demo_original_size = COALESCE(demo_original_size, ?5),
               demo_storage = 'compressed', demo_storage_at = datetime('now')
             WHERE id = ?1",
            params![id, new_rel, file_name, compressed as i64, safe.size as i64],
        )
        .map_err(|e| e.to_string())?;
        // 先删原文件再提交：删不掉（例如被占用）就回滚，库里仍指向原文件
        std::fs::remove_file(&safe.abs_path).map_err(|e| format!("删除原始 demo 失败: {e}"))?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    };
    match swap() {
        Ok(()) => {
            log(format!("compressed {id}: {} → {} bytes", safe.size, compressed));
            Ok(DemoOpOutcome { id: id.into(), bytes_before: safe.size, bytes_after: compressed })
        }
        Err(e) => {
            // 原文件还在才删压缩件；否则压缩件是唯一副本，必须留下
            if safe.abs_path.exists() {
                let _ = std::fs::remove_file(&dst);
            }
            Err(e)
        }
    }
}

/// Delete a match's raw demo (`.dem` or `.dem.zst`), keeping all analysed data.
pub fn delete_match_demo(db: &Arc<Mutex<Database>>, id: &str) -> Result<DemoOpOutcome, String> {
    delete_match_demo_locked(&db.lock(), id)
}

pub(crate) fn delete_match_demo_locked(db: &Database, id: &str) -> Result<DemoOpOutcome, String> {
    let safe = check_demo_safe(db, id, DemoAction::Delete)?;
    let tx = db.conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let original = if safe.storage == "raw" { Some(safe.size as i64) } else { None };
    tx.execute(
        "UPDATE matches SET file_path = NULL, file_size = NULL,
           demo_original_size = COALESCE(demo_original_size, ?2),
           demo_storage = 'deleted', demo_storage_at = datetime('now')
         WHERE id = ?1",
        params![id, original],
    )
    .map_err(|e| e.to_string())?;
    std::fs::remove_file(&safe.abs_path).map_err(|e| format!("删除 demo 失败: {e}"))?;
    tx.commit().map_err(|e| e.to_string())?;
    log(format!("deleted demo of {id}: {} bytes freed", safe.size));
    Ok(DemoOpOutcome { id: id.into(), bytes_before: safe.size, bytes_after: 0 })
}
