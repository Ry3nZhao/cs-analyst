//! Disk usage breakdown: by category (demo / replay / database / backups / exports), by platform
//! and by account. Sizes come from the files on disk, not from `matches.file_size`.

use super::dir_size;
use crate::database::{Database, Result};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageGroup {
    pub key: String,
    pub label: String,
    pub matches: u64,
    pub demo_bytes: u64,
    pub replay_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub data_root: String,
    pub demo_bytes: u64,
    pub replay_bytes: u64,
    pub database_bytes: u64,
    pub backup_bytes: u64,
    pub export_bytes: u64,
    /// 不在应用数据目录里的 demo（用户自己的目录，不计入清理）
    pub external_demo_bytes: u64,
    pub by_platform: Vec<UsageGroup>,
    pub by_account: Vec<UsageGroup>,
    pub raw_count: u64,
    pub compressed_count: u64,
    pub deleted_count: u64,
    pub favorite_count: u64,
    /// 压缩 / 删除累计节省的字节
    pub saved_bytes: u64,
}

fn platform_label(p: &str) -> &str {
    match p {
        "valve" => "Valve 官匹",
        "5eplay" => "5E",
        "renown" => "Renown",
        "imported" => "手动导入",
        other => other,
    }
}

pub fn compute(db: &Database) -> Result<StorageUsage> {
    let mut u = StorageUsage { data_root: db.get_data_root(), ..Default::default() };
    let demos_root = std::fs::canonicalize(db.data_root.join("demos")).unwrap_or_else(|_| db.data_root.join("demos"));

    let mut stmt = db.conn.prepare(
        "SELECT m.platform, m.sub_account_id, s.display_name, s.platform,
                m.file_path, m.replay_path, m.demo_storage, m.favorite, m.demo_original_size, m.file_size
         FROM matches m LEFT JOIN sub_accounts s ON s.id = m.sub_account_id",
    )?;
    let mut platforms: BTreeMap<String, UsageGroup> = BTreeMap::new();
    let mut accounts: BTreeMap<String, UsageGroup> = BTreeMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let platform: String = r.get(0)?;
        let account_id: Option<String> = r.get(1)?;
        let account_name: Option<String> = r.get(2)?;
        let account_platform: Option<String> = r.get(3)?;
        let file_path: Option<String> = r.get(4)?;
        let replay_path: Option<String> = r.get(5)?;
        let storage: String = r.get(6)?;
        let favorite: bool = r.get::<_, i64>(7)? != 0;
        let original: Option<i64> = r.get(8)?;

        let mut demo = 0u64;
        let mut external = false;
        if let Some(p) = file_path.filter(|p| !p.trim().is_empty()) {
            let abs = db.resolve_path(&p);
            demo = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
            external = !std::fs::canonicalize(&abs).map(|c| c.starts_with(&demos_root)).unwrap_or(true);
        }
        let replay = replay_path
            .filter(|p| !p.trim().is_empty())
            .and_then(|p| std::fs::metadata(db.resolve_path(&p)).ok())
            .map(|m| m.len())
            .unwrap_or(0);

        if external {
            u.external_demo_bytes += demo;
        } else {
            u.demo_bytes += demo;
        }
        u.replay_bytes += replay;
        match storage.as_str() {
            "compressed" => u.compressed_count += 1,
            "deleted" => u.deleted_count += 1,
            _ if demo > 0 => u.raw_count += 1,
            _ => {}
        }
        if storage != "raw" {
            u.saved_bytes += (original.unwrap_or(0) as u64).saturating_sub(demo);
        }
        if favorite {
            u.favorite_count += 1;
        }

        let g = platforms.entry(platform.clone()).or_insert_with(|| UsageGroup {
            key: platform.clone(),
            label: platform_label(&platform).to_string(),
            ..Default::default()
        });
        g.matches += 1;
        g.demo_bytes += demo;
        g.replay_bytes += replay;

        let (key, label) = match (account_id, account_name) {
            (Some(id), name) => {
                let plat = account_platform.as_deref().map(platform_label).unwrap_or("");
                let name = name.unwrap_or_else(|| id.clone());
                (id, if plat.is_empty() { name } else { format!("{name} · {plat}") })
            }
            (None, _) => ("_none".to_string(), "未关联账号".to_string()),
        };
        let g = accounts.entry(key.clone()).or_insert_with(|| UsageGroup { key, label, ..Default::default() });
        g.matches += 1;
        g.demo_bytes += demo;
        g.replay_bytes += replay;
    }

    let db_file = db.data_root.join("db").join("analyst.db");
    for suffix in ["", "-wal", "-shm"] {
        let p = std::path::PathBuf::from(format!("{}{suffix}", db_file.display()));
        u.database_bytes += std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    }
    u.backup_bytes = dir_size(&db.data_root.join("backups"));
    u.export_bytes = dir_size(&db.data_root.join("exports"));

    let by_size = |g: &UsageGroup| std::cmp::Reverse(g.demo_bytes + g.replay_bytes);
    u.by_platform = platforms.into_values().collect();
    u.by_platform.sort_by_key(by_size);
    u.by_account = accounts.into_values().collect();
    u.by_account.sort_by_key(by_size);
    Ok(u)
}
