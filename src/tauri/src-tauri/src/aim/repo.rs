//! Persistence for aim analysis (migration 005).

use crate::database::{Database, Result};
use ::aim::{AimReport, AIM_VERSION};
use rusqlite::{params, Connection};

const AIM_TABLES: &[&str] = &["player_match_aim", "aim_engagements", "sprays"];

fn sid(v: u64) -> i64 {
    v as i64
}

fn clear(c: &Connection, match_id: &str) -> rusqlite::Result<()> {
    for t in AIM_TABLES {
        // spray_shots go with their spray (ON DELETE CASCADE).
        c.execute(&format!("DELETE FROM {t} WHERE match_id = ?"), params![match_id])?;
    }
    Ok(())
}

/// Successfully parsed matches whose aim data is missing or from an older algorithm version.
pub fn list_matches_needing_aim(db: &Database) -> Result<Vec<String>> {
    let mut stmt = db.conn.prepare(
        "SELECT m.id FROM matches m LEFT JOIN match_aim a ON a.match_id = m.id
         WHERE m.analysis_status = 'done' AND m.file_path IS NOT NULL AND m.file_path <> ''
           AND (a.match_id IS NULL OR a.aim_version < ?1)
         ORDER BY m.played_at DESC NULLS LAST, m.discovered_at DESC",
    )?;
    let rows = stmt.query_map(params![AIM_VERSION], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn mark_aim_failed(db: &Database, match_id: &str, error: &str) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    clear(&tx, match_id)?;
    tx.execute(
        "INSERT OR REPLACE INTO match_aim (match_id, aim_version, status, error, analyzed_at)
         SELECT ?1, ?2, 'failed', ?3, CURRENT_TIMESTAMP WHERE EXISTS (SELECT 1 FROM matches WHERE id = ?1)",
        params![match_id, AIM_VERSION, error],
    )?;
    tx.commit()?;
    Ok(())
}

/// Replace every aim row of `match_id` with `r`, in one transaction.
pub fn replace_match_aim(db: &Database, match_id: &str, r: &AimReport, elapsed_ms: i64) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    clear(&tx, match_id)?;
    write_rows(&tx, match_id, r)?;
    tx.execute(
        "INSERT OR REPLACE INTO match_aim (match_id, aim_version, status, error, visibility_quality,
           visibility_reliable, analysis_ms, analyzed_at)
         VALUES (?, ?, 'done', NULL, ?, ?, ?, CURRENT_TIMESTAMP)",
        params![match_id, AIM_VERSION, r.visibility_quality, r.visibility_reliable, elapsed_ms],
    )?;
    tx.commit()?;
    Ok(())
}

fn write_rows(c: &Connection, m: &str, r: &AimReport) -> rusqlite::Result<()> {
    let mut st = c.prepare(
        "INSERT INTO player_match_aim (match_id, steamid, weapon, shots, hits, head_hits, vis_shots, vis_hits,
           first_shots, first_hits, cs_shots, cs_ok, ttd_n, ttd_median_ms, ttd_mean_ms, reaction_n,
           reaction_median_ms, reaction_mean_ms, cp_n, cp_median_deg, cp_mean_deg, cp_v_median_abs,
           cp_h_median_abs, cp_v_mean, sprays, spray_control, spray_err)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for p in &r.players {
        let c = &p.counters;
        st.execute(params![
            m, sid(p.steamid), p.weapon, c.shots, c.hits, c.head_hits, c.vis_shots, c.vis_hits, c.first_shots,
            c.first_hits, c.cs_shots, c.cs_ok, p.ttd.n, p.ttd.median, p.ttd.mean, p.reaction.n, p.reaction.median,
            p.reaction.mean, p.cp.n, p.cp.median, p.cp.mean, p.cp_v_abs.median, p.cp_h_abs.median, p.cp_v.mean,
            p.sprays, p.spray_control, p.spray_err
        ])?;
    }

    let mut st = c.prepare(
        "INSERT INTO aim_engagements (match_id, round, observer, target, role, weapon, onset_tick, damage_tick,
           ttd_ms, reaction_ms, cp_deg, cp_v, cp_h, distance, killed)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for e in &r.engagements {
        st.execute(params![
            m, e.round, sid(e.observer), sid(e.target), e.role.as_str(), e.weapon, e.onset_tick, e.damage_tick,
            e.ttd_ms, e.reaction_ms, e.cp_deg, e.cp_v, e.cp_h, e.distance, e.killed
        ])?;
    }

    let mut sp = c.prepare(
        "INSERT INTO sprays (match_id, steamid, round, team, weapon, start_tick, end_tick, shots, hits, head_hits,
           target, killed, start_speed, mean_err, mean_err_v, mean_err_h, control, break_shot)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    let mut ss = c.prepare(
        "INSERT INTO spray_shots (spray_id, idx, tick, pitch, yaw, punch_pitch, punch_yaw, recoil_v, recoil_h,
           aim_v, aim_h, err_v, err_h, hit, hitgroup, tgt_v, tgt_h)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for s in &r.sprays {
        sp.execute(params![
            m, sid(s.steamid), s.round, s.team, s.weapon, s.start_tick, s.end_tick, s.shots.len() as i64, s.hits,
            s.head_hits, s.target.map(sid), s.killed, s.start_speed, s.mean_err, s.mean_err_v, s.mean_err_h,
            s.control, s.break_shot
        ])?;
        let id = c.last_insert_rowid();
        for x in &s.shots {
            ss.execute(params![
                id, x.idx, x.tick, x.pitch, x.yaw, x.punch_pitch, x.punch_yaw, x.recoil_v, x.recoil_h, x.aim_v,
                x.aim_h, x.err_v, x.err_h, x.hit, x.hitgroup, x.tgt_v, x.tgt_h
            ])?;
        }
    }
    Ok(())
}
