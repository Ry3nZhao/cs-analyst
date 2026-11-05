//! `get_heatmap_points`: kill / death locations (from `kills`) and optional standing positions
//! (sampled from replay files), filtered by match, map, player, side, buy type and round, across
//! any number of matches. Returned column-wise to keep the JSON small.

use crate::database::Database;
use demo_parse::replay::{Replay, FLAG_ALIVE, FLAG_CT};
use parking_lot::Mutex;
use rusqlite::types::Value;
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const KIND_KILL: u8 = 0;
pub const KIND_DEATH: u8 = 1;
pub const KIND_PRESENCE: u8 = 2;
pub const DEFAULT_LIMIT: usize = 200_000;
/// Standing positions: one sample per player every this many ticks of live play.
pub const PRESENCE_EVERY_TICKS: i32 = 64;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeatmapFilters {
    pub match_ids: Option<Vec<String>>,
    pub maps: Option<Vec<String>>,
    /// Kill points use the attacker, death points the victim, presence the player.
    pub steamids: Option<Vec<String>>,
    /// 2 = T, 3 = CT — side of the player the point belongs to.
    pub side: Option<u8>,
    /// `pistol` | `eco` | `force` | `full` of that player's team in that round (`team_round_stats`).
    pub buy_types: Option<Vec<String>>,
    pub rounds: Option<Vec<u32>>,
    /// `kill` | `death` | `presence`; default kill + death.
    pub kinds: Option<Vec<String>>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatMatch {
    pub id: String,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapCount {
    pub map_name: Option<String>,
    pub points: usize,
}

/// Parallel arrays, one entry per point.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapResult {
    /// 0 kill, 1 death, 2 presence
    pub kind: Vec<u8>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub z: Vec<f32>,
    /// 2 = T, 3 = CT
    pub side: Vec<u8>,
    /// Index into `matches`.
    pub match_index: Vec<u32>,
    pub round: Vec<u16>,
    pub matches: Vec<HeatMatch>,
    /// Points per map (heatmaps only make sense one map at a time).
    pub maps: Vec<MapCount>,
    pub truncated: bool,
}

impl HeatmapResult {
    fn len(&self) -> usize {
        self.kind.len()
    }
    fn push(&mut self, kind: u8, p: [f32; 3], side: u8, mi: u32, round: u32) {
        self.kind.push(kind);
        self.x.push(p[0]);
        self.y.push(p[1]);
        self.z.push(p[2]);
        self.side.push(side);
        self.match_index.push(mi);
        self.round.push(round.min(u16::MAX as u32) as u16);
    }
}

fn ph(n: usize) -> String {
    vec!["?"; n].join(",")
}

fn steamids(f: &HeatmapFilters) -> Result<Option<Vec<i64>>, String> {
    f.steamids
        .as_ref()
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().map(|s| s.trim().parse::<u64>().map(|x| x as i64).map_err(|_| format!("无效的 steamid: {s}"))).collect())
        .transpose()
}

fn non_empty<T>(v: &Option<Vec<T>>) -> Option<&Vec<T>> {
    v.as_ref().filter(|v| !v.is_empty())
}

/// `WHERE` fragment + params on `matches m`.
fn match_conds(f: &HeatmapFilters, conds: &mut Vec<String>, args: &mut Vec<Value>) {
    if let Some(ids) = non_empty(&f.match_ids) {
        conds.push(format!("m.id IN ({})", ph(ids.len())));
        args.extend(ids.iter().map(|s| Value::Text(s.clone())));
    }
    if let Some(maps) = non_empty(&f.maps) {
        conds.push(format!("m.map_name IN ({})", ph(maps.len())));
        args.extend(maps.iter().map(|s| Value::Text(s.clone())));
    }
}

