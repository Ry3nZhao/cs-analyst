use super::models::*;
use super::{Database, DbError, Result};
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

pub fn list_users(db: &Database) -> Result<Vec<User>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, display_name, note, created_at, updated_at FROM users ORDER BY display_name",
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
        "SELECT id, user_id, platform, external_id, display_name, avatar_url, status, metadata, created_at, updated_at FROM sub_accounts WHERE platform = ? ORDER BY display_name",
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

pub fn delete_sub_account(db: &Database, id: &str) -> Result<()> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE sub_account_id = ?",
        params![id],
        |row| row.get(0),
    )?;
    if count > 0 {
        return Err(DbError::Message("账号已有关联的比赛记录，无法删除".into()));
    }
    db.conn.execute("DELETE FROM sub_accounts WHERE id = ?", params![id])?;
    Ok(())
}

pub fn update_sub_account_meta(
    db: &Database,
    id: &str,
    display_name: &str,
    metadata: serde_json::Value,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE sub_accounts SET display_name = ?, metadata = ?, updated_at = ? WHERE id = ?",
        params![display_name, metadata.to_string(), now, id],
    )?;
    Ok(())
}

pub fn list_active_sub_accounts(db: &Database, platform: &str) -> Result<Vec<SubAccount>> {
    let all = list_sub_accounts(db, platform)?;
    Ok(all.into_iter().filter(|a| a.status == "active").collect())
}

