//! Whole-database backup and restore.
//!
//! **Never copy `analyst.db` / `-wal` / `-shm` files directly.** With WAL journaling the main
//! file alone is not a consistent snapshot, and pairing a main file with the wrong (or a stale)
//! `-wal` file loses or corrupts data. Instead:
//! - backup: `VACUUM INTO` on a separate read-only connection → one self-contained, consistent
//!   `.db` file (no WAL), plus an optional copy of the replay files;
//! - restore: SQLite's online backup API copies the snapshot page by page *into the live
//!   connection*, so the running WAL database stays consistent. The current database is backed up
//!   automatically first.
//!
//! Layout: `<data_root>/backups/<YYYYMMDD-HHMMSS>[-pre-restore]/{analyst.db, manifest.json, analysis/*.replay.zst}`.

use super::{dir_size, log, now_stamp, unique_dir};
use crate::database::{Database, DbError, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const BACKUP_DB_NAME: &str = "analyst.db";
const MANIFEST: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
    pub parser_version: u32,
    pub matches: i64,
    pub includes_replays: bool,
    pub replay_files: u64,
    /// `manual` / `pre-restore`
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub manifest: Option<BackupManifest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    /// 恢复前自动生成的当前库备份
    pub pre_restore_backup: BackupInfo,
    pub schema_version: i64,
    pub matches: i64,
    pub replay_files_restored: u64,
}

pub fn backups_root(data_root: &Path) -> PathBuf {
    data_root.join("backups")
}

fn db_file(data_root: &Path) -> PathBuf {
    data_root.join("db").join("analyst.db")
}

fn count_matches(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM matches", [], |r| r.get(0)).unwrap_or(0)
}

