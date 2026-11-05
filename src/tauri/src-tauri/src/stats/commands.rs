//! Tauri commands for derived stats. JSON shapes are mirrored in `src/shared-types/src/stats.ts`.

use super::repo;
use crate::database::{Database, Result};
use crate::AppState;
use ::stats::{
    opt_steamid_str, steamid_str, KillEvent, MatchStats, PlayerMatchStats, PlayerRoundStats, Team, TeamRoundStats,
    STATS_VERSION,
};
use rusqlite::{params, params_from_iter, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetail {
    pub match_id: String,
    pub platform: String,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
    pub tick_rate: f32,
    pub rounds_played: u32,
    pub stats_version: i32,
    pub blind_data_available: bool,
    pub teams: Vec<TeamSummary>,
    pub scoreboard: Vec<PlayerMatchStats>,
    pub rounds: Vec<RoundDetail>,
    pub player_rounds: Vec<PlayerRoundStats>,
    /// Two rows per round (team A and B): equipment value, money, buy type, score.
    pub economy: Vec<TeamRoundStats>,
    pub damage_matrix: Vec<DamageCell>,
    pub duel_matrix: Vec<DuelCell>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub team: Team,
    pub start_side: u8,
    pub score: u32,
    /// `None` on a draw.
    pub won: Option<bool>,
    pub players: Vec<SteamId>,
}

/// Steam64 serialized as a string.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SteamId(#[serde(with = "steamid_str")] pub u64);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundDetail {
    pub number: u32,
    pub winner_side: Option<u8>,
    pub winner_team: Option<Team>,
    pub reason: Option<String>,
    pub side_a: u8,
    pub score_a: u32,
    pub score_b: u32,
    pub duration_secs: f32,
    pub start_tick: i32,
    pub freeze_end_tick: Option<i32>,
    pub end_tick: i32,
    pub bomb_planted: bool,
    pub bomb_site: Option<i32>,
    #[serde(with = "opt_steamid_str")]
    pub mvp: Option<u64>,
    pub kills: Vec<KillEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageCell {
    #[serde(with = "steamid_str")]
    pub attacker: u64,
    #[serde(with = "steamid_str")]
    pub victim: u64,
    pub damage: u32,
    pub hits: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuelCell {
    #[serde(with = "steamid_str")]
    pub attacker: u64,
    #[serde(with = "steamid_str")]
    pub victim: u64,
    pub kills: u32,
    pub headshot_kills: u32,
    pub opening_kills: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatsFilters {
    pub map_name: Option<String>,
    pub platform: Option<String>,
    /// Inclusive lower bound on `matches.played_at` (ISO string compare).
    pub since: Option<String>,
    /// Exclusive upper bound on `matches.played_at`.
    pub until: Option<String>,
    /// Max matches returned in `matches` (the summary covers all filtered matches). Default 50.
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatsList {
    #[serde(with = "steamid_str")]
    pub steamid: u64,
    /// Total matches matching the filters.
    pub total: u32,
    /// Aggregate over all filtered matches (rates recomputed from summed counts).
    pub summary: Option<PlayerMatchStats>,
    pub matches: Vec<PlayerMatchRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerMatchRow {
    pub match_id: String,
    pub platform: String,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
    /// Final score from this player's team's perspective.
    pub score_own: u32,
    pub score_opponent: u32,
    /// `win` | `loss` | `draw`
    pub result: &'static str,
    pub stats: PlayerMatchStats,
}

fn build_detail(db: &Database, match_id: &str) -> Result<Option<MatchDetail>> {
    let meta: Option<(String, Option<String>, Option<String>)> = db
        .conn
        .query_row("SELECT platform, map_name, played_at FROM matches WHERE id = ?", params![match_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?;
    let Some((platform, map_name, played_at)) = meta else { return Ok(None) };
    let ev = repo::load_events(db, match_id)?;
    if ev.rounds.is_empty() {
        return Ok(None);
    }
    let s: MatchStats = ::stats::compute(ev.view());
    // Self-heal: keep the stored rows in sync with what we just computed.
    if repo::stored_version(db, match_id)? != Some(STATS_VERSION) {
        repo::replace_match_stats(db, match_id, &s)?;
    }

    let start_side_a = s.rounds.first().map(|r| r.side_a).unwrap_or(::stats::TEAM_CT);
    let teams = [Team::A, Team::B]
        .into_iter()
        .map(|t| TeamSummary {
            team: t,
            start_side: if t == Team::A { start_side_a } else { 5 - start_side_a },
            score: if t == Team::A { s.score_a } else { s.score_b },
            won: s.winner.map(|w| w == t),
            players: s.players.iter().filter(|p| p.team == Some(t)).map(|p| SteamId(p.steamid)).collect(),
        })
        .collect();
    let rounds = s
        .rounds
        .iter()
        .map(|r| RoundDetail {
            number: r.number,
            winner_side: r.winner_side,
            winner_team: r.winner_team,
            reason: r.reason.clone(),
            side_a: r.side_a,
            score_a: r.score_a,
            score_b: r.score_b,
            duration_secs: r.duration_secs,
            start_tick: r.start_tick,
            freeze_end_tick: r.freeze_end_tick,
            end_tick: r.end_tick,
            bomb_planted: r.bomb_planted,
            bomb_site: r.bomb_site,
            mvp: r.mvp,
            kills: r.kills.clone(),
        })
        .collect();
    let damage_matrix = s
        .pairs
        .iter()
        .filter(|p| p.damage > 0 || p.hits > 0)
        .map(|p| DamageCell { attacker: p.attacker, victim: p.victim, damage: p.damage, hits: p.hits })
        .collect();
    let duel_matrix = s
        .pairs
        .iter()
        .filter(|p| p.kills > 0)
        .map(|p| DuelCell {
            attacker: p.attacker,
            victim: p.victim,
            kills: p.kills,
            headshot_kills: p.headshot_kills,
            opening_kills: p.opening_kills,
        })
        .collect();

    Ok(Some(MatchDetail {
        match_id: match_id.to_string(),
        platform,
        map_name,
        played_at,
        tick_rate: repo::DEFAULT_TICK_RATE,
        rounds_played: s.rounds.len() as u32,
        stats_version: s.version,
        blind_data_available: s.blind_data_available,
        teams,
        scoreboard: s.players,
        rounds,
        player_rounds: s.player_rounds,
        economy: s.team_rounds,
        damage_matrix,
        duel_matrix,
    }))
}

pub fn list_player_stats_impl(db: &Database, steamid: u64, f: &PlayerStatsFilters) -> Result<PlayerStatsList> {
    let mut wh = vec!["s.steamid = ?".to_string()];
    let mut args: Vec<rusqlite::types::Value> = vec![(steamid as i64).into()];
    if let Some(v) = f.map_name.as_ref().filter(|v| !v.is_empty()) {
        wh.push("m.map_name = ?".into());
        args.push(v.clone().into());
    }
    if let Some(v) = f.platform.as_ref().filter(|v| !v.is_empty()) {
        wh.push("m.platform = ?".into());
        args.push(v.clone().into());
    }
    if let Some(v) = f.since.as_ref().filter(|v| !v.is_empty()) {
        wh.push("m.played_at >= ?".into());
        args.push(v.clone().into());
    }
    if let Some(v) = f.until.as_ref().filter(|v| !v.is_empty()) {
        wh.push("m.played_at < ?".into());
        args.push(v.clone().into());
    }
    let n = repo::pms_column_count();
    let sql = format!(
        "SELECT {}, m.platform, m.map_name, m.played_at,
           (SELECT MAX(t.score) FROM team_round_stats t WHERE t.match_id = s.match_id AND t.team = s.team),
           (SELECT MAX(t.score) FROM team_round_stats t WHERE t.match_id = s.match_id AND t.team <> s.team)
         FROM player_match_stats s JOIN matches m ON m.id = s.match_id
         WHERE {}
         ORDER BY m.played_at DESC NULLS LAST, m.discovered_at DESC",
        repo::pms_select("s"),
        wh.join(" AND ")
    );
    let mut st = db.conn.prepare(&sql)?;
    let rows = st.query_map(params_from_iter(args.iter()), |r| {
        let stats = repo::row_to_player_stats(r, 0)?;
        let match_id: String = r.get(0)?;
        let own: Option<i64> = r.get(n + 3)?;
        let opp: Option<i64> = r.get(n + 4)?;
        let result = if stats.wins > 0 {
            "win"
        } else if stats.losses > 0 {
            "loss"
        } else {
            "draw"
        };
        Ok(PlayerMatchRow {
            match_id,
            platform: r.get(n)?,
            map_name: r.get(n + 1)?,
            played_at: r.get(n + 2)?,
            score_own: own.unwrap_or(0).max(0) as u32,
            score_opponent: opp.unwrap_or(0).max(0) as u32,
            result,
            stats,
        })
    })?;
    let all: Vec<PlayerMatchRow> = rows.collect::<rusqlite::Result<_>>()?;

    let summary = all.iter().fold(None::<PlayerMatchStats>, |acc, row| {
        Some(match acc {
            None => {
                let mut s = row.stats.clone();
                s.team = None;
                s.start_side = None;
                s
            }
            Some(mut s) => {
                s.add(&row.stats);
                s
            }
        })
    });
    let summary = summary.map(|mut s| {
        s.finalize();
        s
    });
    let offset = f.offset.unwrap_or(0) as usize;
    let limit = f.limit.unwrap_or(50) as usize;
    Ok(PlayerStatsList {
        steamid,
        total: all.len() as u32,
        summary,
        matches: all.into_iter().skip(offset).take(limit).collect(),
    })
}

/// Full match detail: scoreboard, round timeline (with kill feed), economy, damage / duel matrices.
#[tauri::command]
pub fn get_match_detail(state: State<'_, AppState>, match_id: String) -> std::result::Result<Option<MatchDetail>, String> {
    build_detail(&state.db.lock(), &match_id).map_err(|e| e.to_string())
}

/// One player's per-match stats (newest first) plus an aggregate over the filtered matches.
#[tauri::command]
pub fn list_player_stats(
    state: State<'_, AppState>,
    steamid: String,
    filters: Option<PlayerStatsFilters>,
) -> std::result::Result<PlayerStatsList, String> {
    let sid: u64 = steamid.trim().parse().map_err(|_| "无效的 SteamID".to_string())?;
    list_player_stats_impl(&state.db.lock(), sid, &filters.unwrap_or_default()).map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) fn build_detail_for_test(db: &Database, match_id: &str) -> Result<Option<MatchDetail>> {
    build_detail(db, match_id)
}