pub fn list_matches(db: &Database) -> Result<Vec<Match>> {
    let mut stmt = db.conn.prepare(
        "SELECT m.id, m.status, m.progress, m.platform, m.match_id,
                m.platform AS source,
                m.map_name, m.played_at, m.valve_channel,
                m.duration_seconds, m.game_mode, m.game_type,
                m.share_code, m.checksum, m.file_path, m.file_name, m.file_size,
                m.attempts, m.last_error, m.demo_url,
                m.demo_file_stamp, m.network_protocol, m.server_name, m.client_name,
                m.game_directory, m.fullpackets_version, m.allow_clientside_entities,
                m.demo_header_field_9, m.demo_header_field_10, m.demo_version_name,
                m.demo_version_guid, m.playback_time, m.playback_ticks, m.playback_frames,
                m.dem_game_info
         FROM matches m
         ORDER BY m.played_at DESC NULLS LAST, m.discovered_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let allow_clientside: Option<i32> = row.get(26)?;
        let dem_game_info_raw: Option<String> = row.get(34)?;
        let dem_game_info = dem_game_info_raw
            .as_deref()
            .filter(|s| !s.is_empty())
            .and_then(|s| serde_json::from_str(s).ok());
        Ok(Match {
            id: row.get(0)?,
            status: row.get(1)?,
            progress: row.get(2)?,
            platform: row.get(3)?,
            match_id: row.get(4)?,
            source: row.get(5)?,
            map_name: row.get(6)?,
            played_at: row.get(7)?,
            valve_channel: row.get(8)?,
            duration_seconds: row.get(9)?,
            game_mode: row.get(10)?,
            game_type: row.get(11)?,
            share_code: row.get(12)?,
            checksum: row.get(13)?,
            file_path: row.get(14)?,
            file_name: row.get(15)?,
            file_size: row.get(16)?,
            attempts: row.get(17)?,
            last_error: row.get(18)?,
            demo_url: row.get(19)?,
            demo_file_stamp: row.get(20)?,
            network_protocol: row.get(21)?,
            server_name: row.get(22)?,
            client_name: row.get(23)?,
            game_directory: row.get(24)?,
            fullpackets_version: row.get(25)?,
            allow_clientside_entities: allow_clientside.map(|v| v != 0),
            demo_header_field_9: row.get(27)?,
            demo_header_field_10: row.get(28)?,
            demo_version_name: row.get(29)?,
            demo_version_guid: row.get(30)?,
            playback_time: row.get(31)?,
            playback_ticks: row.get(32)?,
            playback_frames: row.get(33)?,
            dem_game_info,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Alias kept so orchestrator code compiles without changes.
#[allow(dead_code)]
pub fn list_download_jobs(db: &Database) -> Result<Vec<Match>> {
    list_matches(db)
}

pub fn get_settings(db: &Database) -> Result<AppSettings> {
    let mut settings = db.conn.query_row(
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
    settings.data_root = db.get_data_root();
    Ok(settings)
}

pub fn sync_data_root_setting(db: &Database) -> Result<()> {
    let effective = db.get_data_root();
    let stored: String = db.conn.query_row(
        "SELECT data_root FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    if stored != effective {
        let now = Utc::now().to_rfc3339();
        db.conn.execute(
            "UPDATE app_settings SET data_root = ?, updated_at = ? WHERE id = 1",
            params![effective, now],
        )?;
    }
    Ok(())
}

pub fn update_settings(db: &Database, settings: &AppSettings) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let max_concurrent = settings.global_max_concurrent.clamp(1, 10);
    db.conn.execute(
        "UPDATE app_settings SET data_root = ?, global_max_concurrent = ?, auto_discover_on_startup = ?, updated_at = ? WHERE id = 1",
        params![db.get_data_root(), max_concurrent, settings.auto_discover_on_startup, now],
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
    let max_concurrent_jobs = p.max_concurrent_jobs.clamp(1, 10);
    let max_auto_attempts = p.max_auto_attempts.filter(|attempts| *attempts > 0);
    db.conn.execute(
        "UPDATE platform_settings SET enabled = ?, auto_discover = ?, discover_interval_min = ?, max_concurrent_jobs = ?, auto_retry_failed = ?, max_auto_attempts = ?, updated_at = ? WHERE platform = ?",
        params![p.enabled, p.auto_discover, p.discover_interval_min, max_concurrent_jobs, p.auto_retry_failed, max_auto_attempts, now, p.platform],
    )?;
    Ok(())
}

pub fn share_code_exists(db: &Database, share_code: &str) -> Result<bool> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE share_code = ? AND status IN ('queued','downloading','completed')",
        params![share_code],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn match_exists(db: &Database, platform: &str, match_id: &str) -> Result<bool> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE platform = ? AND match_id = ? AND status IN ('queued','downloading','completed')",
        params![platform, match_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
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
            "SELECT id FROM matches WHERE platform = ? AND match_id = ?",
            params![platform, m.match_id],
            |row| row.get(0),
        )
        .ok();

    let raw = m.raw_payload.to_string();
    if let Some(id) = existing {
        db.conn.execute(
            "UPDATE matches SET demo_url = ?, map_name = ?, played_at = ?, share_code = ?, valve_channel = ?, raw_payload = ?, discovered_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![m.demo_url, m.map_name, m.played_at, m.share_code, m.valve_channel, raw, id],
        )?;
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO matches (id, platform, sub_account_id, match_id, share_code, demo_url, map_name, played_at, valve_channel, raw_payload, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'queued')",
        params![id, platform, sub_account_id, m.match_id, m.share_code, m.demo_url, m.map_name, m.played_at, m.valve_channel, raw],
    )?;
    Ok(id)
}

pub fn insert_imported_match(
    db: &Database,
    platform: &str,
    match_id: &str,
    map_name: Option<&str>,
    played_at: Option<&str>,
    duration_seconds: Option<i64>,
    game_mode: Option<&str>,
    game_type: Option<i64>,
    valve_channel: Option<&str>,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    checksum: Option<&str>,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO matches
           (id, platform, sub_account_id, match_id, map_name, played_at, duration_seconds, game_mode, game_type,
            valve_channel, raw_payload, status, progress, finished_at, file_path, file_name, file_size, checksum)
         VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, '{}', 'completed', 1.0, CURRENT_TIMESTAMP, ?, ?, ?, ?)",
        params![
            id,
            platform,
            match_id,
            map_name,
            played_at,
            duration_seconds,
            game_mode,
            game_type,
            valve_channel,
            file_path,
            file_name,
            file_size,
            checksum,
        ],
    )?;
    Ok(id)
}

pub fn should_skip_import(
    db: &Database,
    platform: &str,
    match_id: &str,
    checksum: Option<&str>,
) -> Result<bool> {
    if match_exists(db, platform, match_id)? {
        return Ok(true);
    }
    if let Some(cs) = checksum {
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM matches WHERE checksum = ?",
            params![cs],
            |row| row.get(0),
        )?;
        if count > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn get_match_platform(db: &Database, id: &str) -> Result<Option<String>> {
    Ok(db
        .conn
        .query_row("SELECT platform FROM matches WHERE id = ?", params![id], |row| {
            row.get(0)
        })
        .ok())
}

pub fn get_match_stored_match_id(db: &Database, id: &str) -> Result<Option<String>> {
    Ok(db
        .conn
        .query_row("SELECT match_id FROM matches WHERE id = ?", params![id], |row| {
            row.get(0)
        })
        .ok())
}

pub fn find_match_id_by_platform_excluding(
    db: &Database,
    platform: &str,
    match_id: &str,
    exclude_id: &str,
) -> Result<Option<String>> {
    Ok(db
        .conn
        .query_row(
            "SELECT id FROM matches WHERE platform = ? AND match_id = ? AND id != ?",
            params![platform, match_id, exclude_id],
            |row| row.get(0),
        )
        .ok())
}

pub fn target_accepts_imported_file(db: &Database, target_id: &str) -> Result<bool> {
    let row: Option<(Option<String>, String)> = db
        .conn
        .query_row(
            "SELECT file_path, status FROM matches WHERE id = ?",
            params![target_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    Ok(match row {
        None => false,
        Some((path, status)) => {
            path.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_none()
                || status != "completed"
        }
    })
}

pub fn reclassify_match_to_valve(
    db: &Database,
    id: &str,
    match_id: &str,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    checksum: Option<&str>,
) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET
           platform = 'valve',
           match_id = ?,
           valve_channel = 'local',
           sub_account_id = NULL,
           file_path = ?,
           file_name = ?,
           file_size = ?,
           checksum = COALESCE(?, checksum),
           status = 'completed',
           progress = 1.0,
           finished_at = COALESCE(finished_at, CURRENT_TIMESTAMP)
         WHERE id = ?",
        params![match_id, file_path, file_name, file_size, checksum, id],
    )?;
    Ok(())
}

pub fn attach_file_to_match(
    db: &Database,
    target_id: &str,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    checksum: Option<&str>,
) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET
           file_path = ?,
           file_name = ?,
           file_size = ?,
           checksum = COALESCE(?, checksum),
           status = 'completed',
           progress = 1.0,
           finished_at = COALESCE(finished_at, CURRENT_TIMESTAMP)
         WHERE id = ?",
        params![file_path, file_name, file_size, checksum, target_id],
    )?;
    Ok(())
}

pub fn delete_match_by_id(db: &Database, id: &str) -> Result<()> {
    db.conn
        .execute("DELETE FROM matches WHERE id = ?", params![id])?;
    Ok(())
}

pub fn get_match_abs_path(db: &Database, id: &str) -> Result<Option<std::path::PathBuf>> {
    let rel: Option<String> = db
        .conn
        .query_row(
            "SELECT file_path FROM matches WHERE id = ?",
            params![id],
            |row| row.get(0),
        )
        .ok();
    Ok(rel
        .filter(|s| !s.trim().is_empty())
        .map(|p| db.resolve_path(&p)))
}

pub fn get_sync_cursor(db: &Database, sub_account_id: &str, cursor_type: &str) -> Result<Option<String>> {
    Ok(db
        .conn
        .query_row(
            "SELECT cursor_value FROM sync_cursors WHERE sub_account_id = ? AND cursor_type = ?",
            params![sub_account_id, cursor_type],
            |row| row.get(0),
        )
        .ok())
}

pub fn upsert_sync_cursor(
    db: &Database,
    sub_account_id: &str,
    cursor_type: &str,
    cursor_value: &str,
) -> Result<()> {
    db.conn.execute(
        "INSERT INTO sync_cursors (sub_account_id, cursor_type, cursor_value, updated_at)
         VALUES (?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(sub_account_id) DO UPDATE SET
           cursor_type  = excluded.cursor_type,
           cursor_value = excluded.cursor_value,
           updated_at   = CURRENT_TIMESTAMP",
        params![sub_account_id, cursor_type, cursor_value],
    )?;
    Ok(())
}

pub fn retry_job(db: &Database, job_id: &str) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET status = 'queued', last_error = NULL, failed_at = NULL, queued_at = CURRENT_TIMESTAMP WHERE id = ? AND status IN ('failed','expired')",
        params![job_id],
    )?;
    Ok(())
}

