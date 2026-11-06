//! Persistence for parsed demo data (migration 002).

use super::{Database, Result};
use demo_parse::{ParsedDemo, Vec3};
use rusqlite::{params, Connection};

/// Failed analyses are retried automatically this many times (manual re-analysis resets the count).
pub const MAX_AUTO_ATTEMPTS: i32 = 2;

const ANALYSIS_TABLES: &[&str] = &[
    "match_players",
    "rounds",
    "kills",
    "damages",
    "shots",
    "grenades",
    "blinds",
    "bomb_events",
    "purchases",
    "economies",
];

fn sid(v: u64) -> i64 {
    v as i64
}

fn osid(v: Option<u64>) -> Option<i64> {
    v.map(sid)
}

fn xyz(v: Option<Vec3>) -> (Option<f32>, Option<f32>, Option<f32>) {
    match v {
        Some(p) => (Some(p.x), Some(p.y), Some(p.z)),
        None => (None, None, None),
    }
}

/// Completed downloads whose analysis is missing, interrupted, outdated or retryable.
pub fn list_matches_needing_analysis(db: &Database) -> Result<Vec<String>> {
    let mut stmt = db.conn.prepare(
        "SELECT id FROM matches
         WHERE status = 'completed' AND file_path IS NOT NULL AND file_path <> ''
           AND (analysis_status IS NULL
                OR analysis_status IN ('pending', 'running')
                OR (analysis_status = 'done' AND COALESCE(analyzer_version, 0) < ?1)
                OR (analysis_status = 'failed' AND analysis_attempts < ?2))
         ORDER BY played_at DESC NULLS LAST, discovered_at DESC",
    )?;
    let rows = stmt.query_map(params![demo_parse::PARSER_VERSION, MAX_AUTO_ATTEMPTS], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// All completed matches with a file (for "re-analyze everything").
pub fn list_analyzable_matches(db: &Database) -> Result<Vec<String>> {
    let mut stmt = db.conn.prepare(
        "SELECT id FROM matches WHERE status = 'completed' AND file_path IS NOT NULL AND file_path <> ''
         ORDER BY played_at DESC NULLS LAST, discovered_at DESC",
    )?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// True unless the match already has a current, successful analysis.
pub fn needs_analysis(db: &Database, id: &str) -> Result<bool> {
    let row: Option<(Option<String>, Option<i64>)> = db
        .conn
        .query_row("SELECT analysis_status, analyzer_version FROM matches WHERE id = ?", params![id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .ok();
    Ok(match row {
        Some((Some(status), version)) if status == "done" => version.unwrap_or(0) < demo_parse::PARSER_VERSION as i64,
        Some(_) => true,
        None => false,
    })
}

pub fn mark_analysis_pending(db: &Database, id: &str, reset_attempts: bool) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET analysis_status = 'pending', analysis_error = NULL,
           analysis_attempts = CASE WHEN ?2 THEN 0 ELSE analysis_attempts END
         WHERE id = ?1",
        params![id, reset_attempts],
    )?;
    Ok(())
}

pub fn mark_analysis_running(db: &Database, id: &str) -> Result<()> {
    db.conn.execute("UPDATE matches SET analysis_status = 'running' WHERE id = ?", params![id])?;
    Ok(())
}

pub fn mark_analysis_failed(db: &Database, id: &str, error: &str) -> Result<()> {
    db.conn.execute(
        "UPDATE matches SET analysis_status = 'failed', analysis_error = ?2,
           analysis_attempts = analysis_attempts + 1, analyzed_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        params![id, error],
    )?;
    Ok(())
}

/// Replace every analysis row of `match_id` with `demo`, in one transaction, and mark it done.
pub fn replace_match_analysis(
    db: &Database,
    match_id: &str,
    demo: &ParsedDemo,
    replay_rel_path: Option<&str>,
    elapsed_ms: i64,
) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    write_rows(&tx, match_id, demo)?;
    tx.execute(
        "UPDATE matches SET
           analysis_status = 'done', analysis_error = NULL, analysis_attempts = 0,
           analyzer_version = ?2, analyzed_at = CURRENT_TIMESTAMP, analysis_ms = ?3,
           replay_path = ?4, rounds_played = ?5,
           map_name = COALESCE(map_name, ?6)
         WHERE id = ?1",
        params![
            match_id,
            demo_parse::PARSER_VERSION,
            elapsed_ms,
            replay_rel_path,
            demo.rounds.len() as i64,
            demo.header.map_name,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn write_rows(c: &Connection, m: &str, d: &ParsedDemo) -> rusqlite::Result<()> {
    for t in ANALYSIS_TABLES {
        c.execute(&format!("DELETE FROM {t} WHERE match_id = ?"), params![m])?;
    }

    let mut st = c.prepare(
        "INSERT INTO match_players (match_id, steamid, slot, name, team_start, team_end, rank, rank_type)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for (slot, p) in d.players.iter().enumerate() {
        st.execute(params![m, sid(p.steamid), slot as i64, p.name, p.team_start, p.team_end, p.rank, p.rank_type])?;
    }

    let mut st = c.prepare(
        "INSERT INTO rounds (match_id, number, start_tick, freeze_end_tick, end_tick, winner, reason)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )?;
    for r in &d.rounds {
        st.execute(params![m, r.number, r.start_tick, r.freeze_end_tick, r.end_tick, r.winner, r.reason])?;
    }

    let mut st = c.prepare(
        "INSERT INTO kills (match_id, round, tick, attacker, victim, assister, weapon, headshot, penetrated,
           noscope, thrusmoke, attacker_blind, attacker_in_air, assisted_flash, distance,
           attacker_x, attacker_y, attacker_z, victim_x, victim_y, victim_z, attacker_team, victim_team)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for k in &d.kills {
        let (ax, ay, az) = xyz(k.attacker_pos);
        let (vx, vy, vz) = xyz(k.victim_pos);
        st.execute(params![
            m, k.round, k.tick, osid(k.attacker), osid(k.victim), osid(k.assister), k.weapon, k.headshot,
            k.penetrated, k.noscope, k.thrusmoke, k.attacker_blind, k.attacker_in_air, k.assisted_flash,
            k.distance, ax, ay, az, vx, vy, vz, k.attacker_team, k.victim_team
        ])?;
    }

    let mut st = c.prepare(
        "INSERT INTO damages (match_id, round, tick, attacker, victim, weapon, dmg_health, dmg_armor,
           health_after, armor_after, hitgroup, attacker_team, victim_team)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for x in &d.damages {
        st.execute(params![
            m, x.round, x.tick, osid(x.attacker), osid(x.victim), x.weapon, x.dmg_health, x.dmg_armor,
            x.health_after, x.armor_after, x.hitgroup, x.attacker_team, x.victim_team
        ])?;
    }

    let mut st = c.prepare(
        "INSERT INTO shots (match_id, round, tick, shooter, weapon, x, y, z, pitch, yaw, punch_pitch, punch_yaw,
           vel_x, vel_y, vel_z, shots_fired, accuracy_penalty, is_scoped, team)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for s in &d.shots {
        let (x, y, z) = xyz(s.pos);
        let (vx, vy, vz) = xyz(s.velocity);
        st.execute(params![
            m, s.round, s.tick, sid(s.shooter), s.weapon, x, y, z, s.pitch, s.yaw, s.punch_pitch, s.punch_yaw,
            vx, vy, vz, s.shots_fired, s.accuracy_penalty, s.is_scoped, s.team
        ])?;
    }

    let mut st = c.prepare(
        "INSERT INTO grenades (match_id, round, tick, thrower, kind, entity_id, x, y, z)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for g in &d.grenades {
        let (x, y, z) = xyz(g.pos);
        st.execute(params![m, g.round, g.tick, osid(g.thrower), g.kind, g.entity_id, x, y, z])?;
    }

    let mut st = c.prepare(
        "INSERT INTO blinds (match_id, round, tick, attacker, victim, duration, attacker_team, victim_team)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for b in &d.blinds {
        st.execute(params![m, b.round, b.tick, osid(b.attacker), osid(b.victim), b.duration, b.attacker_team, b.victim_team])?;
    }

    let mut st = c.prepare(
        "INSERT INTO bomb_events (match_id, round, tick, player, kind, site, x, y, z)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for b in &d.bombs {
        let (x, y, z) = xyz(b.pos);
        st.execute(params![m, b.round, b.tick, osid(b.player), b.kind, b.site, x, y, z])?;
    }

    let mut st = c.prepare(
        "INSERT INTO purchases (match_id, round, tick, player, item, cost, was_sold) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )?;
    for p in &d.purchases {
        st.execute(params![m, p.round, p.tick, sid(p.player), p.item, p.cost, p.was_sold])?;
    }

    let mut st = c.prepare(
        "INSERT OR REPLACE INTO economies (match_id, round, player, team, balance, equip_value,
           round_start_equip_value, cash_spent_this_round, armor, has_helmet, has_defuser)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for e in &d.economies {
        st.execute(params![
            m, e.round, sid(e.player), e.team, e.balance, e.equip_value, e.round_start_equip_value,
            e.cash_spent_this_round, e.armor, e.has_helmet, e.has_defuser
        ])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_db() -> (Database, PathBuf) {
        let dir = std::env::temp_dir().join(format!("cs-analyst-db-test-{}-{}", std::process::id(), uuid::Uuid::new_v4()));
        let db = Database::open(dir.clone()).expect("open db");
        (db, dir)
    }

    fn insert_match(db: &Database, id: &str) {
        db.conn
            .execute(
                "INSERT INTO matches (id, platform, match_id, status, file_path) VALUES (?, 'imported', ?, 'completed', 'demos/x.dem')",
                params![id, id],
            )
            .unwrap();
    }

    #[test]
    fn migration_and_queue_queries() {
        let (db, dir) = temp_db();
        let v: i32 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, crate::database::MIGRATIONS.last().unwrap().1);
        insert_match(&db, "m1");
        assert_eq!(list_matches_needing_analysis(&db).unwrap(), vec!["m1".to_string()]);

        for _ in 0..MAX_AUTO_ATTEMPTS {
            mark_analysis_failed(&db, "m1", "boom").unwrap();
        }
        assert!(list_matches_needing_analysis(&db).unwrap().is_empty(), "exhausted retries are not auto-queued");
        mark_analysis_pending(&db, "m1", true).unwrap();
        assert_eq!(list_matches_needing_analysis(&db).unwrap().len(), 1);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Full ingest of a real demo (skipped if the sample folder is missing).
    #[test]
    fn ingest_sample_demo() {
        let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_default();
        let path = PathBuf::from(root).join("Platforms/g161-n-20260709134236148390830.dem");
        if !path.exists() {
            eprintln!("skip: {} not found", path.display());
            return;
        }
        let demo = demo_parse::parse_demo_file(&path).unwrap();
        let (db, dir) = temp_db();
        insert_match(&db, "m1");
        replace_match_analysis(&db, "m1", &demo, Some("analysis/m1.replay.zst"), 1).unwrap();
        // Re-ingest must replace, not duplicate.
        replace_match_analysis(&db, "m1", &demo, Some("analysis/m1.replay.zst"), 1).unwrap();

        let count = |t: &str| -> i64 { db.conn.query_row(&format!("SELECT COUNT(*) FROM {t} WHERE match_id='m1'"), [], |r| r.get(0)).unwrap() };
        assert_eq!(count("rounds"), demo.rounds.len() as i64);
        assert_eq!(count("kills"), demo.kills.len() as i64);
        assert_eq!(count("shots"), demo.shots.len() as i64);
        assert_eq!(count("match_players"), 10);
        let (status, rounds): (String, i64) = db
            .conn
            .query_row("SELECT analysis_status, rounds_played FROM matches WHERE id='m1'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(status, "done");
        assert_eq!(rounds, 15);
        assert!(!needs_analysis(&db, "m1").unwrap(), "current analysis is not re-queued");
        assert!(!needs_analysis(&db, "nope").unwrap(), "unknown match is not queued");

        // Steam64 round-trips through INTEGER.
        let k: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM kills WHERE match_id='m1' AND attacker = ?1 AND victim <> ?1", [demo_parse::sample::PLAYER as i64], |r| r.get(0))
            .unwrap();
        assert_eq!(k, 15, "main player kills in this demo (suicides excluded)");

        // Deleting the match cascades.
        db.conn.execute("DELETE FROM matches WHERE id='m1'", []).unwrap();
        assert_eq!(count("kills"), 0);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
