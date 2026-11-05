//! Replay / heatmap / radar commands. JSON shapes are mirrored in
//! `src/shared-types/src/replay.ts`. SteamIDs cross IPC as decimal strings.

use super::{heatmap, pack, radar};
use crate::database::{Database, DbError, Result};
use crate::AppState;
use demo_parse::Replay;
use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::ipc::Response;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayPlayer {
    /// Index into the frame columns.
    pub slot: usize,
    pub steamid: String,
    pub name: String,
    /// 2 = T, 3 = CT at the start of the match.
    pub team_start: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayRound {
    pub number: u32,
    pub start_tick: i32,
    pub freeze_end_tick: Option<i32>,
    pub end_tick: i32,
    pub winner: Option<u8>,
    pub reason: Option<String>,
    /// Score after this round, by side (from `team_round_stats`; `None` before stats exist).
    pub score_t: Option<u32>,
    pub score_ct: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayKill {
    pub tick: i32,
    pub round: u32,
    pub attacker: Option<String>,
    pub victim: Option<String>,
    pub assister: Option<String>,
    pub weapon: String,
    pub headshot: bool,
    pub penetrated: bool,
    pub noscope: bool,
    pub thrusmoke: bool,
    pub attacker_blind: bool,
    pub assisted_flash: bool,
    pub attacker_pos: Option<[f32; 3]>,
    pub victim_pos: Option<[f32; 3]>,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayGrenade {
    pub tick: i32,
    pub round: u32,
    pub thrower: Option<String>,
    /// `flashbang` | `hegrenade` | `smokegrenade` | `inferno` | `decoy` (detonation / burn start)
    pub kind: String,
    pub pos: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayBombEvent {
    pub tick: i32,
    pub round: u32,
    pub player: Option<String>,
    pub kind: String,
    pub site: Option<i64>,
    pub pos: Option<[f32; 3]>,
}

/// JSON header of the `get_replay` payload (see [`pack`] for the binary layout).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayHeader {
    pub match_id: String,
    pub map_name: Option<String>,
    pub platform: String,
    pub played_at: Option<String>,
    pub tick_rate: f32,
    pub tick_step: u16,
    pub n_frames: usize,
    pub n_players: usize,
    pub players: Vec<ReplayPlayer>,
    pub rounds: Vec<ReplayRound>,
    pub kills: Vec<ReplayKill>,
    pub grenades: Vec<ReplayGrenade>,
    pub bomb: Vec<ReplayBombEvent>,
    pub columns: pack::Columns,
}

pub const TICK_RATE: f32 = 64.0;

fn sid(v: Option<i64>) -> Option<String> {
    v.filter(|s| *s != 0).map(|s| (s as u64).to_string())
}

fn pos(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Option<[f32; 3]> {
    Some([x? as f32, y? as f32, z? as f32])
}

struct MatchRow {
    map_name: Option<String>,
    platform: String,
    played_at: Option<String>,
    replay_path: Option<String>,
}

/// Header (minus frame counts / columns) from the database.
fn load_events(db: &Database, match_id: &str) -> Result<(MatchRow, ReplayHeader)> {
    let c = &db.conn;
    let m = c
        .query_row(
            "SELECT map_name, platform, played_at, replay_path FROM matches WHERE id = ?1",
            [match_id],
            |r| Ok(MatchRow { map_name: r.get(0)?, platform: r.get(1)?, played_at: r.get(2)?, replay_path: r.get(3)? }),
        )
        .optional()?
        .ok_or_else(|| DbError::Message(format!("对局 {match_id} 不存在")))?;

    let mut st = c.prepare("SELECT steamid, slot, name, team_start FROM match_players WHERE match_id = ?1 ORDER BY slot")?;
    let players = st
        .query_map([match_id], |r| {
            Ok(ReplayPlayer {
                slot: r.get::<_, i64>(1)? as usize,
                steamid: (r.get::<_, i64>(0)? as u64).to_string(),
                name: r.get(2)?,
                team_start: r.get::<_, Option<i64>>(3)?.map(|t| t as u8),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let has_trs: bool = c
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='team_round_stats'", [], |r| r.get::<_, i64>(0))
        .map(|n| n > 0)?;
    let score_sql = if has_trs {
        "(SELECT score FROM team_round_stats t WHERE t.match_id = r.match_id AND t.round = r.number AND t.side = 2),
         (SELECT score FROM team_round_stats t WHERE t.match_id = r.match_id AND t.round = r.number AND t.side = 3)"
    } else {
        "NULL, NULL"
    };
    let mut st = c.prepare(&format!(
        "SELECT number, start_tick, freeze_end_tick, end_tick, winner, reason, {score_sql}
         FROM rounds r WHERE match_id = ?1 ORDER BY number"
    ))?;
    let rounds = st
        .query_map([match_id], |r| {
            Ok(ReplayRound {
                number: r.get(0)?,
                start_tick: r.get(1)?,
                freeze_end_tick: r.get(2)?,
                end_tick: r.get(3)?,
                winner: r.get::<_, Option<i64>>(4)?.map(|t| t as u8),
                reason: r.get(5)?,
                score_t: r.get(6)?,
                score_ct: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut st = c.prepare(
        "SELECT tick, round, attacker, victim, assister, weapon, headshot, penetrated, noscope, thrusmoke,
                attacker_blind, assisted_flash, attacker_x, attacker_y, attacker_z, victim_x, victim_y, victim_z,
                attacker_team, victim_team
         FROM kills WHERE match_id = ?1 ORDER BY tick",
    )?;
    let kills = st
        .query_map([match_id], |r| {
            Ok(ReplayKill {
                tick: r.get(0)?,
                round: r.get(1)?,
                attacker: sid(r.get(2)?),
                victim: sid(r.get(3)?),
                assister: sid(r.get(4)?),
                weapon: r.get(5)?,
                headshot: r.get(6)?,
                penetrated: r.get(7)?,
                noscope: r.get(8)?,
                thrusmoke: r.get(9)?,
                attacker_blind: r.get(10)?,
                assisted_flash: r.get(11)?,
                attacker_pos: pos(r.get(12)?, r.get(13)?, r.get(14)?),
                victim_pos: pos(r.get(15)?, r.get(16)?, r.get(17)?),
                attacker_team: r.get::<_, Option<i64>>(18)?.map(|t| t as u8),
                victim_team: r.get::<_, Option<i64>>(19)?.map(|t| t as u8),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut st = c.prepare("SELECT tick, round, thrower, kind, x, y, z FROM grenades WHERE match_id = ?1 ORDER BY tick")?;
    let grenades = st
        .query_map([match_id], |r| {
            Ok(ReplayGrenade {
                tick: r.get(0)?,
                round: r.get(1)?,
                thrower: sid(r.get(2)?),
                kind: r.get(3)?,
                pos: pos(r.get(4)?, r.get(5)?, r.get(6)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut st = c.prepare("SELECT tick, round, player, kind, site, x, y, z FROM bomb_events WHERE match_id = ?1 ORDER BY tick")?;
    let bomb = st
        .query_map([match_id], |r| {
            Ok(ReplayBombEvent {
                tick: r.get(0)?,
                round: r.get(1)?,
                player: sid(r.get(2)?),
                kind: r.get(3)?,
                site: r.get(4)?,
                pos: pos(r.get(5)?, r.get(6)?, r.get(7)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let header = ReplayHeader {
        match_id: match_id.to_string(),
        map_name: m.map_name.clone(),
        platform: m.platform.clone(),
        played_at: m.played_at.clone(),
        tick_rate: TICK_RATE,
        tick_step: 0,
        n_frames: 0,
        n_players: 0,
        players,
        rounds,
        kills,
        grenades,
        bomb,
        columns: Default::default(),
    };
    Ok((m, header))
}

/// Build the binary payload. DB work under the lock, file IO after it.
pub fn build_replay(db: &parking_lot::Mutex<Database>, match_id: &str) -> std::result::Result<Vec<u8>, String> {
    let (mut header, path) = {
        let db = db.lock();
        let (m, header) = load_events(&db, match_id).map_err(|e| e.to_string())?;
        let path = m.replay_path.as_deref().map(|p| db.resolve_path(p));
        (header, path)
    };
    let path = path.ok_or("这场对局还没有回放数据（需要完整解析）")?;
    let replay = Replay::read_from(&path).map_err(|e| format!("读取回放文件失败：{e}"))?;
    // Players come from the replay's slot order; names / start sides from match_players.
    let mut players = Vec::with_capacity(replay.n_players());
    for (slot, s) in replay.steamids.iter().enumerate() {
        let known = header.players.iter().find(|p| p.steamid == s.to_string());
        players.push(ReplayPlayer {
            slot,
            steamid: s.to_string(),
            name: known.map(|p| p.name.clone()).unwrap_or_else(|| s.to_string()),
            team_start: known.and_then(|p| p.team_start),
        });
    }
    header.players = players;
    header.tick_step = replay.tick_step;
    header.n_frames = replay.n_frames();
    header.n_players = replay.n_players();
    let (body, columns) = pack::body(&replay);
    header.columns = columns;
    let json = serde_json::to_vec(&header).map_err(|e| e.to_string())?;
    Ok(pack::pack(&json, &body))
}

#[tauri::command]
pub async fn get_replay(state: State<'_, AppState>, match_id: String) -> std::result::Result<Response, String> {
    let db = state.db.clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || build_replay(&db, &match_id))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn get_heatmap_points(
    state: State<'_, AppState>,
    filters: Option<heatmap::HeatmapFilters>,
) -> std::result::Result<heatmap::HeatmapResult, String> {
    let db = state.db.clone();
    let filters = filters.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || heatmap::query(&db, &filters))
        .await
        .map_err(|e| e.to_string())?
}

fn data_root(state: &State<'_, AppState>) -> std::path::PathBuf {
    state.db.lock().data_root.clone()
}

#[tauri::command]
pub async fn get_radar(state: State<'_, AppState>, map_name: String) -> std::result::Result<radar::RadarInfo, String> {
    let map = radar::sanitize_map(&map_name).ok_or_else(|| format!("无效的地图名：{map_name}"))?;
    let root = data_root(&state);
    tauri::async_runtime::spawn_blocking(move || radar::resolve(&root, &map)).await.map_err(|e| e.to_string())
}

/// Raw image bytes (PNG / JPEG / WebP) for `level` (`default` | `lower`).
#[tauri::command]
pub async fn get_radar_image(
    state: State<'_, AppState>,
    map_name: String,
    level: Option<String>,
) -> std::result::Result<Response, String> {
    let map = radar::sanitize_map(&map_name).ok_or_else(|| format!("无效的地图名：{map_name}"))?;
    let level = if level.as_deref() == Some("lower") { "lower" } else { "default" };
    let root = data_root(&state);
    let bytes = tauri::async_runtime::spawn_blocking(move || radar::image_bytes(&root, &map, level))
        .await
        .map_err(|e| e.to_string())?
        .ok_or("没有雷达图")?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub fn get_radar_settings(state: State<'_, AppState>) -> radar::RadarSettings {
    radar::load_settings(&data_root(&state))
}

#[tauri::command]
pub fn update_radar_settings(state: State<'_, AppState>, settings: radar::RadarSettings) -> std::result::Result<radar::RadarSettings, String> {
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let s = radar::RadarSettings { image_dir: clean(settings.image_dir), cs2_dir: clean(settings.cs2_dir), use_cs2: settings.use_cs2 };
    radar::save_settings(&data_root(&state), &s).map_err(|e| e.to_string())?;
    Ok(s)
}