pub fn cancel_job(db: &Database, job_id: &str) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET status = 'skipped', finished_at = CURRENT_TIMESTAMP WHERE id = ? AND status IN ('queued','downloading')",
        params![job_id],
    )?;
    Ok(())
}

pub fn is_job_cancelled(db: &Database, job_id: &str) -> Result<bool> {
    let status: Option<String> = db
        .conn
        .query_row(
            "SELECT status FROM matches WHERE id = ?",
            params![job_id],
            |row| row.get(0),
        )
        .ok();
    Ok(status.as_deref() == Some("skipped"))
}

pub fn reset_interrupted_downloads(db: &Database) -> Result<usize> {
    Ok(db.conn.execute(
        "UPDATE matches
         SET status     = 'queued',
             progress   = 0.0,
             last_error = '应用退出时下载中断，已自动重新排队',
             queued_at  = CURRENT_TIMESTAMP,
             started_at = NULL,
             finished_at = NULL
         WHERE status = 'downloading'",
        [],
    )?)
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
        .map(|m| format!("AND attempts < {m}"))
        .unwrap_or_default();

    // 只自动重试 'failed'：'expired' 表示 demo 已被 Valve 删除，重试永远不会成功，
    // 自动重排会让队列无限空转（用户手动点「重试」仍可强制排队）
    let sql = format!(
        "UPDATE matches SET status = 'queued', last_error = NULL, failed_at = NULL, queued_at = CURRENT_TIMESTAMP
         WHERE platform = ? AND status = 'failed' {max_clause}"
    );
    Ok(db.conn.execute(&sql, params![platform])?)
}

