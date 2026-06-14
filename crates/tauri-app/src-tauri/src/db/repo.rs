use super::models::*;
use super::{Database, DbError, Result};
use chrono::Utc;
use duckdb::params;
use uuid::Uuid;

pub fn list_users(db: &Database) -> Result<Vec<User>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, display_name, note, created_at::VARCHAR, updated_at::VARCHAR FROM users ORDER BY display_name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(User {
            id: row.get(0)?,
            display_name: row.get(1)?,
            note: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_user(db: &Database, display_name: &str, note: Option<&str>) -> Result<User> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "INSERT INTO users (id, display_name, note, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        params![id, display_name, note, now, now],
    )?;
    Ok(User {
        id,
        display_name: display_name.to_string(),
        note: note.map(|s| s.to_string()),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn update_user(db: &Database, id: &str, display_name: &str, note: Option<&str>) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE users SET display_name = ?, note = ?, updated_at = ? WHERE id = ?",
        params![display_name, note, now, id],
    )?;
    Ok(())
}

pub fn delete_user(db: &Database, id: &str) -> Result<()> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM sub_accounts WHERE user_id = ?",
        params![id],
        |row| row.get(0),
    )?;
    if count > 0 {
        return Err(DbError::Message("User still has bound sub accounts".into()));
    }
    db.conn.execute("DELETE FROM users WHERE id = ?", params![id])?;
    Ok(())
}

