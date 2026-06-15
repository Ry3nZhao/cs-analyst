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

        if version < 1 {
            self.conn
                .execute_batch(include_str!("../../../../../migrations/sqlite/001_initial.sql"))?;
            self.conn.execute_batch("PRAGMA user_version = 1")?;
        }

        Ok(())
    }

    pub fn get_data_root(&self) -> String {
        self.data_root.to_string_lossy().to_string()
    }

    pub fn demo_dir(&self, platform: &str, external_id: &str) -> PathBuf {
        self.data_root
            .join("demos")
            .join(platform)
            .join(external_id)
    }

    pub fn ensure_demo_dir(&self, platform: &str, external_id: &str) -> Result<PathBuf> {
        let dir = self.demo_dir(platform, external_id);
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
}

pub fn resolve_data_root(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}