pub fn requeue_all_failed(db: &Database) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET status = 'queued', last_error = NULL, failed_at = NULL, queued_at = CURRENT_TIMESTAMP WHERE status IN ('failed','expired')",
        [],
    )?;
    Ok(())
}

pub fn update_match_file(
    db: &Database,
    id: &str,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    checksum: Option<&str>,
) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET file_path = ?, file_name = ?, file_size = ?, checksum = COALESCE(?, checksum) WHERE id = ?",
        params![file_path, file_name, file_size, checksum, id],
    )?;
    Ok(())
}

pub fn update_match_dem_metadata(
    db: &Database,
    id: &str,
    meta: &ParsedDemMetadata,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    overwrite: bool,
) -> Result<()> {
    let dem_game_info = meta
        .dem_game_info
        .as_ref()
        .map(|v| v.to_string());
    let allow_clientside = meta.allow_clientside_entities.map(|b| if b { 1 } else { 0 });

    if overwrite {
        db.conn.execute(
            "UPDATE matches SET
               file_path = ?,
               file_name = ?,
               file_size = ?,
               map_name = ?,
               played_at = ?,
               duration_seconds = ?,
               game_mode = ?,
               game_type = ?,
               checksum = ?,
               demo_file_stamp = ?,
               network_protocol = ?,
               server_name = ?,
               client_name = ?,
               game_directory = ?,
               fullpackets_version = ?,
               allow_clientside_entities = ?,
               demo_header_field_9 = ?,
               demo_header_field_10 = ?,
               demo_version_name = ?,
               demo_version_guid = ?,
               playback_time = ?,
               playback_ticks = ?,
               playback_frames = ?,
               dem_game_info = ?
             WHERE id = ?",
            params![
                file_path,
                file_name,
                file_size,
                meta.map_name,
                meta.played_at,
                meta.duration_seconds,
                meta.game_mode,
                meta.game_type,
                meta.checksum,
                meta.demo_file_stamp,
                meta.network_protocol,
                meta.server_name,
                meta.client_name,
                meta.game_directory,
                meta.fullpackets_version,
                allow_clientside,
                meta.demo_header_field_9,
                meta.demo_header_field_10,
                meta.demo_version_name,
                meta.demo_version_guid,
                meta.playback_time,
                meta.playback_ticks,
                meta.playback_frames,
                dem_game_info,
                id
            ],
        )?;
    } else {
        db.conn.execute(
            "UPDATE matches SET
               file_path = ?,
               file_name = ?,
               file_size = ?,
               map_name = COALESCE(?, map_name),
               played_at = COALESCE(?, played_at),
               duration_seconds = COALESCE(?, duration_seconds),
               game_mode = COALESCE(?, game_mode),
               game_type = COALESCE(?, game_type),
               checksum = COALESCE(?, checksum),
               demo_file_stamp = COALESCE(?, demo_file_stamp),
               network_protocol = COALESCE(?, network_protocol),
               server_name = COALESCE(?, server_name),
               client_name = COALESCE(?, client_name),
               game_directory = COALESCE(?, game_directory),
               fullpackets_version = COALESCE(?, fullpackets_version),
               allow_clientside_entities = COALESCE(?, allow_clientside_entities),
               demo_header_field_9 = COALESCE(?, demo_header_field_9),
               demo_header_field_10 = COALESCE(?, demo_header_field_10),
               demo_version_name = COALESCE(?, demo_version_name),
               demo_version_guid = COALESCE(?, demo_version_guid),
               playback_time = COALESCE(?, playback_time),
               playback_ticks = COALESCE(?, playback_ticks),
               playback_frames = COALESCE(?, playback_frames),
               dem_game_info = COALESCE(?, dem_game_info)
             WHERE id = ?",
            params![
                file_path,
                file_name,
                file_size,
                meta.map_name,
                meta.played_at,
                meta.duration_seconds,
                meta.game_mode,
                meta.game_type,
                meta.checksum,
                meta.demo_file_stamp,
                meta.network_protocol,
                meta.server_name,
                meta.client_name,
                meta.game_directory,
                meta.fullpackets_version,
                allow_clientside,
                meta.demo_header_field_9,
                meta.demo_header_field_10,
                meta.demo_version_name,
                meta.demo_version_guid,
                meta.playback_time,
                meta.playback_ticks,
                meta.playback_frames,
                dem_game_info,
                id
            ],
        )?;
    }
    Ok(())
}

