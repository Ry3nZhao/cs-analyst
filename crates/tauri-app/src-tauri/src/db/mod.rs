pub mod models;
pub mod repo;

use duckdb::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("duckdb: {0}")]
    Duck(#[from] duckdb::Error),
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
        let db_path = data_root.join("db").join("analyst.duckdb");
        let conn = Connection::open(&db_path)?;
        let db = Self { conn, data_root };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let sql = include_str!("../../../../../migrations/duckdb/001_initial.sql");
        for statement in sql.split(';') {
            let stmt = statement.trim();
            if !stmt.is_empty() && !stmt.starts_with("--") {
                let _ = self.conn.execute_batch(&format!("{stmt};"));
            }
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

pub fn default_data_root() -> PathBuf {
    PathBuf::from(r"C:\CS-Analyst\Data")
}