/// Write a consistent snapshot of the live database into a new backup directory.
///
/// Uses a separate read-only connection, so the app's own connection (and its mutex) is not held
/// while the snapshot is written; WAL readers see a consistent view of the last commit.
pub fn create_backup(data_root: &Path, include_replays: bool, kind: &str) -> Result<BackupInfo> {
    let root = backups_root(data_root);
    std::fs::create_dir_all(&root)?;
    let suffix = if kind == "manual" { String::new() } else { format!("-{kind}") };
    let dir = unique_dir(&root, &format!("{}{suffix}", now_stamp()));
    std::fs::create_dir_all(&dir)?;
    let result = (|| {
        let target = dir.join(BACKUP_DB_NAME);
        let src = Connection::open_with_flags(db_file(data_root), OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        src.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
        let (schema_version, matches) = {
            let snap = Connection::open_with_flags(&target, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let v: i64 = snap.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            (v, count_matches(&snap))
        };
        let mut replay_files = 0;
        if include_replays {
            replay_files = copy_replays(&data_root.join("analysis"), &dir.join("analysis"))?;
        }
        let manifest = BackupManifest {
            created_at: chrono::Local::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            schema_version,
            parser_version: demo_parse::PARSER_VERSION,
            matches,
            includes_replays: include_replays,
            replay_files,
            kind: kind.into(),
        };
        std::fs::write(
            dir.join(MANIFEST),
            serde_json::to_vec_pretty(&manifest).map_err(|e| DbError::Message(e.to_string()))?,
        )?;
        log(format!("backup {} ({matches} matches, {replay_files} replays)", dir.display()));
        Ok(info(&dir))
    })();
    if result.is_err() {
        // 半成品备份目录没有用处，删掉（只删本函数刚建的目录）
        let _ = std::fs::remove_dir_all(&dir);
    }
    result
}

fn copy_replays(from: &Path, to: &Path) -> Result<u64> {
    let Ok(rd) = std::fs::read_dir(from) else { return Ok(0) };
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_file() && name.ends_with(".replay.zst") {
            std::fs::copy(&p, to.join(&name))?;
            n += 1;
        }
    }
    Ok(n)
}

fn info(dir: &Path) -> BackupInfo {
    let manifest = std::fs::read(dir.join(MANIFEST)).ok().and_then(|b| serde_json::from_slice(&b).ok());
    BackupInfo {
        name: dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        path: dir.to_string_lossy().into_owned(),
        bytes: dir_size(dir),
        manifest,
    }
}

pub fn list_backups(db: &Database) -> Result<Vec<BackupInfo>> {
    let Ok(rd) = std::fs::read_dir(backups_root(&db.data_root)) else { return Ok(vec![]) };
    let mut out: Vec<BackupInfo> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join(BACKUP_DB_NAME).is_file())
        .map(|p| info(&p))
        .collect();
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// A backup is a directory containing `analyst.db`, or the `.db` file itself.
fn resolve_backup(path: &Path) -> Result<(PathBuf, Option<PathBuf>)> {
    if path.is_dir() {
        let f = path.join(BACKUP_DB_NAME);
        if !f.is_file() {
            return Err(DbError::Message(format!("目录里没有 {BACKUP_DB_NAME}")));
        }
        return Ok((f, Some(path.to_path_buf())));
    }
    if path.is_file() {
        return Ok((path.to_path_buf(), path.parent().map(Path::to_path_buf)));
    }
    Err(DbError::Message("备份不存在".into()))
}

/// Check a backup file before restoring: it opens, passes `quick_check`, looks like this app's
/// schema and is not newer than the running app.
pub fn validate_backup(db: &Database, path: &Path) -> Result<(PathBuf, Option<PathBuf>, i64)> {
    let (file, dir) = resolve_backup(path)?;
    for suffix in ["-wal", "-shm"] {
        if PathBuf::from(format!("{}{suffix}", file.display())).exists() {
            return Err(DbError::Message(format!(
                "备份旁边有 {suffix} 文件，说明它是被直接复制的数据库或仍在使用中，不能用于恢复"
            )));
        }
    }
    let conn = Connection::open_with_flags(&file, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(DbError::Message(format!("备份文件损坏: {check}")));
    }
    let has_matches: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='matches')",
        [],
        |r| r.get(0),
    )?;
    if !has_matches {
        return Err(DbError::Message("不是 CS Analyst 的数据库备份".into()));
    }
    let backup_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let current: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if backup_version > current {
        return Err(DbError::Message(format!(
            "备份来自更新版本的应用（数据库版本 {backup_version} > 当前 {current}），请先升级应用"
        )));
    }
    Ok((file, dir, backup_version))
}

/// Restore a backup into the live database. Backs up the current database first; the caller holds
/// the app's database lock for the whole call, so nothing else writes meanwhile.
pub fn restore_backup(db: &mut Database, path: &Path) -> Result<RestoreResult> {
    let (file, dir, _) = validate_backup(db, path)?;
    // 1. 恢复前自动备份当前库（含回放文件，恢复会覆盖它们）
    let data_root = db.data_root.clone();
    let pre = create_backup(&data_root, true, "pre-restore")?;
    // 2. 在线备份 API：把快照逐页写进正在使用的连接，WAL 由 SQLite 自己维护
    {
        let src = Connection::open_with_flags(&file, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let backup = rusqlite::backup::Backup::new(&src, &mut db.conn)?;
        backup.run_to_completion(256, Duration::from_millis(0), None)?;
    }
    db.conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    // 3. 备份来自旧版本时补齐迁移
    db.migrate()?;
    let schema_version: i64 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    // 4. 回放文件（备份里有才恢复；同名覆盖，旧的已在第 1 步备份）
    let mut replays = 0;
    if let Some(dir) = dir {
        let from = dir.join("analysis");
        if from.is_dir() {
            replays = copy_replays(&from, &db.data_root.join("analysis"))?;
        }
    }
    let matches = count_matches(&db.conn);
    log(format!("restored {} ({matches} matches, {replays} replays)", file.display()));
    Ok(RestoreResult { pre_restore_backup: pre, schema_version, matches, replay_files_restored: replays })
}