pub fn list_sub_accounts(db: &Database, platform: &str) -> Result<Vec<SubAccount>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, user_id, platform, external_id, display_name, avatar_url, status, metadata, created_at::VARCHAR, updated_at::VARCHAR FROM sub_accounts WHERE platform = ? ORDER BY display_name",
    )?;
    let rows = stmt.query_map(params![platform], |row| {
        let meta: String = row.get(7)?;
        Ok(SubAccount {
            id: row.get(0)?,
            user_id: row.get(1)?,
            platform: row.get(2)?,
            external_id: row.get(3)?,
            display_name: row.get(4)?,
            avatar_url: row.get(5)?,
            status: row.get(6)?,
            metadata: serde_json::from_str(&meta).unwrap_or(serde_json::json!({})),
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn add_sub_account(
    db: &Database,
    user_id: &str,
    platform: &str,
    external_id: &str,
    display_name: &str,
    metadata: serde_json::Value,
) -> Result<SubAccount> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let meta = metadata.to_string();
    db.conn.execute(
        "INSERT INTO sub_accounts (id, user_id, platform, external_id, display_name, status, metadata, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?)",
        params![id, user_id, platform, external_id, display_name, meta, now, now],
    )?;
    Ok(SubAccount {
        id,
        user_id: user_id.to_string(),
        platform: platform.to_string(),
        external_id: external_id.to_string(),
        display_name: display_name.to_string(),
        avatar_url: None,
        status: "active".into(),
        metadata,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn update_sub_account_user(db: &Database, id: &str, user_id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE sub_accounts SET user_id = ?, updated_at = ? WHERE id = ?",
        params![user_id, now, id],
    )?;
    Ok(())
}

pub fn set_sub_account_status(db: &Database, id: &str, status: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE sub_accounts SET status = ?, updated_at = ? WHERE id = ?",
        params![status, now, id],
    )?;
    Ok(())
}

pub fn list_active_sub_accounts(db: &Database, platform: &str) -> Result<Vec<SubAccount>> {
    let all = list_sub_accounts(db, platform)?;
    Ok(all.into_iter().filter(|a| a.status == "active").collect())
}

pub fn get_sub_account(db: &Database, id: &str) -> Result<SubAccount> {
    let mut stmt = db.conn.prepare(
        "SELECT id, user_id, platform, external_id, display_name, avatar_url, status, metadata, created_at::VARCHAR, updated_at::VARCHAR FROM sub_accounts WHERE id = ?",
    )?;
    let account = stmt.query_row(params![id], |row| {
        let meta: String = row.get(7)?;
        Ok(SubAccount {
            id: row.get(0)?,
            user_id: row.get(1)?,
            platform: row.get(2)?,
            external_id: row.get(3)?,
            display_name: row.get(4)?,
            avatar_url: row.get(5)?,
            status: row.get(6)?,
            metadata: serde_json::from_str(&meta).unwrap_or(serde_json::json!({})),
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;
    Ok(account)
}

pub fn list_download_jobs(db: &Database) -> Result<Vec<DownloadJob>> {
    let mut stmt = db.conn.prepare(
        "SELECT job_id, status, progress, platform, match_id, map_name, played_at::VARCHAR, valve_channel, user_id, user_name, account_name, account_id, attempts, last_error, demo_url, share_code FROM v_download_dashboard ORDER BY played_at DESC NULLS LAST",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(DownloadJob {
            job_id: row.get(0)?,
            status: row.get(1)?,
            progress: row.get(2)?,
            platform: row.get(3)?,
            match_id: row.get(4)?,
            map_name: row.get(5)?,
            played_at: row.get(6)?,
            valve_channel: row.get(7)?,
            user_id: row.get(8)?,
            user_name: row.get(9)?,
            account_name: row.get(10)?,
            account_id: row.get(11)?,
            attempts: row.get(12)?,
            last_error: row.get(13)?,
            demo_url: row.get(14)?,
            share_code: row.get(15)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_settings(db: &Database) -> Result<AppSettings> {
    let row = db.conn.query_row(
        "SELECT data_root, global_max_concurrent, auto_discover_on_startup FROM app_settings WHERE id = 1",
        [],
        |row| {
            Ok(AppSettings {
                data_root: row.get(0)?,
                global_max_concurrent: row.get(1)?,
                auto_discover_on_startup: row.get(2)?,
            })
        },
    )?;
    Ok(row)
}

pub fn update_settings(db: &Database, settings: &AppSettings) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE app_settings SET data_root = ?, global_max_concurrent = ?, auto_discover_on_startup = ?, updated_at = ? WHERE id = 1",
        params![
            settings.data_root,
            settings.global_max_concurrent,
            settings.auto_discover_on_startup,
            now
        ],
    )?;
    Ok(())
}

pub fn list_platform_settings(db: &Database) -> Result<Vec<PlatformSettings>> {
    let mut stmt = db.conn.prepare(
        "SELECT platform, enabled, auto_discover, discover_interval_min, max_concurrent_jobs, auto_retry_failed, max_auto_attempts FROM platform_settings",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(PlatformSettings {
            platform: row.get(0)?,
            enabled: row.get(1)?,
            auto_discover: row.get(2)?,
            discover_interval_min: row.get(3)?,
            max_concurrent_jobs: row.get(4)?,
            auto_retry_failed: row.get(5)?,
            max_auto_attempts: row.get(6)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn update_platform_settings(db: &Database, p: &PlatformSettings) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE platform_settings SET enabled = ?, auto_discover = ?, discover_interval_min = ?, max_concurrent_jobs = ?, auto_retry_failed = ?, max_auto_attempts = ?, updated_at = ? WHERE platform = ?",
        params![
            p.enabled,
            p.auto_discover,
            p.discover_interval_min,
            p.max_concurrent_jobs,
            p.auto_retry_failed,
            p.max_auto_attempts,
            now,
            p.platform
        ],
    )?;
    Ok(())
}

pub fn share_code_exists(db: &Database, share_code: &str) -> Result<bool> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM demos WHERE share_code = ?",
        params![share_code],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn match_exists(db: &Database, platform: &str, match_id: &str) -> Result<bool> {
    let demo_count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM demos WHERE platform = ? AND match_id = ?",
        params![platform, match_id],
        |row| row.get(0),
    )?;
    if demo_count > 0 {
        return Ok(true);
    }
    let job_count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM download_jobs j JOIN discovered_matches dm ON dm.id = j.discovered_match_id WHERE dm.platform = ? AND dm.match_id = ? AND j.status IN ('queued','downloading','completed')",
        params![platform, match_id],
        |row| row.get(0),
    )?;
    Ok(job_count > 0)
}

pub fn upsert_discovered_match(
    db: &Database,
    sub_account_id: &str,
    platform: &str,
    m: &DiscoveredMatchPayload,
) -> Result<String> {
    let existing: Option<String> = db
        .conn
        .query_row(
            "SELECT id FROM discovered_matches WHERE platform = ? AND match_id = ?",
            params![platform, m.match_id],
            |row| row.get(0),
        )
        .ok();

    let raw = m.raw_payload.to_string();
    if let Some(id) = existing {
        db.conn.execute(
            "UPDATE discovered_matches SET demo_url = ?, map_name = ?, played_at = ?, share_code = ?, valve_channel = ?, raw_payload = ?, discovered_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![
                m.demo_url,
                m.map_name,
                m.played_at,
                m.share_code,
                m.valve_channel,
                raw,
                id
            ],
        )?;
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO discovered_matches (id, platform, sub_account_id, match_id, share_code, demo_url, map_name, played_at, valve_channel, raw_payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            id,
            platform,
            sub_account_id,
            m.match_id,
            m.share_code,
            m.demo_url,
            m.map_name,
            m.played_at,
            m.valve_channel,
            raw
        ],
    )?;
    Ok(id)
}

pub fn ensure_download_job(db: &Database, discovered_match_id: &str) -> Result<String> {
    let existing: Option<String> = db
        .conn
        .query_row(
            "SELECT id FROM download_jobs WHERE discovered_match_id = ?",
            params![discovered_match_id],
            |row| row.get(0),
        )
        .ok();
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO download_jobs (id, discovered_match_id, status, queued_at) VALUES (?, ?, 'queued', CURRENT_TIMESTAMP)",
        params![id, discovered_match_id],
    )?;
    Ok(id)
}

pub fn mark_job_skipped(db: &Database, job_id: &str) -> Result<()> {
    db.conn.execute(
        "UPDATE download_jobs SET status = 'skipped', finished_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![job_id],
    )?;
    Ok(())
}

pub fn retry_job(db: &Database, job_id: &str) -> Result<()> {
    db.conn.execute(
        "UPDATE download_jobs SET status = 'queued', last_error = NULL, failed_at = NULL, queued_at = CURRENT_TIMESTAMP WHERE id = ? AND status IN ('failed','expired')",
        params![job_id],
    )?;
    Ok(())
}

pub fn cancel_job(db: &Database, job_id: &str) -> Result<()> {
    db.conn.execute(
        "UPDATE download_jobs SET status = 'skipped', finished_at = CURRENT_TIMESTAMP WHERE id = ? AND status IN ('queued','downloading')",
        params![job_id],
    )?;
    Ok(())
}

pub fn requeue_failed_jobs(db: &Database, platform: &str) -> Result<usize> {
    let settings = list_platform_settings(db)?
        .into_iter()
        .find(|p| p.platform == platform)
        .unwrap_or(PlatformSettings {
            platform: platform.into(),
            enabled: true,
            auto_discover: true,
            discover_interval_min: 60,
            max_concurrent_jobs: 1,
            auto_retry_failed: true,
            max_auto_attempts: None,
        });

    if !settings.auto_retry_failed {
        return Ok(0);
    }

    let max_clause = settings
        .max_auto_attempts
        .map(|m| format!("AND j.attempts < {m}"))
        .unwrap_or_default();

    let sql = format!(
        "UPDATE download_jobs SET status = 'queued', attempts = attempts + 1, last_error = NULL, failed_at = NULL, queued_at = CURRENT_TIMESTAMP
         WHERE id IN (
           SELECT j.id FROM download_jobs j
           JOIN discovered_matches dm ON dm.id = j.discovered_match_id
           WHERE dm.platform = ? AND j.status IN ('failed','expired') {max_clause}
         )"
    );
    Ok(db.conn.execute(&sql, params![platform])?)
}

pub fn insert_demo(
    db: &Database,
    platform: &str,
    sub_account_id: &str,
    match_id: &str,
    share_code: Option<&str>,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    valve_channel: Option<&str>,
) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO demos (id, platform, sub_account_id, match_id, share_code, file_path, file_name, file_size, valve_channel) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            id,
            platform,
            sub_account_id,
            match_id,
            share_code,
            file_path,
            file_name,
            file_size,
            valve_channel
        ],
    )?;
    Ok(())
}

pub fn update_job_status(
    db: &Database,
    job_id: &str,
    status: &str,
    progress: Option<f64>,
    last_error: Option<&str>,
) -> Result<()> {
    match status {
        "downloading" => {
            db.conn.execute(
                "UPDATE download_jobs SET status = 'downloading', started_at = COALESCE(started_at, CURRENT_TIMESTAMP), progress = COALESCE(?, progress) WHERE id = ?",
                params![progress, job_id],
            )?;
        }
        "completed" => {
            db.conn.execute(
                "UPDATE download_jobs SET status = 'completed', progress = 1.0, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![job_id],
            )?;
        }
        "failed" => {
            db.conn.execute(
                "UPDATE download_jobs SET status = 'failed', last_error = ?, failed_at = CURRENT_TIMESTAMP, attempts = attempts + 1, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![last_error, job_id],
            )?;
        }
        "expired" => {
            db.conn.execute(
                "UPDATE download_jobs SET status = 'expired', last_error = ?, failed_at = CURRENT_TIMESTAMP, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![last_error, job_id],
            )?;
        }
        _ => {
            db.conn.execute(
                "UPDATE download_jobs SET status = ?, progress = COALESCE(?, progress) WHERE id = ?",
                params![status, progress, job_id],
            )?;
        }
    }
    Ok(())
}

pub fn fetch_queued_jobs(
    db: &Database,
    limit: i32,
) -> Result<Vec<(String, String, String, Option<String>, String, Option<String>, Option<String>, String)>> {
    let mut stmt = db.conn.prepare(
        "SELECT j.id, dm.platform, dm.match_id, dm.demo_url, sa.external_id, dm.share_code, dm.valve_channel, sa.id
         FROM download_jobs j
         JOIN discovered_matches dm ON dm.id = j.discovered_match_id
         JOIN sub_accounts sa ON sa.id = dm.sub_account_id
         WHERE j.status = 'queued'
         ORDER BY j.priority DESC, j.queued_at ASC
         LIMIT ?",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, String>(7)?,
        ))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