pub fn list_completed_matches_with_files(db: &Database) -> Result<Vec<(String, String)>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, file_path FROM matches
         WHERE status = 'completed' AND file_path IS NOT NULL AND file_path != ''
         ORDER BY finished_at DESC NULLS LAST, discovered_at DESC",
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_matches_needing_metadata(db: &Database) -> Result<Vec<(String, String)>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, file_path FROM matches
         WHERE status = 'completed' AND file_path IS NOT NULL AND file_path != ''
           AND (map_name IS NULL OR map_name = '')
         ORDER BY finished_at DESC NULLS LAST, discovered_at DESC",
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
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
                "UPDATE matches SET status = 'downloading', started_at = COALESCE(started_at, CURRENT_TIMESTAMP), progress = COALESCE(?, progress) WHERE id = ?",
                params![progress, job_id],
            )?;
        }
        "completed" => {
            db.conn.execute(
                "UPDATE matches SET status = 'completed', progress = 1.0, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![job_id],
            )?;
        }
        "failed" => {
            db.conn.execute(
                "UPDATE matches SET status = 'failed', last_error = ?, failed_at = CURRENT_TIMESTAMP, attempts = attempts + 1, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![last_error, job_id],
            )?;
        }
        "expired" => {
            db.conn.execute(
                "UPDATE matches SET status = 'expired', last_error = ?, failed_at = CURRENT_TIMESTAMP, attempts = attempts + 1, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![last_error, job_id],
            )?;
        }
        _ => {
            db.conn.execute(
                "UPDATE matches SET status = ?, progress = COALESCE(?, progress) WHERE id = ?",
                params![status, progress, job_id],
            )?;
        }
    }
    Ok(())
}

