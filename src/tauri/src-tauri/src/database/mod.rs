pub mod analysis;
pub mod models;
pub mod repo;

use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, DbError>;

/// Each entry: (SQL file, PRAGMA user_version after apply). Add `002_*.sql` when needed.
const MIGRATIONS: &[(&str, i32)] = &[
    (include_str!("../../../../../migrations/sqlite/001_initial.sql"), 1),
    (include_str!("../../../../../migrations/sqlite/002_analysis.sql"), 2),
    (include_str!("../../../../../migrations/sqlite/003_stats.sql"), 3),
    (include_str!("../../../../../migrations/sqlite/004_identity.sql"), 4),
    (include_str!("../../../../../migrations/sqlite/005_aim.sql"), 5),
];

pub struct Database {
    pub conn: Connection,
    pub data_root: PathBuf,
}

impl Database {
    pub fn open(data_root: PathBuf) -> Result<Self> {
        eprintln!("[db] data_root = {}", data_root.display());
        fs::create_dir_all(data_root.join("db"))?;
        fs::create_dir_all(data_root.join("demos"))?;
        fs::create_dir_all(data_root.join("state"))?;
        fs::create_dir_all(data_root.join("analysis"))?;
        let db_path = data_root.join("db").join("analyst.db");
        let mut conn = Connection::open(&db_path)?;
        let mut legacy_path: Option<PathBuf> = None;
        if Self::is_legacy_schema(&conn)? {
            // v0.1.x 的库 user_version=1 但表结构完全不同（discovered_matches/download_jobs），
            // 无法原地迁移：归档旧文件后重建，再从归档中带回配置数据
            drop(conn);
            legacy_path = Some(Self::archive_legacy_db(&db_path)?);
            conn = Connection::open(&db_path)?;
        }
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn, data_root };
        db.migrate()?;
        if let Some(ref legacy) = legacy_path {
            db.import_legacy_config(legacy)?;
        }
        Ok(db)
    }

    fn is_legacy_schema(conn: &Connection) -> Result<bool> {
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            return Ok(false);
        }
        let has_matches: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='matches')",
            [],
            |r| r.get(0),
        )?;
        if !has_matches {
            eprintln!("[db] detected legacy schema (user_version={version}), archiving");
        }
        Ok(!has_matches)
    }

    fn archive_legacy_db(db_path: &Path) -> Result<PathBuf> {
        let dir = db_path
            .parent()
            .ok_or_else(|| DbError::Message("db path has no parent".into()))?;
        // 归档名保持 <base>-wal/<base>-shm 约定，SQLite 重新打开归档库时才能恢复 WAL 数据
        let archived = dir.join("analyst.legacy.db");
        for suffix in ["", "-wal", "-shm"] {
            let src = PathBuf::from(format!("{}{suffix}", db_path.display()));
            if src.exists() {
                let dest = PathBuf::from(format!("{}{suffix}", archived.display()));
                if dest.exists() {
                    fs::remove_file(&dest)?;
                }
                fs::rename(&src, &dest)?;
            }
        }
        Ok(archived)
    }

    /// 从归档的 v0.1.x 库中带回配置类数据（users/sub_accounts/settings/bot 凭据）。
    /// 对局与下载任务表结构差异过大，不迁移（重新 discover 即可）。
    fn import_legacy_config(&self, legacy_db: &Path) -> Result<()> {
        let path = legacy_db.to_string_lossy().replace('\'', "''");
        self.conn.execute_batch(&format!(
            "ATTACH DATABASE '{path}' AS legacy;
             INSERT OR IGNORE INTO users SELECT * FROM legacy.users;
             INSERT OR IGNORE INTO sub_accounts SELECT * FROM legacy.sub_accounts;
             INSERT OR IGNORE INTO sync_cursors SELECT * FROM legacy.sync_cursors;
             INSERT OR REPLACE INTO platform_settings SELECT * FROM legacy.platform_settings;
             INSERT OR REPLACE INTO app_settings SELECT * FROM legacy.app_settings;
             UPDATE valve_bot_config SET
               username = (SELECT username FROM legacy.valve_bot_config WHERE id = 1),
               password_enc = (SELECT password_enc FROM legacy.valve_bot_config WHERE id = 1),
               secret_enc = (SELECT secret_enc FROM legacy.valve_bot_config WHERE id = 1),
               steam_api_key_enc = (SELECT steam_api_key_enc FROM legacy.valve_bot_config WHERE id = 1),
               cron_schedule = COALESCE((SELECT cron_schedule FROM legacy.valve_bot_config WHERE id = 1), cron_schedule),
               run_on_startup = COALESCE((SELECT run_on_startup FROM legacy.valve_bot_config WHERE id = 1), run_on_startup)
             WHERE id = 1 AND EXISTS (SELECT 1 FROM legacy.valve_bot_config WHERE id = 1);
             DETACH DATABASE legacy;"
        ))?;
        Ok(())
    }

    fn migrate(&self) -> Result<()> {
        let version: i32 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

        for (sql, target_version) in MIGRATIONS {
            if version >= *target_version {
                continue;
            }
            // 每个迁移单独一个事务：失败时整体回滚，user_version 不前进
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", target_version)?;
            tx.commit()?;
            eprintln!("[db] migrated to user_version={target_version}");
        }
        Ok(())
    }

    pub fn get_data_root(&self) -> String {
        self.data_root.to_string_lossy().to_string()
    }

    pub fn ensure_demo_dir(&self, platform: &str) -> Result<PathBuf> {
        let dir = self.data_root.join("demos").join(platform);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn relative_path(&self, absolute: &Path) -> String {
        absolute
            .strip_prefix(&self.data_root)
            .unwrap_or(absolute)
            .to_string_lossy()
            .replace('\\', "/")
    }

    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let trimmed = path.strip_prefix("./").unwrap_or(path);
        let candidate = Path::new(trimmed);
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.data_root.join(trimmed)
        }
    }
}

pub fn resolve_data_root(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}
