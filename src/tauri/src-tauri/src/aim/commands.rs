//! Aim commands: `get_aim_stats`, `get_sprays`. JSON shapes are mirrored in
//! `src/shared-types/src/aim.ts`. SteamIDs cross IPC as decimal strings (beyond JS safe integers).

use crate::database::{Database, DbError, Result};
use crate::AppState;
use ::aim::{histogram, ratio, Dist, Spray, SprayShot, WeaponPattern, ALL_WEAPONS};
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Row};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::State;

/// Histogram bins handed to the frontend.
pub const TTD_BIN_MS: f32 = 50.0;
pub const TTD_BINS: usize = 40; // 0–2 s, last bin = overflow
pub const CP_BIN_DEG: f32 = 0.5;
pub const CP_BINS: usize = 40; // 0–20°, last bin = overflow
/// Without a match filter `get_sprays` lists at most this many (newest) sprays; patterns still
/// use all of them.
pub const MAX_LISTED_SPRAYS: usize = 500;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AimFilters {
    pub match_ids: Option<Vec<String>>,
    pub platforms: Option<Vec<String>>,
    pub maps: Option<Vec<String>>,
    /// Inclusive bounds on `matches.played_at` (ISO strings, compared as text).
    pub from: Option<String>,
    pub to: Option<String>,
    /// Only the newest N matches (after the other filters).
    pub last_matches: Option<u32>,
    /// Canonical weapon (e.g. `ak47`); default = all weapons.
    pub weapon: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Histogram {
    pub bin_width: f32,
    pub counts: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotRates {
    pub shots: u32,
    pub hits: u32,
    pub head_hits: u32,
    pub accuracy: Option<f32>,
    /// head hits / hits
    pub headshot_rate: Option<f32>,
    pub vis_shots: u32,
    pub vis_hits: u32,
    pub visible_accuracy: Option<f32>,
    pub first_shots: u32,
    pub first_hits: u32,
    pub first_shot_accuracy: Option<f32>,
    pub cs_shots: u32,
    pub cs_ok: u32,
    pub counter_strafe_rate: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AimOverall {
    #[serde(flatten)]
    pub rates: ShotRates,
    pub ttd: Dist,
    pub reaction: Dist,
    pub crosshair: Dist,
    /// Signed vertical offset (v > 0 crosshair above the head).
    pub crosshair_v: Dist,
    pub crosshair_v_abs: Dist,
    pub crosshair_h_abs: Dist,
    pub ttd_histogram: Histogram,
    pub crosshair_histogram: Histogram,
    pub sprays: u32,
    pub spray_control: Option<f32>,
    pub spray_err: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponAim {
    pub weapon: String,
    #[serde(flatten)]
    pub rates: ShotRates,
    pub ttd_median_ms: Option<f32>,
    pub crosshair_median_deg: Option<f32>,
    pub sprays: u32,
    pub spray_control: Option<f32>,
    pub spray_err: Option<f32>,
    pub break_shot: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AimTrendPoint {
    pub match_id: String,
    pub played_at: Option<String>,
    pub map_name: Option<String>,
    pub platform: String,
    pub visibility_reliable: bool,
    pub ttd_median_ms: Option<f32>,
    pub reaction_median_ms: Option<f32>,
    pub crosshair_median_deg: Option<f32>,
    pub accuracy: Option<f32>,
    pub counter_strafe_rate: Option<f32>,
    pub sprays: u32,
    pub spray_control: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AimStats {
    pub steamid: String,
    pub weapon: Option<String>,
    pub matches: u32,
    /// Matches whose visibility data is reliable (the only ones feeding TTD / crosshair / reaction
    /// / visible and first-shot accuracy).
    pub reliable_matches: u32,
    pub overall: AimOverall,
    /// Per weapon (all weapons, ignoring the weapon filter), most shots first.
    pub weapons: Vec<WeaponAim>,
    /// Oldest → newest.
    pub trend: Vec<AimTrendPoint>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SprayDto {
    pub id: i64,
    pub match_id: String,
    pub steamid: String,
    pub round: u32,
    pub team: Option<u8>,
    pub weapon: String,
    pub start_tick: i32,
    pub end_tick: i32,
    pub hits: u32,
    pub head_hits: u32,
    pub target: Option<String>,
    pub killed: bool,
    pub start_speed: Option<f32>,
    pub mean_err: f32,
    pub mean_err_v: f32,
    pub mean_err_h: f32,
    pub control: Option<f32>,
    pub break_shot: Option<u32>,
    pub shots: Vec<SprayShot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpraysResult {
    pub sprays: Vec<SprayDto>,
    /// Truncated to [`MAX_LISTED_SPRAYS`] newest sprays.
    pub truncated: bool,
    pub patterns: Vec<WeaponPattern>,
}

#[tauri::command]
pub fn get_aim_stats(state: State<'_, AppState>, steamid: String, filters: Option<AimFilters>) -> std::result::Result<AimStats, String> {
    let sid = parse_sid(&steamid)?;
    aim_stats(&state.db.lock(), sid, &filters.unwrap_or_default()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sprays(
    state: State<'_, AppState>,
    match_id: Option<String>,
    steamid: String,
    weapon: Option<String>,
) -> std::result::Result<SpraysResult, String> {
    let sid = parse_sid(&steamid)?;
    sprays(&state.db.lock(), match_id.as_deref(), sid, weapon.as_deref()).map_err(|e| e.to_string())
}

fn parse_sid(s: &str) -> std::result::Result<u64, String> {
    s.trim().parse().map_err(|_| format!("无效的 steamid: {s}"))
}

// ---------------------------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------------------------

struct MatchRow {
    id: String,
    played_at: Option<String>,
    map_name: Option<String>,
    platform: String,
    reliable: bool,
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

/// Matches with aim data for `steamid`, newest first, after filters.
fn select_matches(db: &Database, steamid: u64, f: &AimFilters) -> Result<Vec<MatchRow>> {
    let mut sql = String::from(
        "SELECT m.id, m.played_at, m.map_name, m.platform, a.visibility_reliable
         FROM player_match_aim p
         JOIN matches m ON m.id = p.match_id
         JOIN match_aim a ON a.match_id = p.match_id AND a.status = 'done'
         WHERE p.steamid = ? AND p.weapon = '*'",
    );
    let mut args: Vec<Value> = vec![Value::Integer(steamid as i64)];
    let mut list = |col: &str, vals: &Option<Vec<String>>, sql: &mut String| {
        if let Some(v) = vals {
            sql.push_str(&format!(" AND {col} IN ({})", placeholders(v.len().max(1))));
            if v.is_empty() {
                args.push(Value::Null);
            }
            args.extend(v.iter().cloned().map(Value::Text));
        }
    };
    list("m.id", &f.match_ids, &mut sql);
    list("m.platform", &f.platforms, &mut sql);
    list("m.map_name", &f.maps, &mut sql);
    if let Some(from) = &f.from {
        sql.push_str(" AND m.played_at >= ?");
        args.push(Value::Text(from.clone()));
    }
    if let Some(to) = &f.to {
        sql.push_str(" AND m.played_at <= ?");
        args.push(Value::Text(to.clone()));
    }
    sql.push_str(" ORDER BY COALESCE(m.played_at, m.discovered_at) DESC, m.id");
    if let Some(n) = f.last_matches {
        sql.push_str(&format!(" LIMIT {n}"));
    }
    let mut stmt = db.conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args), |r| {
        Ok(MatchRow {
            id: r.get(0)?,
            played_at: r.get(1)?,
            map_name: r.get(2)?,
            platform: r.get(3)?,
            reliable: r.get::<_, i64>(4)? != 0,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[derive(Default, Clone)]
struct Sums {
    shots: u32,
    hits: u32,
    head_hits: u32,
    vis_shots: u32,
    vis_hits: u32,
    first_shots: u32,
    first_hits: u32,
    cs_shots: u32,
    cs_ok: u32,
}

impl Sums {
    fn from_row(r: &Row, at: usize) -> rusqlite::Result<Self> {
        let g = |i: usize| -> rusqlite::Result<u32> { Ok(r.get::<_, Option<i64>>(at + i)?.unwrap_or(0) as u32) };
        Ok(Self {
            shots: g(0)?,
            hits: g(1)?,
            head_hits: g(2)?,
            vis_shots: g(3)?,
            vis_hits: g(4)?,
            first_shots: g(5)?,
            first_hits: g(6)?,
            cs_shots: g(7)?,
            cs_ok: g(8)?,
        })
    }
    fn add(&mut self, o: &Sums) {
        self.shots += o.shots;
        self.hits += o.hits;
        self.head_hits += o.head_hits;
        self.vis_shots += o.vis_shots;
        self.vis_hits += o.vis_hits;
        self.first_shots += o.first_shots;
        self.first_hits += o.first_hits;
        self.cs_shots += o.cs_shots;
        self.cs_ok += o.cs_ok;
    }
    fn rates(&self) -> ShotRates {
        ShotRates {
            shots: self.shots,
            hits: self.hits,
            head_hits: self.head_hits,
            accuracy: ratio(self.hits, self.shots),
            headshot_rate: ratio(self.head_hits, self.hits),
            vis_shots: self.vis_shots,
            vis_hits: self.vis_hits,
            visible_accuracy: ratio(self.vis_hits, self.vis_shots),
            first_shots: self.first_shots,
            first_hits: self.first_hits,
            first_shot_accuracy: ratio(self.first_hits, self.first_shots),
            cs_shots: self.cs_shots,
            cs_ok: self.cs_ok,
            counter_strafe_rate: ratio(self.cs_ok, self.cs_shots),
        }
    }
}

struct Sample {
    weapon: Option<String>,
    ttd: Option<f32>,
    reaction: Option<f32>,
    cp: Option<f32>,
    cp_v: Option<f32>,
    cp_h: Option<f32>,
}

pub fn aim_stats(db: &Database, steamid: u64, f: &AimFilters) -> Result<AimStats> {
    let matches = select_matches(db, steamid, f)?;
    let mut out = AimStats {
        steamid: steamid.to_string(),
        weapon: f.weapon.clone(),
        matches: matches.len() as u32,
        reliable_matches: matches.iter().filter(|m| m.reliable).count() as u32,
        ..Default::default()
    };
    if matches.is_empty() {
        return Ok(out);
    }
    let ids: Vec<Value> = matches.iter().map(|m| Value::Text(m.id.clone())).collect();
    let inlist = placeholders(ids.len());
    let with_sid = |extra: Vec<Value>| -> Vec<Value> {
        let mut v = vec![Value::Integer(steamid as i64)];
        v.extend(ids.iter().cloned());
        v.extend(extra);
        v
    };

    // Per-match, per-weapon rows.
    struct PmRow {
        match_id: String,
        weapon: String,
        sums: Sums,
        ttd_median: Option<f32>,
        reaction_median: Option<f32>,
        cp_median: Option<f32>,
        sprays: u32,
        spray_control: Option<f32>,
    }
    let mut stmt = db.conn.prepare(&format!(
        "SELECT match_id, weapon, shots, hits, head_hits, vis_shots, vis_hits, first_shots, first_hits, cs_shots, cs_ok,
                ttd_median_ms, reaction_median_ms, cp_median_deg, sprays, spray_control
         FROM player_match_aim WHERE steamid = ? AND match_id IN ({inlist})"
    ))?;
    let rows: Vec<PmRow> = stmt
        .query_map(params_from_iter(with_sid(vec![])), |r| {
            Ok(PmRow {
                match_id: r.get(0)?,
                weapon: r.get(1)?,
                sums: Sums::from_row(r, 2)?,
                ttd_median: r.get(11)?,
                reaction_median: r.get(12)?,
                cp_median: r.get(13)?,
                sprays: r.get::<_, i64>(14)? as u32,
                spray_control: r.get(15)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    let target_weapon = f.weapon.as_deref().unwrap_or(ALL_WEAPONS);

    // Contact samples from reliable matches.
    let mut stmt = db.conn.prepare(&format!(
        "SELECT e.weapon, e.ttd_ms, e.reaction_ms, e.cp_deg, e.cp_v, e.cp_h
         FROM aim_engagements e JOIN match_aim a ON a.match_id = e.match_id AND a.visibility_reliable = 1
         WHERE e.observer = ? AND e.match_id IN ({inlist})"
    ))?;
    let samples: Vec<Sample> = stmt
        .query_map(params_from_iter(with_sid(vec![])), |r| {
            Ok(Sample { weapon: r.get(0)?, ttd: r.get(1)?, reaction: r.get(2)?, cp: r.get(3)?, cp_v: r.get(4)?, cp_h: r.get(5)? })
        })?
        .filter_map(|r| r.ok())
        .collect();
    let picked: Vec<&Sample> = samples.iter().filter(|s| f.weapon.is_none() || s.weapon.as_deref() == f.weapon.as_deref()).collect();
    let dist = |g: fn(&Sample) -> Option<f32>| Dist::of(picked.iter().filter_map(|s| g(s)));

    // Sprays (for spray aggregates and break shot).
    let all_sprays = load_sprays(db, &format!("s.steamid = ? AND s.match_id IN ({inlist})"), with_sid(vec![]))?;

    let mut overall_sums = Sums::default();
    for r in rows.iter().filter(|r| r.weapon == target_weapon) {
        overall_sums.add(&r.sums);
    }
    let chosen: Vec<&Spray> = all_sprays.iter().map(|(_, s)| s).filter(|s| f.weapon.as_deref().is_none_or(|w| s.weapon == w)).collect();
    let ctrl: Vec<f32> = chosen.iter().filter_map(|s| s.control).collect();
    out.overall = AimOverall {
        rates: overall_sums.rates(),
        ttd: dist(|s| s.ttd),
        reaction: dist(|s| s.reaction),
        crosshair: dist(|s| s.cp),
        crosshair_v: dist(|s| s.cp_v),
        crosshair_v_abs: dist(|s| s.cp_v.map(f32::abs)),
        crosshair_h_abs: dist(|s| s.cp_h.map(f32::abs)),
        ttd_histogram: Histogram { bin_width: TTD_BIN_MS, counts: histogram(picked.iter().filter_map(|s| s.ttd), TTD_BIN_MS, TTD_BINS) },
        crosshair_histogram: Histogram { bin_width: CP_BIN_DEG, counts: histogram(picked.iter().filter_map(|s| s.cp), CP_BIN_DEG, CP_BINS) },
        sprays: chosen.len() as u32,
        spray_control: (!ctrl.is_empty()).then(|| ctrl.iter().sum::<f32>() / ctrl.len() as f32),
        spray_err: (!chosen.is_empty()).then(|| chosen.iter().map(|s| s.mean_err).sum::<f32>() / chosen.len() as f32),
    };

    // Per weapon.
    let mut by_weapon: BTreeMap<&str, Sums> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.weapon != ALL_WEAPONS) {
        by_weapon.entry(&r.weapon).or_default().add(&r.sums);
    }
    out.weapons = by_weapon
        .into_iter()
        .map(|(w, sums)| {
            let ws: Vec<&Sample> = samples.iter().filter(|s| s.weapon.as_deref() == Some(w)).collect();
            let sp: Vec<&Spray> = all_sprays.iter().map(|(_, s)| s).filter(|s| s.weapon == w).collect();
            let pat = ::aim::aggregate(w, sp.iter().copied());
            WeaponAim {
                weapon: w.to_string(),
                rates: sums.rates(),
                ttd_median_ms: Dist::of(ws.iter().filter_map(|s| s.ttd)).median,
                crosshair_median_deg: Dist::of(ws.iter().filter_map(|s| s.cp)).median,
                sprays: pat.sprays,
                spray_control: pat.control,
                spray_err: pat.mean_err,
                break_shot: pat.break_shot,
            }
        })
        .collect();
    out.weapons.sort_by(|a, b| b.rates.shots.cmp(&a.rates.shots).then(a.weapon.cmp(&b.weapon)));

    // Trend, oldest first.
    out.trend = matches
        .iter()
        .rev()
        .map(|m| {
            let row = rows.iter().find(|r| r.match_id == m.id && r.weapon == target_weapon);
            AimTrendPoint {
                match_id: m.id.clone(),
                played_at: m.played_at.clone(),
                map_name: m.map_name.clone(),
                platform: m.platform.clone(),
                visibility_reliable: m.reliable,
                ttd_median_ms: row.and_then(|r| r.ttd_median),
                reaction_median_ms: row.and_then(|r| r.reaction_median),
                crosshair_median_deg: row.and_then(|r| r.cp_median),
                accuracy: row.and_then(|r| ratio(r.sums.hits, r.sums.shots)),
                counter_strafe_rate: row.and_then(|r| ratio(r.sums.cs_ok, r.sums.cs_shots)),
                sprays: row.map(|r| r.sprays).unwrap_or(0),
                spray_control: row.and_then(|r| r.spray_control),
            }
        })
        .collect();
    Ok(out)
}

/// Sprays (with shots) matching `where_sql` on alias `s`, newest first, keyed by row id.
fn load_sprays(db: &Database, where_sql: &str, args: Vec<Value>) -> Result<Vec<(SprayMeta, Spray)>> {
    let mut stmt = db.conn.prepare(&format!(
        "SELECT s.id, s.match_id, s.steamid, s.round, s.team, s.weapon, s.start_tick, s.end_tick, s.hits, s.head_hits,
                s.target, s.killed, s.start_speed, s.mean_err, s.mean_err_v, s.mean_err_h, s.control, s.break_shot
         FROM sprays s JOIN matches m ON m.id = s.match_id
         WHERE {where_sql}
         ORDER BY COALESCE(m.played_at, m.discovered_at) DESC, s.match_id, s.start_tick DESC"
    ))?;
    let mut sprays: Vec<(SprayMeta, Spray)> = stmt
        .query_map(params_from_iter(args.clone()), |r| {
            Ok((
                SprayMeta { id: r.get(0)?, match_id: r.get(1)? },
                Spray {
                    steamid: r.get::<_, i64>(2)? as u64,
                    round: r.get(3)?,
                    team: r.get(4)?,
                    weapon: r.get(5)?,
                    start_tick: r.get(6)?,
                    end_tick: r.get(7)?,
                    hits: r.get(8)?,
                    head_hits: r.get(9)?,
                    target: r.get::<_, Option<i64>>(10)?.map(|v| v as u64),
                    killed: r.get(11)?,
                    start_speed: r.get(12)?,
                    mean_err: r.get(13)?,
                    mean_err_v: r.get(14)?,
                    mean_err_h: r.get(15)?,
                    control: r.get(16)?,
                    break_shot: r.get(17)?,
                    shots: vec![],
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    if sprays.is_empty() {
        return Ok(sprays);
    }
    let index: std::collections::HashMap<i64, usize> = sprays.iter().enumerate().map(|(i, (m, _))| (m.id, i)).collect();
    let mut stmt = db.conn.prepare(&format!(
        "SELECT x.spray_id, x.idx, x.tick, x.pitch, x.yaw, x.punch_pitch, x.punch_yaw, x.recoil_v, x.recoil_h,
                x.aim_v, x.aim_h, x.err_v, x.err_h, x.hit, x.hitgroup, x.tgt_v, x.tgt_h
         FROM spray_shots x JOIN sprays s ON s.id = x.spray_id JOIN matches m ON m.id = s.match_id
         WHERE {where_sql} ORDER BY x.spray_id, x.idx"
    ))?;
    let mut rows = stmt.query(params_from_iter(args))?;
    while let Some(r) = rows.next()? {
        let Some(&i) = index.get(&r.get::<_, i64>(0)?) else { continue };
        sprays[i].1.shots.push(SprayShot {
            idx: r.get(1)?,
            tick: r.get(2)?,
            pitch: r.get(3)?,
            yaw: r.get(4)?,
            punch_pitch: r.get(5)?,
            punch_yaw: r.get(6)?,
            recoil_v: r.get(7)?,
            recoil_h: r.get(8)?,
            aim_v: r.get(9)?,
            aim_h: r.get(10)?,
            err_v: r.get(11)?,
            err_h: r.get(12)?,
            hit: r.get(13)?,
            hitgroup: r.get(14)?,
            tgt_v: r.get(15)?,
            tgt_h: r.get(16)?,
        });
    }
    Ok(sprays)
}

struct SprayMeta {
    id: i64,
    match_id: String,
}

pub fn sprays(db: &Database, match_id: Option<&str>, steamid: u64, weapon: Option<&str>) -> Result<SpraysResult> {
    if let Some(w) = weapon {
        if !::aim::weapons::SPRAY_WEAPONS.contains(&w) {
            return Err(DbError::Message(format!("不支持压枪分析的武器: {w}")));
        }
    }
    let mut where_sql = String::from("s.steamid = ?");
    let mut args = vec![Value::Integer(steamid as i64)];
    if let Some(m) = match_id {
        where_sql.push_str(" AND s.match_id = ?");
        args.push(Value::Text(m.to_string()));
    }
    if let Some(w) = weapon {
        where_sql.push_str(" AND s.weapon = ?");
        args.push(Value::Text(w.to_string()));
    }
    let all = load_sprays(db, &where_sql, args)?;
    let patterns = ::aim::weapons::SPRAY_WEAPONS
        .iter()
        .filter(|w| weapon.is_none_or(|x| x == **w))
        .map(|w| ::aim::aggregate(w, all.iter().map(|(_, s)| s).filter(|s| s.weapon == *w)))
        .filter(|p| p.sprays > 0)
        .collect();
    let truncated = match_id.is_none() && all.len() > MAX_LISTED_SPRAYS;
    let mut list: Vec<SprayDto> = all
        .into_iter()
        .take(if match_id.is_none() { MAX_LISTED_SPRAYS } else { usize::MAX })
        .map(|(m, s)| SprayDto {
            id: m.id,
            match_id: m.match_id,
            steamid: s.steamid.to_string(),
            round: s.round,
            team: s.team,
            weapon: s.weapon,
            start_tick: s.start_tick,
            end_tick: s.end_tick,
            hits: s.hits,
            head_hits: s.head_hits,
            target: s.target.map(|t| t.to_string()),
            killed: s.killed,
            start_speed: s.start_speed,
            mean_err: s.mean_err,
            mean_err_v: s.mean_err_v,
            mean_err_h: s.mean_err_h,
            control: s.control,
            break_shot: s.break_shot,
            shots: s.shots,
        })
        .collect();
    if match_id.is_some() {
        // Within one match: chronological.
        list.sort_by_key(|s| s.start_tick);
    }
    Ok(SpraysResult { sprays: list, truncated, patterns })
}