pub fn fetch_queued_jobs(
    db: &Database,
    limit: i32,
) -> Result<Vec<(String, String, String, Option<String>, Option<String>, Option<String>)>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, platform, match_id, demo_url, share_code, valve_channel
         FROM matches
         WHERE status = 'queued'
         ORDER BY priority DESC, queued_at ASC
         LIMIT ?",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn count_queued_jobs(db: &Database) -> Result<usize> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE status = 'queued'",
        [],
        |row| row.get(0),
    )?;
    Ok(count.max(0) as usize)
}

pub fn list_valve_authcode_pending_resolve(
    db: &Database,
) -> Result<Vec<(String, String, String, Option<String>)>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, match_id, share_code, sub_account_id
         FROM matches
         WHERE platform = 'valve'
           AND valve_channel = 'authcode'
           AND status IN ('queued', 'failed', 'expired')
           AND share_code IS NOT NULL
           AND (demo_url IS NULL OR demo_url = '')
         ORDER BY queued_at ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_queued_valve_authcode_missing_demo_url(
    db: &Database,
) -> Result<Vec<(String, Option<String>)>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, share_code
         FROM matches
         WHERE platform = 'valve'
           AND valve_channel = 'authcode'
           AND status = 'queued'
           AND (demo_url IS NULL OR demo_url = '')",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn update_match_resolve_result(
    db: &Database,
    job_id: &str,
    demo_url: &str,
    played_at: Option<&str>,
    map_name: Option<&str>,
) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET demo_url = ?, played_at = COALESCE(?, played_at), map_name = COALESCE(?, map_name), progress = MAX(progress, 0.05), last_error = NULL WHERE id = ?",
        params![demo_url, played_at, map_name, job_id],
    )?;
    Ok(())
}

pub fn update_job_progress(db: &Database, job_id: &str, progress: f64) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET progress = ? WHERE id = ?",
        params![progress, job_id],
    )?;
    Ok(())
}

pub fn mark_match_expired(db: &Database, job_id: &str, error: &str) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET status = 'expired', demo_url = NULL, last_error = ?, failed_at = CURRENT_TIMESTAMP, attempts = attempts + 1, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![error, job_id],
    )?;
    Ok(())
}


fn read_valve_bot_row(
    db: &Database,
) -> Result<(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    bool,
)> {
    db.conn.query_row(
        "SELECT username, password_enc, secret_enc, steam_api_key_enc, refresh_token_enc, cron_schedule, run_on_startup FROM valve_bot_config WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get::<_, i32>(6)? != 0,
            ))
        },
    )
    .map_err(Into::into)
}

fn is_set(value: &Option<String>) -> bool {
    value.as_ref().is_some_and(|v| !v.is_empty())
}

fn secret_key(db: &Database) -> Vec<u8> {
    let mut key = b"cs-demo-analyst-local-secret".to_vec();
    key.extend_from_slice(db.get_data_root().as_bytes());
    key
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let s = std::str::from_utf8(chunk).ok()?;
        out.push(u8::from_str_radix(s, 16).ok()?);
    }
    Some(out)
}

