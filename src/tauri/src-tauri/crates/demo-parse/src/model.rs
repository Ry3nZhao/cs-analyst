//! Normalized demo data. Team numbers follow the engine: 2 = T, 3 = CT.

use serde::Serialize;

pub const TEAM_T: u8 = 2;
pub const TEAM_CT: u8 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct ParsedDemo {
    pub header: DemoHeader,
    pub players: Vec<Player>,
    pub rounds: Vec<Round>,
    pub kills: Vec<Kill>,
    pub damages: Vec<Damage>,
    pub shots: Vec<Shot>,
    pub grenades: Vec<Grenade>,
    pub blinds: Vec<Blind>,
    pub bombs: Vec<BombEvent>,
    pub purchases: Vec<Purchase>,
    pub economies: Vec<Economy>,
    pub replay: crate::replay::Replay,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DemoHeader {
    pub map_name: Option<String>,
    pub server_name: Option<String>,
    pub client_name: Option<String>,
    pub demo_version_name: Option<String>,
    pub patch_version: Option<String>,
    /// Last tick seen in the demo.
    pub last_tick: i32,
    /// Server tick rate (CS2 SourceTV demos are 64).
    pub tick_rate: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    pub steamid: u64,
    pub name: String,
    /// Team at the first live round (2 = T, 3 = CT).
    pub team_start: Option<u8>,
    /// Team at the end of the demo, as reported by the scoreboard.
    pub team_end: Option<u8>,
    pub rank: Option<i32>,
    pub rank_type: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Round {
    /// 1-based round number.
    pub number: u32,
    pub start_tick: i32,
    pub freeze_end_tick: Option<i32>,
    pub end_tick: i32,
    /// Winning side (2 = T, 3 = CT).
    pub winner: Option<u8>,
    /// Engine reason string, e.g. `t_killed`, `bomb_exploded`.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Kill {
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
    pub assisted_flash: bool,
    pub distance: Option<f32>,
    pub attacker_pos: Option<Vec3>,
    pub victim_pos: Option<Vec3>,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Damage {
    pub round: u32,
    pub tick: i32,
    pub attacker: Option<u64>,
    pub victim: Option<u64>,
    pub weapon: String,
    pub dmg_health: i32,
    pub dmg_armor: i32,
    pub health_after: i32,
    pub armor_after: i32,
    pub hitgroup: Option<String>,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
}

/// One `weapon_fire`, joined with the shooter's state at that tick.
#[derive(Debug, Clone, Serialize)]
pub struct Shot {
    pub round: u32,
    pub tick: i32,
    pub shooter: u64,
    pub weapon: String,
    pub pos: Option<Vec3>,
    pub pitch: Option<f32>,
    pub yaw: Option<f32>,
    /// Aim punch (recoil) pitch/yaw from `CCSPlayer_AimPunchServices`.
    pub punch_pitch: Option<f32>,
    pub punch_yaw: Option<f32>,
    pub velocity: Option<Vec3>,
    /// `m_iShotsFired`: consecutive shots in the current spray (state at this tick).
    pub shots_fired: Option<i32>,
    pub accuracy_penalty: Option<f32>,
    pub is_scoped: Option<bool>,
    pub team: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Grenade {
    pub round: u32,
    pub tick: i32,
    pub thrower: Option<u64>,
    /// `flashbang` | `hegrenade` | `smokegrenade` | `inferno` | `decoy`
    pub kind: String,
    pub entity_id: Option<i32>,
    pub pos: Option<Vec3>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Blind {
    pub round: u32,
    pub tick: i32,
    pub attacker: Option<u64>,
    pub victim: Option<u64>,
    pub duration: f32,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BombEvent {
    pub round: u32,
    pub tick: i32,
    pub player: Option<u64>,
    /// `plant_begin` | `planted` | `defuse_begin` | `defused` | `exploded`
    pub kind: String,
    /// Raw bomb site entity index from the event.
    pub site: Option<i32>,
    pub pos: Option<Vec3>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Purchase {
    pub round: u32,
    pub tick: i32,
    pub player: u64,
    pub item: String,
    pub cost: Option<i32>,
    pub was_sold: bool,
}

/// Player state at freeze-time end of each round.
#[derive(Debug, Clone, Serialize)]
pub struct Economy {
    pub round: u32,
    pub player: u64,
    pub team: Option<u8>,
    pub balance: Option<i32>,
    pub equip_value: Option<i32>,
    pub round_start_equip_value: Option<i32>,
    pub cash_spent_this_round: Option<i32>,
    pub armor: Option<i32>,
    pub has_helmet: Option<bool>,
    pub has_defuser: Option<bool>,
}
