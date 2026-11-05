//! Input rows (read back from the SQLite event tables) and the output of the rule engine.
//!
//! The input mirrors the stored tables, not `demo_parse::ParsedDemo`: highlights are computed from
//! what is already in the database, without parsing the demo again.

use serde::{Deserialize, Serialize};

pub const TEAM_T: u8 = 2;
pub const TEAM_CT: u8 = 3;

#[derive(Debug, Clone, Default)]
pub struct MatchInput {
    pub tick_rate: f32,
    pub rounds: Vec<RoundIn>,
    /// `player_round_stats` (+ `swing`).
    pub players: Vec<PlayerRoundIn>,
    pub kills: Vec<KillIn>,
    pub damages: Vec<DamageIn>,
    pub shots: Vec<ShotIn>,
    pub blinds: Vec<BlindIn>,
    pub bombs: Vec<BombIn>,
    /// `purchases`（`item` 是显示名，例如 `Flashbang`）
    pub purchases: Vec<PurchaseIn>,
    /// `round_win_probability` (Swing curve, migration 006).
    pub win_prob: Vec<WinProbIn>,
    /// `aim_engagements`; `None` when the aim analysis of this match is missing or failed.
    pub engagements: Option<Vec<EngagementIn>>,
    /// The demo has `player_blind` events (ESL demos do not).
    pub blind_data: bool,
}

#[derive(Debug, Clone)]
pub struct RoundIn {
    pub number: u32,
    pub start_tick: i32,
    pub freeze_end_tick: Option<i32>,
    pub end_tick: i32,
    pub winner: Option<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct PlayerRoundIn {
    pub round: u32,
    pub steamid: u64,
    pub side: u8,
    pub kills: i32,
    pub died: bool,
    pub death_tick: Option<i32>,
    pub opening_death: bool,
    pub traded: bool,
    pub clutch_vs: Option<i32>,
    pub clutch_won: Option<bool>,
    pub utility_thrown: i32,
    pub unused_utility_value: i32,
    pub swing: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct KillIn {
    pub round: u32,
    pub tick: i32,
    pub attacker: Option<u64>,
    pub victim: Option<u64>,
    pub assister: Option<u64>,
    pub weapon: String,
    pub headshot: bool,
    pub penetrated: bool,
    pub noscope: bool,
    pub thrusmoke: bool,
    pub attacker_blind: bool,
    pub attacker_in_air: bool,
    pub distance: Option<f32>,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct DamageIn {
    pub round: u32,
    pub tick: i32,
    pub attacker: Option<u64>,
    pub victim: Option<u64>,
    pub weapon: String,
    pub dmg_health: i32,
    pub health_after: i32,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct ShotIn {
    pub round: u32,
    pub tick: i32,
    pub shooter: u64,
    pub weapon: String,
}

#[derive(Debug, Clone, Default)]
pub struct BlindIn {
    pub round: u32,
    pub tick: i32,
    pub attacker: Option<u64>,
    pub victim: Option<u64>,
    pub duration: f32,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct BombIn {
    pub round: u32,
    pub tick: i32,
    pub player: Option<u64>,
    pub kind: String,
}

#[derive(Debug, Clone, Default)]
pub struct PurchaseIn {
    pub round: u32,
    pub tick: i32,
    pub player: u64,
    pub item: String,
    pub cost: Option<i32>,
    pub was_sold: bool,
}

#[derive(Debug, Clone, Default)]
pub struct WinProbIn {
    pub round: u32,
    pub seq: i32,
    pub tick: i32,
    pub kind: String,
    pub player: Option<u64>,
    pub victim: Option<u64>,
    pub t_alive: i32,
    pub ct_alive: i32,
    /// T win probability before / after the event (0–1).
    pub p_before: f64,
    pub p_after: f64,
}

#[derive(Debug, Clone, Default)]
pub struct EngagementIn {
    pub round: u32,
    pub observer: u64,
    pub target: u64,
    /// `attacker` | `victim`
    pub role: String,
    pub weapon: Option<String>,
    pub onset_tick: i32,
    pub damage_tick: i32,
    pub ttd_ms: Option<f64>,
    pub cp_deg: Option<f64>,
    pub killed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Highlight,
    Mistake,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Highlight => "highlight",
            Category::Mistake => "mistake",
        }
    }
}

/// One detected moment.
#[derive(Debug, Clone, Serialize)]
pub struct Highlight {
    pub round: u32,
    /// First / last tick of the moment (e.g. first and last kill of a 4K).
    pub start_tick: i32,
    pub end_tick: i32,
    pub steamid: u64,
    pub kind: crate::Kind,
    /// Raw rule score 0–100 (before the user's weight).
    pub score: f64,
    /// Rule-specific details; steamids are decimal strings.
    pub detail: serde_json::Value,
}