fn protect_secret(db: &Database, value: Option<String>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return Some(value);
    }
    let key = secret_key(db);
    let encoded: Vec<u8> = value
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key[index % key.len()])
        .collect();
    Some(format!("local:v1:{}", encode_hex(&encoded)))
}

fn reveal_secret(db: &Database, value: Option<String>) -> Option<String> {
    let value = value?;
    let Some(encoded) = value.strip_prefix("local:v1:") else {
        return Some(value);
    };
    let bytes = decode_hex(encoded)?;
    let key = secret_key(db);
    let decoded: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key[index % key.len()])
        .collect();
    String::from_utf8(decoded).ok()
}

pub fn get_valve_bot_config(db: &Database) -> Result<ValveBotConfig> {
    let (username, password_enc, secret_enc, steam_api_key_enc, _, cron_schedule, run_on_startup) =
        read_valve_bot_row(db)?;
    let steam_api_key = reveal_secret(db, steam_api_key_enc.clone());
    Ok(ValveBotConfig {
        username,
        password_set: is_set(&password_enc),
        secret_set: is_set(&secret_enc),
        steam_api_key,
        cron_schedule,
        run_on_startup,
    })
}

pub fn get_valve_bot_secrets(db: &Database) -> Result<ValveBotSecrets> {
    let (username, password_enc, secret_enc, steam_api_key_enc, refresh_token_enc, _, _) =
        read_valve_bot_row(db)?;
    Ok(ValveBotSecrets {
        username,
        password: reveal_secret(db, password_enc),
        secret: reveal_secret(db, secret_enc),
        steam_api_key: reveal_secret(db, steam_api_key_enc),
        refresh_token: reveal_secret(db, refresh_token_enc),
    })
}

pub fn update_valve_bot_refresh_token(db: &Database, refresh_token: &str) -> Result<()> {
    let encrypted = protect_secret(db, Some(refresh_token.to_string()));
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE valve_bot_config SET refresh_token_enc = ?, updated_at = ? WHERE id = 1",
        params![encrypted, now],
    )?;
    Ok(())
}

pub fn update_valve_bot_config(db: &Database, update: &ValveBotConfigUpdate) -> Result<()> {
    let (cur_username, cur_password, cur_secret, cur_steam_api_key, cur_refresh_token, cur_cron, cur_run) =
        read_valve_bot_row(db)?;

    let username = update.username.clone().or(cur_username);
    let password_plain = match &update.password {
        None => cur_password,
        Some(value) if value.is_empty() => cur_password,
        Some(value) => Some(value.clone()),
    };
    let secret_plain = match &update.secret {
        None => cur_secret,
        Some(value) if value.is_empty() => cur_secret,
        Some(value) => Some(value.clone()),
    };
    let steam_api_key_plain = update.steam_api_key.clone().or(cur_steam_api_key);
    let password = protect_secret(db, reveal_secret(db, password_plain));
    let secret = protect_secret(db, reveal_secret(db, secret_plain));
    let steam_api_key = protect_secret(db, reveal_secret(db, steam_api_key_plain));
    let refresh_token = protect_secret(db, reveal_secret(db, cur_refresh_token));
    let cron_schedule = update.cron_schedule.clone().unwrap_or(cur_cron);
    let run_on_startup = update.run_on_startup.unwrap_or(cur_run);
    let now = Utc::now().to_rfc3339();

    db.conn.execute(
        "UPDATE valve_bot_config SET username = ?, password_enc = ?, secret_enc = ?, steam_api_key_enc = ?, refresh_token_enc = ?, cron_schedule = ?, run_on_startup = ?, updated_at = ? WHERE id = 1",
        params![username, password, secret, steam_api_key, refresh_token, cron_schedule, run_on_startup as i32, now],
    )?;
    Ok(())
}