pub fn query(db: &Mutex<Database>, f: &HeatmapFilters) -> Result<HeatmapResult, String> {
    let kinds: HashSet<&str> = match non_empty(&f.kinds) {
        Some(k) => k.iter().map(String::as_str).collect(),
        None => ["kill", "death"].into_iter().collect(),
    };
    let limit = f.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, 2_000_000);
    let sids = steamids(f)?;
    let side = f.side.filter(|s| *s == 2 || *s == 3);
    let buy = non_empty(&f.buy_types).cloned();

    let mut out = HeatmapResult::default();
    let mut match_idx: HashMap<String, u32> = HashMap::new();
    let mut map_counts: BTreeMap<Option<String>, usize> = BTreeMap::new();

    // Matches in scope (also the presence source).
    struct M {
        id: String,
        map: Option<String>,
        played_at: Option<String>,
        replay_path: Option<std::path::PathBuf>,
    }
    let (all_matches, has_trs) = {
        let db = db.lock();
        let mut conds = vec!["m.analysis_status = 'done'".to_string()];
        let mut args = vec![];
        match_conds(f, &mut conds, &mut args);
        let sql = format!("SELECT m.id, m.map_name, m.played_at, m.replay_path FROM matches m WHERE {} ORDER BY m.played_at", conds.join(" AND "));
        let mut st = db.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params_from_iter(args), |r| {
                Ok(M { id: r.get(0)?, map: r.get(1)?, played_at: r.get(2)?, replay_path: r.get::<_, Option<String>>(3)?.map(|p| db.resolve_path(&p)) })
            })
            .and_then(|r| r.collect::<rusqlite::Result<Vec<_>>>())
            .map_err(|e| e.to_string())?;
        let has_trs = db
            .conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='team_round_stats'", [], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            > 0;
        (rows, has_trs)
    };
    let mut index_of = |out: &mut HeatmapResult, m: &M| -> u32 {
        *match_idx.entry(m.id.clone()).or_insert_with(|| {
            out.matches.push(HeatMatch { id: m.id.clone(), map_name: m.map.clone(), played_at: m.played_at.clone() });
            (out.matches.len() - 1) as u32
        })
    };
    let by_id: HashMap<&str, &M> = all_matches.iter().map(|m| (m.id.as_str(), m)).collect();
    if buy.is_some() && !has_trs {
        return Ok(out);
    }

    // Kill / death points.
    for (kind, who, team, px) in [(KIND_KILL, "attacker", "attacker_team", "attacker"), (KIND_DEATH, "victim", "victim_team", "victim")] {
        if !kinds.contains(if kind == KIND_KILL { "kill" } else { "death" }) || out.len() >= limit {
            continue;
        }
        let mut conds = vec![
            "m.analysis_status = 'done'".to_string(),
            "k.attacker IS NOT NULL AND k.victim IS NOT NULL AND k.attacker <> k.victim AND k.weapon <> 'world'".into(),
            "(k.attacker_team IS NULL OR k.victim_team IS NULL OR k.attacker_team <> k.victim_team)".into(),
            format!("k.{px}_x IS NOT NULL"),
        ];
        let mut args: Vec<Value> = vec![];
        match_conds(f, &mut conds, &mut args);
        if let Some(s) = &sids {
            conds.push(format!("k.{who} IN ({})", ph(s.len())));
            args.extend(s.iter().map(|v| Value::Integer(*v)));
        }
        if let Some(s) = side {
            conds.push(format!("k.{team} = ?"));
            args.push(Value::Integer(s as i64));
        }
        if let Some(r) = non_empty(&f.rounds) {
            conds.push(format!("k.round IN ({})", ph(r.len())));
            args.extend(r.iter().map(|v| Value::Integer(*v as i64)));
        }
        if let Some(b) = &buy {
            conds.push(format!(
                "EXISTS (SELECT 1 FROM team_round_stats t WHERE t.match_id = k.match_id AND t.round = k.round AND t.side = k.{team} AND t.buy_type IN ({}))",
                ph(b.len())
            ));
            args.extend(b.iter().map(|v| Value::Text(v.clone())));
        }
        let sql = format!(
            "SELECT k.match_id, k.round, k.{px}_x, k.{px}_y, COALESCE(k.{px}_z, 0), COALESCE(k.{team}, 0)
             FROM kills k JOIN matches m ON m.id = k.match_id WHERE {} ORDER BY m.played_at, k.tick LIMIT ?",
            conds.join(" AND ")
        );
        args.push(Value::Integer((limit - out.len() + 1) as i64));
        let rows: Vec<(String, u32, f64, f64, f64, i64)> = {
            let db = db.lock();
            let mut st = db.conn.prepare(&sql).map_err(|e| e.to_string())?;
            let rows = st
                .query_map(params_from_iter(args), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
                .and_then(|r| r.collect::<rusqlite::Result<Vec<_>>>())
                .map_err(|e| e.to_string())?;
            rows
        };
        for (mid, round, x, y, z, t) in rows {
            if out.len() >= limit {
                out.truncated = true;
                break;
            }
            let Some(m) = by_id.get(mid.as_str()) else { continue };
            let mi = index_of(&mut out, m);
            *map_counts.entry(m.map.clone()).or_default() += 1;
            out.push(kind, [x as f32, y as f32, z as f32], t as u8, mi, round);
        }
    }

    // Standing positions from replay files.
    if kinds.contains("presence") {
        for m in &all_matches {
            if out.len() >= limit {
                out.truncated = true;
                break;
            }
            let Some(path) = &m.replay_path else { continue };
            // Live part of each round, and the allowed (round, side) pairs for the buy filter.
            let (rounds, buy_ok): (Vec<(u32, i32, i32)>, Option<HashSet<(u32, u8)>>) = {
                let db = db.lock();
                let mut st = db
                    .conn
                    .prepare("SELECT number, COALESCE(freeze_end_tick, start_tick), end_tick FROM rounds WHERE match_id = ?1 ORDER BY number")
                    .map_err(|e| e.to_string())?;
                let rounds = st
                    .query_map([&m.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .and_then(|r| r.collect::<rusqlite::Result<Vec<_>>>())
                    .map_err(|e| e.to_string())?;
                let buy_ok = match &buy {
                    Some(b) => {
                        let sql = format!("SELECT round, side FROM team_round_stats WHERE match_id = ? AND buy_type IN ({})", ph(b.len()));
                        let mut args = vec![Value::Text(m.id.clone())];
                        args.extend(b.iter().map(|v| Value::Text(v.clone())));
                        let mut st = db.conn.prepare(&sql).map_err(|e| e.to_string())?;
                        let set = st
                            .query_map(params_from_iter(args), |r| Ok((r.get::<_, u32>(0)?, r.get::<_, i64>(1)? as u8)))
                            .and_then(|r| r.collect::<rusqlite::Result<HashSet<_>>>())
                            .map_err(|e| e.to_string())?;
                        Some(set)
                    }
                    None => None,
                };
                (rounds, buy_ok)
            };
            let Ok(rep) = Replay::read_from(path) else { continue };
            let slots: Vec<usize> = (0..rep.n_players())
                .filter(|&s| sids.as_ref().is_none_or(|v| v.contains(&(rep.steamids[s] as i64))))
                .collect();
            if slots.is_empty() {
                continue;
            }
            let every = (PRESENCE_EVERY_TICKS / rep.tick_step.max(1) as i32).max(1) as usize;
            let round_filter: Option<HashSet<u32>> = non_empty(&f.rounds).map(|r| r.iter().copied().collect());
            let mut ri = 0usize;
            let mi = index_of(&mut out, m);
            let mut added = 0usize;
            'frames: for fi in (0..rep.n_frames()).step_by(every) {
                let tick = rep.ticks[fi];
                while ri < rounds.len() && tick > rounds[ri].2 {
                    ri += 1;
                }
                let Some(&(number, live_from, _)) = rounds.get(ri) else { break };
                if tick < live_from || round_filter.as_ref().is_some_and(|r| !r.contains(&number)) {
                    continue;
                }
                for &s in &slots {
                    let i = rep.idx(fi, s);
                    let fl = rep.flags[i];
                    if fl & FLAG_ALIVE == 0 {
                        continue;
                    }
                    let ps = if fl & FLAG_CT != 0 { 3u8 } else { 2u8 };
                    if side.is_some_and(|sd| sd != ps) || buy_ok.as_ref().is_some_and(|b| !b.contains(&(number, ps))) {
                        continue;
                    }
                    if out.len() >= limit {
                        out.truncated = true;
                        break 'frames;
                    }
                    out.push(KIND_PRESENCE, [rep.x[i], rep.y[i], rep.z[i]], ps, mi, number);
                    added += 1;
                }
            }
            *map_counts.entry(m.map.clone()).or_default() += added;
        }
    }

    out.maps = map_counts.into_iter().filter(|(_, n)| *n > 0).map(|(map_name, points)| MapCount { map_name, points }).collect();
    out.maps.sort_by(|a, b| b.points.cmp(&a.points));
    Ok(out)
}
