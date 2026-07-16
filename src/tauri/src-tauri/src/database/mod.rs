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
const MIGRATIONS: &[(&str, i32)] = &[(
    include_str!("../../../../../migrations/sqlite/001_initial.sql"),
    1,
)];

pub struct Database {
    pub conn: Connection,
    pub data_root: PathBuf,
}

impl Database {
    pub fn open(data_root: PathBuf) -> Result<Self> {
        fs::create_dir_all(data_root.join("db"))?;
        fs::create_dir_all(data_root.join("demos"))?;
        fs::create_dir_all(data_root.join("state"))?;
        let db_path = data_root.join("db").join("analyst.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn, data_root };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let version: i32 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

        for (sql, target_version) in MIGRATIONS {
            if version >= *target_version {
                continue;
            }
            self.conn.execute_batch(sql)?;
            self.conn
                .execute_batch(&format!("PRAGMA user_version = {target_version}"))?;
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
