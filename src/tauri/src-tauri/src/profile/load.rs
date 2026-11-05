//! Profile queries. Everything is read from already-stored tables (003 / 005 / 006 / 002
//! `match_players`, `kills`); no demo is parsed again.

use super::compute::{AimRow, MatchRow};
use crate::database::Result;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::BTreeMap;

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

/// Steam identities of the person owning `steamid` (merges followed). Falls back to the steamid
/// itself when it has no identity yet.
pub fn person_steamids(c: &Connection, steamid: u64) -> Result<(Option<i64>, Vec<u64>)> {
    let person: Option<i64> = c
        .query_row(
            "SELECT person_id FROM identities WHERE platform = 'steam' AND steamid = ? LIMIT 1",
            params![steamid as i64],
            |r| r.get(0),
        )
        .optional()?;
    let Some(pid) = person else { return Ok((None, vec![steamid])) };
    let pid = crate::identity::query::resolve(c, pid)?;
    let mut ids: Vec<u64> = crate::identity::query::person_steamids(c, pid)?.into_iter().map(|s| s as u64).collect();
    if !ids.contains(&steamid) {
        ids.push(steamid);
    }
    ids.sort_unstable();
    Ok((Some(pid), ids))
}

/// Every player × match row of the library (or of the given steamids), oldest first.
pub fn player_rows(c: &Connection, only: Option<&[u64]>) -> Result<Vec<MatchRow>> {
    let mut sql = String::from(
        "SELECT s.match_id, s.steamid, m.map_name, NULL, COALESCE(m.played_at, m.discovered_at), s.won,
           (SELECT MAX(t.score) FROM team_round_stats t WHERE t.match_id = s.match_id AND t.team = s.team),
           (SELECT MAX(t.score) FROM team_round_stats t WHERE t.match_id = s.match_id AND t.team <> s.team),
           s.rounds_played, s.kills, s.deaths, s.headshot_kills, s.damage, s.kast_rounds, s.rating, s.swing_total,
           s.opening_kills_t + s.opening_kills_ct, s.opening_deaths_t + s.opening_deaths_ct,
           s.clutch_1v1_attempts + s.clutch_1v2_attempts + s.clutch_1v3_attempts + s.clutch_1v4_attempts + s.clutch_1v5_attempts,
           s.clutch_1v1_wins, s.clutch_1v2_wins, s.clutch_1v3_wins, s.clutch_1v4_wins, s.clutch_1v5_wins,
           s.mk2, s.mk3, s.mk4, s.mk5,
           ma.status, ma.visibility_reliable,
           a.ttd_n, a.ttd_median_ms, a.cp_n, a.cp_median_deg, a.cs_shots, a.cs_ok, a.sprays, a.spray_err,
           mp.rank, mp.rank_type
         FROM player_match_stats s
         JOIN matches m ON m.id = s.match_id
         LEFT JOIN match_aim ma ON ma.match_id = s.match_id
         LEFT JOIN player_match_aim a ON a.match_id = s.match_id AND a.steamid = s.steamid AND a.weapon = '*'
         LEFT JOIN match_players mp ON mp.match_id = s.match_id AND mp.steamid = s.steamid",
    );
    let mut args: Vec<Value> = Vec::new();
    if let Some(ids) = only {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        sql.push_str(&format!(" WHERE s.steamid IN ({})", placeholders(ids.len())));
        args.extend(ids.iter().map(|&s| Value::Integer(s as i64)));
    }
    sql.push_str(" ORDER BY COALESCE(m.played_at, m.discovered_at), m.id, s.steamid");
    let mut st = c.prepare(&sql)?;
    let rows = st.query_map(params_from_iter(args.iter()), |r| {
        let u = |i: usize| -> rusqlite::Result<u32> { Ok(r.get::<_, Option<i64>>(i)?.unwrap_or(0).max(0) as u32) };
        let won: Option<i64> = r.get(5)?;
        let clutch_wins = [u(19)?, u(20)?, u(21)?, u(22)?, u(23)?];
        let mk = [u(24)?, u(25)?, u(26)?, u(27)?];
        let aim_status: Option<String> = r.get(28)?;
        let aim = if aim_status.as_deref() == Some("done") {
            // A done match without a '*' row for this player = he never fired a gun.
            Some(AimRow {
                visibility_reliable: r.get::<_, Option<i64>>(29)?.unwrap_or(0) == 1,
                ttd_n: u(30)?,
                ttd_median_ms: r.get(31)?,
                cp_n: u(32)?,
                cp_median_deg: r.get(33)?,
                cs_shots: u(34)?,
                cs_ok: u(35)?,
                sprays: u(36)?,
                spray_err: r.get(37)?,
            })
        } else {
            None
        };
        Ok(MatchRow {
            match_id: r.get(0)?,
            steamid: r.get::<_, i64>(1)? as u64,
            map_name: r.get(2)?,
            at: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            won: won.map(|w| w == 1),
            score_own: u(6)?,
            score_opp: u(7)?,
            rounds: u(8)?,
            kills: u(9)?,
            deaths: u(10)?,
            headshot_kills: u(11)?,
            damage: u(12)?,
            kast_rounds: u(13)?,
            rating: r.get::<_, Option<f64>>(14)?.unwrap_or(0.0),
            swing_total: r.get(15)?,
            opening_kills: u(16)?,
            opening_deaths: u(17)?,
            clutch_attempts: u(18)?,
            clutch_wins: clutch_wins.iter().sum(),
            best_clutch_won: clutch_wins.iter().rposition(|&w| w > 0).map(|i| i as u32 + 1).unwrap_or(0),
            best_multi_kill: mk.iter().rposition(|&n| n > 0).map(|i| i as u32 + 2).unwrap_or(0),
            aim,
            rank: r.get(38)?,
            rank_type: r.get(39)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Per-side totals from `player_round_stats` (only rounds the player took part in).
#[derive(Debug, Clone, Default)]
pub struct SideTotals {
    pub rounds: u32,
    pub rounds_won: u32,
    pub kills: u32,
    pub deaths: u32,
    pub damage: u32,
    pub kast: u32,
    pub opening_kills: u32,
    pub opening_deaths: u32,
    pub swing: Option<f64>,
}

pub fn side_totals(c: &Connection, steamids: &[u64], match_ids: &[String]) -> Result<BTreeMap<u8, SideTotals>> {
    let mut out = BTreeMap::new();
    if steamids.is_empty() || match_ids.is_empty() {
        return Ok(out);
    }
    let sql = format!(
        "SELECT p.side, COUNT(*), SUM(COALESCE(t.won, 0)), SUM(p.kills), SUM(p.died), SUM(p.damage), SUM(p.kast),
                SUM(p.opening_kill), SUM(p.opening_death), SUM(p.swing)
         FROM player_round_stats p
         LEFT JOIN team_round_stats t ON t.match_id = p.match_id AND t.round = p.round AND t.team = p.team
         WHERE p.steamid IN ({}) AND p.match_id IN ({})
         GROUP BY p.side",
        placeholders(steamids.len()),
        placeholders(match_ids.len())
    );
    let args: Vec<Value> = steamids
        .iter()
        .map(|&s| Value::Integer(s as i64))
        .chain(match_ids.iter().map(|m| Value::Text(m.clone())))
        .collect();
    let mut st = c.prepare(&sql)?;
    let mut rows = st.query(params_from_iter(args.iter()))?;
    while let Some(r) = rows.next()? {
        let u = |i: usize| -> rusqlite::Result<u32> { Ok(r.get::<_, Option<i64>>(i)?.unwrap_or(0).max(0) as u32) };
        let side: i64 = r.get(0)?;
        out.insert(
            side as u8,
            SideTotals {
                rounds: u(1)?,
                rounds_won: u(2)?,
                kills: u(3)?,
                deaths: u(4)?,
                damage: u(5)?,
                kast: u(6)?,
                opening_kills: u(7)?,
                opening_deaths: u(8)?,
                swing: r.get(9)?,
            },
        );
    }
    Ok(out)
}

#[derive(Debug, Clone, Default)]
pub struct WeaponTotals {
    pub kills: u32,
    pub headshot_kills: u32,
    pub shots: u32,
    pub hits: u32,
    pub head_hits: u32,
}

/// Display key of a kill weapon: canonical gun name, `knife`, or the raw name (grenades, taser).
pub fn weapon_key(raw: &str) -> Option<String> {
    let s = raw.strip_prefix("weapon_").unwrap_or(raw);
    if s == "world" || s.is_empty() {
        return None;
    }
    if let Some(w) = ::aim::weapons::canonical(s) {
        return Some(w.name.to_string());
    }
    if s.contains("knife") || s.contains("bayonet") {
        return Some("knife".into());
    }
    Some(match s {
        "molotov" | "incgrenade" | "inferno" => "inferno".into(),
        other => other.to_string(),
    })
}

/// Kills (enemy kills only, before the last round's end) and aim shot counters per weapon.
pub fn weapon_totals(c: &Connection, steamids: &[u64], match_ids: &[String]) -> Result<BTreeMap<String, WeaponTotals>> {
    let mut out: BTreeMap<String, WeaponTotals> = BTreeMap::new();
    if steamids.is_empty() || match_ids.is_empty() {
        return Ok(out);
    }
    let args: Vec<Value> = steamids
        .iter()
        .map(|&s| Value::Integer(s as i64))
        .chain(match_ids.iter().map(|m| Value::Text(m.clone())))
        .collect();
    let sql = format!(
        "SELECT k.weapon, COUNT(*), SUM(k.headshot)
         FROM kills k
         WHERE k.attacker IN ({}) AND k.match_id IN ({})
           AND k.victim IS NOT NULL AND k.victim <> k.attacker
           AND (k.attacker_team IS NULL OR k.victim_team IS NULL OR k.attacker_team <> k.victim_team)
           AND k.tick <= COALESCE((SELECT MAX(r.end_tick) FROM rounds r WHERE r.match_id = k.match_id), k.tick)
         GROUP BY k.weapon",
        placeholders(steamids.len()),
        placeholders(match_ids.len())
    );
    let mut st = c.prepare(&sql)?;
    let mut rows = st.query(params_from_iter(args.iter()))?;
    while let Some(r) = rows.next()? {
        let raw: String = r.get(0)?;
        let Some(key) = weapon_key(&raw) else { continue };
        let e = out.entry(key).or_default();
        e.kills += r.get::<_, i64>(1)?.max(0) as u32;
        e.headshot_kills += r.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0) as u32;
    }
    let sql = format!(
        "SELECT weapon, SUM(shots), SUM(hits), SUM(head_hits) FROM player_match_aim
         WHERE weapon <> '*' AND steamid IN ({}) AND match_id IN ({}) GROUP BY weapon",
        placeholders(steamids.len()),
        placeholders(match_ids.len())
    );
    let mut st = c.prepare(&sql)?;
    let mut rows = st.query(params_from_iter(args.iter()))?;
    while let Some(r) = rows.next()? {
        let w: String = r.get(0)?;
        let e = out.entry(w).or_default();
        e.shots += r.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u32;
        e.hits += r.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0) as u32;
        e.head_hits += r.get::<_, Option<i64>>(3)?.unwrap_or(0).max(0) as u32;
    }
    Ok(out)
}
