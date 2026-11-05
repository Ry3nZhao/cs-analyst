//! CS2 demo parsing for CS Analyst.
//!
//! Two passes over the vendored demoparser:
//! 1. **events** (+ game-rules props) → rounds, live-play filtering, event lists;
//! 2. **ticks**, restricted to the ticks we need (every [`REPLAY_TICK_STEP`]th tick of live play
//!    plus every tick an event happens on) → per-shot shooter state, positions, teams, economy and
//!    sampled replay frames.
//!
//! Anything that needs every tick (TTD, crosshair placement…) re-parses the `.dem` later; parsing
//! is fast enough (~1 s per match) that storing full-rate tick data is not worth the disk.

pub mod engagement;
pub mod model;
pub mod replay;
pub mod source;

pub use engagement::{parse_engagements_bytes, parse_engagements_file, EngagementOptions, Engagements};
pub use model::*;
pub use replay::Replay;
pub use source::{is_zstd_file, parse_demo_zst, read_demo, DemoBytes};

use ahash::{AHashMap, AHashSet};
use parser::first_pass::parser_settings::{rm_user_friendly_names, ParserInputs};
use parser::first_pass::prop_controller::{NAME_ID, STEAMID_ID, TICK_ID};
use parser::parse_demo::{DemoOutput, Parser, ParsingMode};
use parser::second_pass::game_events::GameEvent;
use parser::second_pass::parser_settings::create_huffman_lookup_table;
use parser::second_pass::variants::{PropColumn, VarVec, Variant};
use std::path::Path;

/// Bump when the meaning of parsed output changes; stored per match so old rows can be re-analyzed.
pub const PARSER_VERSION: u32 = 1;
/// Replay frames are sampled every N ticks (64-tick demos → 16 frames/s).
pub const REPLAY_TICK_STEP: i32 = 4;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("demo parser: {0}")]
    Parser(String),
    #[error("demo has no playable rounds")]
    NoRounds,
}

const GAME_RULE_PROPS: &[&str] = &["total_rounds_played", "is_warmup_period", "is_match_started"];

const TICK_PROPS: &[&str] = &[
    "X",
    "Y",
    "Z",
    "pitch",
    "yaw",
    "velocity_X",
    "velocity_Y",
    "velocity_Z",
    "health",
    "armor_value",
    "is_alive",
    "team_num",
    "flash_duration",
    "approximate_spotted_by",
    "aim_punch_angle",
    "shots_fired",
    "accuracy_penalty",
    "is_scoped",
    "balance",
    "current_equip_value",
    "round_start_equip_value",
    "cash_spent_this_round",
    "has_helmet",
    "has_defuser",
    "rank",
    "CCSPlayerController.m_iCompetitiveRankType",
];

/// Parse a `.dem`, or a zstd-compressed `.dem.zst` (detected by magic bytes).
pub fn parse_demo_file(path: &Path) -> Result<ParsedDemo, ParseError> {
    let bytes = source::read_demo(path)?;
    parse_demo_bytes(&bytes)
}

pub fn parse_demo_bytes(bytes: &[u8]) -> Result<ParsedDemo, ParseError> {
    let huf = create_huffman_lookup_table();

    // ---------- pass 1: events ----------
    let ev_out = run_parser(bytes, &huf, vec!["all".into()], vec![], GAME_RULE_PROPS, vec![])?;
    let events: Vec<Ev> = ev_out.game_events.iter().map(Ev).collect();
    let header = build_header(&ev_out, &events);
    let timeline = Timeline::build(&events)?;

    // Which ticks the second pass must materialize.
    let first_tick = timeline.rounds[0].start_tick;
    let mut wanted: AHashSet<i32> = (first_tick..=header.last_tick)
        .step_by(REPLAY_TICK_STEP as usize)
        .collect();
    for e in &events {
        if timeline.round_of(e.0.tick).is_some() {
            wanted.insert(e.0.tick);
        }
    }
    for r in &timeline.rounds {
        wanted.insert(r.start_tick);
        if let Some(t) = r.freeze_end_tick {
            wanted.insert(t);
        }
    }

    // ---------- pass 2: ticks ----------
    let tick_out = run_parser(bytes, &huf, vec![], TICK_PROPS.to_vec(), &[], wanted.into_iter().collect())?;
    let ticks = TickTable::new(&tick_out);

    let players = build_players(&ev_out, &ticks, &timeline);
    let slots: AHashMap<u64, usize> = players.iter().enumerate().map(|(i, p)| (p.steamid, i)).collect();

    let mut demo = ParsedDemo {
        header,
        players,
        rounds: timeline.rounds.clone(),
        kills: vec![],
        damages: vec![],
        shots: vec![],
        grenades: vec![],
        blinds: vec![],
        bombs: vec![],
        purchases: vec![],
        economies: vec![],
        replay: Replay::default(),
    };

    for e in &events {
        let tick = e.0.tick;
        let Some(round) = timeline.round_of(tick) else { continue };
        let team = |sid: Option<u64>| sid.and_then(|s| ticks.team(tick, s));
        let pos = |sid: Option<u64>| sid.and_then(|s| ticks.pos(tick, s));
        match e.0.name.as_str() {
            "player_death" => {
                let (attacker, victim) = (e.sid("attacker_steamid"), e.sid("user_steamid"));
                demo.kills.push(Kill {
                    round,
                    tick,
                    attacker,
                    victim,
                    assister: e.sid("assister_steamid"),
                    weapon: weapon_name(e.str("weapon")),
                    headshot: e.bool("headshot"),
                    penetrated: e.int("penetrated").unwrap_or(0) > 0,
                    noscope: e.bool("noscope"),
                    thrusmoke: e.bool("thrusmoke"),
                    attacker_blind: e.bool("attackerblind"),
                    attacker_in_air: e.bool("attackerinair"),
                    assisted_flash: e.bool("assistedflash"),
                    distance: e.float("distance"),
                    attacker_pos: pos(attacker),
                    victim_pos: pos(victim),
                    attacker_team: team(attacker),
                    victim_team: team(victim),
                });
            }
            "player_hurt" => {
                let (attacker, victim) = (e.sid("attacker_steamid"), e.sid("user_steamid"));
                demo.damages.push(Damage {
                    round,
                    tick,
                    attacker,
                    victim,
                    weapon: weapon_name(e.str("weapon")),
                    dmg_health: e.int("dmg_health").unwrap_or(0) as i32,
                    dmg_armor: e.int("dmg_armor").unwrap_or(0) as i32,
                    health_after: e.int("health").unwrap_or(0) as i32,
                    armor_after: e.int("armor").unwrap_or(0) as i32,
                    hitgroup: e.str("hitgroup"),
                    attacker_team: team(attacker),
                    victim_team: team(victim),
                });
            }
            "weapon_fire" => {
                let Some(shooter) = e.sid("user_steamid") else { continue };
                let row = ticks.row(tick, shooter);
                let punch = row.and_then(|r| ticks.xyz("aim_punch_angle", r));
                demo.shots.push(Shot {
                    round,
                    tick,
                    shooter,
                    weapon: weapon_name(e.str("weapon")),
                    pos: ticks.pos(tick, shooter),
                    pitch: row.and_then(|r| ticks.f32("pitch", r)),
                    yaw: row.and_then(|r| ticks.f32("yaw", r)),
                    punch_pitch: punch.map(|p| p[0]),
                    punch_yaw: punch.map(|p| p[1]),
                    velocity: row.and_then(|r| {
                        Some(Vec3 {
                            x: ticks.f32("velocity_X", r)?,
                            y: ticks.f32("velocity_Y", r)?,
                            z: ticks.f32("velocity_Z", r)?,
                        })
                    }),
                    shots_fired: row.and_then(|r| ticks.i32("shots_fired", r)),
                    accuracy_penalty: row.and_then(|r| ticks.f32("accuracy_penalty", r)),
                    is_scoped: row.and_then(|r| ticks.bool("is_scoped", r)),
                    team: team(Some(shooter)),
                });
            }
            name @ ("flashbang_detonate" | "hegrenade_detonate" | "smokegrenade_detonate" | "inferno_startburn"
            | "decoy_started") => {
                let kind = match name {
                    "flashbang_detonate" => "flashbang",
                    "hegrenade_detonate" => "hegrenade",
                    "smokegrenade_detonate" => "smokegrenade",
                    "inferno_startburn" => "inferno",
                    _ => "decoy",
                };
                demo.grenades.push(Grenade {
                    round,
                    tick,
                    thrower: e.sid("user_steamid"),
                    kind: kind.into(),
                    entity_id: e.int("entityid").map(|v| v as i32),
                    pos: e.vec3("x", "y", "z"),
                });
            }
            "player_blind" => {
                let (attacker, victim) = (e.sid("attacker_steamid"), e.sid("user_steamid"));
                demo.blinds.push(Blind {
                    round,
                    tick,
                    attacker,
                    victim,
                    duration: e.float("blind_duration").unwrap_or(0.0),
                    attacker_team: team(attacker),
                    victim_team: team(victim),
                });
            }
            name @ ("bomb_beginplant" | "bomb_planted" | "bomb_begindefuse" | "bomb_defused" | "bomb_exploded") => {
                let kind = match name {
                    "bomb_beginplant" => "plant_begin",
                    "bomb_planted" => "planted",
                    "bomb_begindefuse" => "defuse_begin",
                    "bomb_defused" => "defused",
                    _ => "exploded",
                };
                let player = e.sid("user_steamid");
                demo.bombs.push(BombEvent {
                    round,
                    tick,
                    player,
                    kind: kind.into(),
                    site: e.int("site").map(|v| v as i32),
                    pos: pos(player),
                });
            }
            "item_purchase" => {
                let Some(player) = e.sid("steamid") else { continue };
                demo.purchases.push(Purchase {
                    round,
                    tick,
                    player,
                    item: e.str("item_name").unwrap_or_default(),
                    cost: e.int("cost").map(|v| v as i32),
                    was_sold: e.bool("was_sold"),
                });
            }
            _ => {}
        }
    }

    for r in &demo.rounds {
        let tick = r.freeze_end_tick.unwrap_or(r.start_tick);
        for p in &demo.players {
            let Some(row) = ticks.row(tick, p.steamid) else { continue };
            demo.economies.push(Economy {
                round: r.number,
                player: p.steamid,
                team: ticks.team_at_row(row),
                balance: ticks.i32("balance", row),
                equip_value: ticks.i32("current_equip_value", row),
                round_start_equip_value: ticks.i32("round_start_equip_value", row),
                cash_spent_this_round: ticks.i32("cash_spent_this_round", row),
                armor: ticks.i32("armor_value", row),
                has_helmet: ticks.bool("has_helmet", row),
                has_defuser: ticks.bool("has_defuser", row),
            });
        }
    }

    demo.replay = build_replay(&ticks, &demo.players, &slots, first_tick, demo.header.last_tick);
    Ok(demo)
}

fn run_parser(
    bytes: &[u8],
    huf: &Vec<(u8, u8)>,
    wanted_events: Vec<String>,
    player_props: Vec<&str>,
    other_props: &[&str],
    wanted_ticks: Vec<i32>,
) -> Result<DemoOutput, ParseError> {
    let player_og: Vec<String> = player_props.iter().map(|s| s.to_string()).collect();
    let other_og: Vec<String> = other_props.iter().map(|s| s.to_string()).collect();
    let player_real = rm_user_friendly_names(&player_og).map_err(|e| ParseError::Parser(format!("{e:?}")))?;
    let other_real = rm_user_friendly_names(&other_og).map_err(|e| ParseError::Parser(format!("{e:?}")))?;
    let mut real_name_to_og_name = AHashMap::default();
    for (r, o) in player_real.iter().zip(&player_og).chain(other_real.iter().zip(&other_og)) {
        real_name_to_og_name.insert(r.clone(), o.clone());
    }
    let inputs = ParserInputs {
        real_name_to_og_name,
        wanted_players: vec![],
        wanted_player_props: player_real,
        wanted_other_props: other_real,
        wanted_events,
        wanted_prop_states: AHashMap::default(),
        parse_ents: true,
        wanted_ticks,
        parse_projectiles: false,
        parse_grenades: false,
        only_header: true,
        list_props: false,
        only_convars: false,
        huffman_lookup_table: huf,
        order_by_steamid: false,
        fallback_bytes: None,
    };
    Parser::new(inputs, ParsingMode::Normal)
        .parse_demo(bytes)
        .map_err(|e| ParseError::Parser(format!("{e:?}")))
}

fn build_header(out: &DemoOutput, events: &[Ev]) -> DemoHeader {
    let h = |k: &str| out.header.as_ref().and_then(|m| m.get(k)).filter(|s| !s.is_empty()).cloned();
    DemoHeader {
        map_name: h("map_name"),
        server_name: h("server_name"),
        client_name: h("client_name"),
        demo_version_name: h("demo_version_name"),
        patch_version: h("patch_version"),
        last_tick: events.iter().map(|e| e.0.tick).max().unwrap_or(0),
        tick_rate: 64.0,
    }
}

fn weapon_name(raw: Option<String>) -> String {
    let w = raw.unwrap_or_default();
    w.strip_prefix("weapon_").map(str::to_string).unwrap_or(w)
}

// ---------------------------------------------------------------------------------------------
// Rounds / live-play timeline
// ---------------------------------------------------------------------------------------------

struct Timeline {
    rounds: Vec<Round>,
    /// Tick ranges (inclusive start, exclusive end) belonging to rounds that were later replayed
    /// (match restart or backup restore); events inside are dropped.
    dead: Vec<(i32, i32)>,
}

impl Timeline {
    fn build(events: &[Ev]) -> Result<Self, ParseError> {
        let live = |e: &&Ev| e.opt_bool("is_warmup_period") != Some(true);
        let starts: Vec<i32> = events.iter().filter(live).filter(|e| e.0.name == "round_start").map(|e| e.0.tick).collect();
        let freezes: Vec<i32> =
            events.iter().filter(live).filter(|e| e.0.name == "round_freeze_end").map(|e| e.0.tick).collect();

        let mut rounds: Vec<Round> = vec![];
        let mut dead = vec![];
        let mut ends: Vec<&Ev> = events.iter().filter(live).filter(|e| e.0.name == "round_end").collect();
        ends.sort_by_key(|e| e.0.tick);
        for e in ends {
            let winner = match e.str("winner").as_deref() {
                Some("T") => TEAM_T,
                Some("CT") => TEAM_CT,
                _ => continue, // tick-1 / aborted round_end without a winner
            };
            let number = e
                .int("round")
                .or_else(|| e.int("total_rounds_played"))
                .map(|n| n as u32)
                .filter(|n| *n > 0)
                .unwrap_or(rounds.last().map(|r| r.number + 1).unwrap_or(1));
            let end_tick = e.0.tick;
            let mut dead_from = None;
            while rounds.last().is_some_and(|r| r.number >= number) {
                dead_from = rounds.pop().map(|r| r.start_tick);
            }
            let prev_end = rounds.last().map(|r| r.end_tick);
            let lower = prev_end.unwrap_or(i32::MIN);
            let start_tick = starts
                .iter()
                .copied()
                .filter(|t| *t > lower && *t < end_tick)
                .max()
                .or_else(|| freezes.iter().copied().filter(|t| *t > lower && *t < end_tick).max())
                .unwrap_or(prev_end.unwrap_or(0).max(0));
            if let Some(from) = dead_from {
                dead.push((from, start_tick));
            }
            let freeze_end_tick = freezes.iter().copied().filter(|t| *t >= start_tick && *t < end_tick).max();
            rounds.push(Round { number, start_tick, freeze_end_tick, end_tick, winner: Some(winner), reason: e.str("reason") });
        }
        if rounds.is_empty() {
            return Err(ParseError::NoRounds);
        }
        Ok(Self { rounds, dead })
    }

    /// Round owning `tick`: round r spans [start_r, start_{r+1}), so post-round (exit) events stay
    /// in the round they happened after.
    fn round_of(&self, tick: i32) -> Option<u32> {
        if self.dead.iter().any(|(a, b)| tick >= *a && tick < *b) {
            return None;
        }
        let idx = self.rounds.partition_point(|r| r.start_tick <= tick);
        (idx > 0).then(|| self.rounds[idx - 1].number)
    }
}

// ---------------------------------------------------------------------------------------------
// Players
// ---------------------------------------------------------------------------------------------

fn build_players(out: &DemoOutput, ticks: &TickTable, timeline: &Timeline) -> Vec<Player> {
    let scoreboard = if out.player_md.is_empty() { &out.roster } else { &out.player_md };
    let end_team: AHashMap<u64, u8> = scoreboard
        .iter()
        .filter_map(|m| Some((m.steamid?, m.team_number.and_then(|t| u8::try_from(t).ok())?)))
        .collect();

    // Everyone who was ever on T or CT during the sampled ticks, in first-seen order.
    let mut order: Vec<u64> = vec![];
    let mut seen = AHashSet::new();
    let mut names: AHashMap<u64, String> = AHashMap::default();
    for row in 0..ticks.len() {
        let Some(sid) = ticks.steamid(row) else { continue };
        if sid == 0 || !matches!(ticks.team_at_row(row), Some(TEAM_T | TEAM_CT)) {
            continue;
        }
        if seen.insert(sid) {
            order.push(sid);
        }
        if let Some(n) = ticks.name(row) {
            names.insert(sid, n);
        }
    }
    for m in scoreboard {
        if let (Some(sid), Some(n)) = (m.steamid, m.name.as_ref()) {
            names.entry(sid).or_insert_with(|| n.clone());
        }
    }

    let first = &timeline.rounds[0];
    let first_tick = first.freeze_end_tick.unwrap_or(first.start_tick);
    let last = timeline.rounds.last().unwrap();
    let last_tick = last.freeze_end_tick.unwrap_or(last.start_tick);
    // Sort T-side first then CT (by starting team), keeping first-seen order inside a team.
    let mut players: Vec<Player> = order
        .into_iter()
        .map(|sid| {
            let team_start = ticks.team(first_tick, sid).or_else(|| ticks.first_team(sid));
            let rank_row = ticks.row(last_tick, sid).or_else(|| ticks.row(first_tick, sid));
            Player {
                steamid: sid,
                name: names.get(&sid).cloned().unwrap_or_default(),
                team_start,
                team_end: end_team.get(&sid).copied(),
                rank: rank_row.and_then(|r| ticks.i32("rank", r)),
                rank_type: rank_row.and_then(|r| ticks.i32("CCSPlayerController.m_iCompetitiveRankType", r)),
            }
        })
        .collect();
    players.sort_by_key(|p| p.team_start.unwrap_or(u8::MAX));
    players
}

// ---------------------------------------------------------------------------------------------
// Replay frames
// ---------------------------------------------------------------------------------------------

fn build_replay(
    ticks: &TickTable,
    players: &[Player],
    slots: &AHashMap<u64, usize>,
    first_tick: i32,
    last_tick: i32,
) -> Replay {
    let np = players.len();
    let mut r = Replay {
        tick_step: REPLAY_TICK_STEP as u16,
        steamids: players.iter().map(|p| p.steamid).collect(),
        ..Default::default()
    };
    for tick in (first_tick..=last_tick).step_by(REPLAY_TICK_STEP as usize) {
        if !ticks.has_tick(tick) {
            continue;
        }
        r.ticks.push(tick);
        for p in players {
            let row = ticks.row(tick, p.steamid);
            let pos = row.and_then(|row| ticks.pos_at_row(row));
            r.x.push(pos.map(|v| v.x).unwrap_or(0.0));
            r.y.push(pos.map(|v| v.y).unwrap_or(0.0));
            r.z.push(pos.map(|v| v.z).unwrap_or(0.0));
            r.yaw.push(row.and_then(|row| ticks.f32("yaw", row)).unwrap_or(0.0));
            r.health.push(row.and_then(|row| ticks.i32("health", row)).unwrap_or(0).clamp(0, 255) as u8);
            r.armor.push(row.and_then(|row| ticks.i32("armor_value", row)).unwrap_or(0).clamp(0, 255) as u8);
            let mut flags = 0u8;
            if let Some(row) = row {
                if ticks.bool("is_alive", row) == Some(true) {
                    flags |= replay::FLAG_ALIVE;
                }
                if ticks.team_at_row(row) == Some(TEAM_CT) {
                    flags |= replay::FLAG_CT;
                }
                if ticks.bool("is_scoped", row) == Some(true) {
                    flags |= replay::FLAG_SCOPED;
                }
                if ticks.f32("flash_duration", row).unwrap_or(0.0) > 0.0 {
                    flags |= replay::FLAG_FLASHED;
                }
            }
            r.flags.push(flags);
            let mut mask = 0u16;
            if let Some(spotters) = row.and_then(|row| ticks.u64vec("approximate_spotted_by", row)) {
                for s in spotters {
                    if let Some(&slot) = slots.get(s) {
                        if slot < 16 {
                            mask |= 1 << slot;
                        }
                    }
                }
            }
            r.spotted_by.push(mask);
        }
    }
    debug_assert_eq!(r.x.len(), r.ticks.len() * np);
    r
}

// ---------------------------------------------------------------------------------------------
// Event access helpers
// ---------------------------------------------------------------------------------------------

struct Ev<'a>(&'a GameEvent);

impl Ev<'_> {
    fn get(&self, name: &str) -> Option<&Variant> {
        self.0.fields.iter().find(|f| f.name == name).and_then(|f| f.data.as_ref())
    }
    fn str(&self, name: &str) -> Option<String> {
        match self.get(name)? {
            Variant::String(s) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }
    fn int(&self, name: &str) -> Option<i64> {
        match self.get(name)? {
            Variant::I32(v) => Some(*v as i64),
            Variant::U32(v) => Some(*v as i64),
            Variant::U64(v) => i64::try_from(*v).ok(),
            Variant::F32(v) => Some(*v as i64),
            Variant::Bool(b) => Some(*b as i64),
            Variant::String(s) => s.parse().ok(),
            _ => None,
        }
    }
    fn float(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Variant::F32(v) => Some(*v),
            Variant::I32(v) => Some(*v as f32),
            Variant::U32(v) => Some(*v as f32),
            _ => None,
        }
    }
    fn opt_bool(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            Variant::Bool(b) => Some(*b),
            Variant::I32(v) => Some(*v != 0),
            Variant::U32(v) => Some(*v != 0),
            _ => None,
        }
    }
    fn bool(&self, name: &str) -> bool {
        self.opt_bool(name).unwrap_or(false)
    }
    fn sid(&self, name: &str) -> Option<u64> {
        let v = match self.get(name)? {
            Variant::U64(v) => *v,
            Variant::String(s) => s.parse().ok()?,
            _ => return None,
        };
        (v != 0).then_some(v)
    }
    fn vec3(&self, x: &str, y: &str, z: &str) -> Option<Vec3> {
        Some(Vec3 { x: self.float(x)?, y: self.float(y)?, z: self.float(z)? })
    }
}

// ---------------------------------------------------------------------------------------------
// Tick table access
// ---------------------------------------------------------------------------------------------

struct TickTable<'a> {
    df: &'a AHashMap<u32, PropColumn>,
    cols: AHashMap<String, u32>,
    index: AHashMap<(i32, u64), usize>,
    tick_set: AHashSet<i32>,
    len: usize,
}

impl<'a> TickTable<'a> {
    fn new(out: &'a DemoOutput) -> Self {
        let cols = out
            .prop_controller
            .prop_infos
            .iter()
            .map(|p| (p.prop_friendly_name.clone(), p.id))
            .collect();
        let mut index = AHashMap::default();
        let mut tick_set = AHashSet::new();
        let mut len = 0;
        if let (Some(VarVec::I32(ticks)), Some(VarVec::U64(sids))) =
            (out.df.get(&TICK_ID).and_then(|c| c.data.as_ref()), out.df.get(&STEAMID_ID).and_then(|c| c.data.as_ref()))
        {
            len = ticks.len();
            for (row, (t, s)) in ticks.iter().zip(sids.iter()).enumerate() {
                if let (Some(t), Some(s)) = (t, s) {
                    index.insert((*t, *s), row);
                    tick_set.insert(*t);
                }
            }
        }
        Self { df: &out.df, cols, index, tick_set, len }
    }

    fn len(&self) -> usize {
        self.len
    }
    fn has_tick(&self, tick: i32) -> bool {
        self.tick_set.contains(&tick)
    }
    fn row(&self, tick: i32, sid: u64) -> Option<usize> {
        self.index.get(&(tick, sid)).copied()
    }
    fn col(&self, name: &str) -> Option<&VarVec> {
        self.df.get(self.cols.get(name)?)?.data.as_ref()
    }
    fn num(&self, name: &str, row: usize) -> Option<f64> {
        match self.col(name)? {
            VarVec::F32(v) => v.get(row).copied().flatten().map(f64::from),
            VarVec::I32(v) => v.get(row).copied().flatten().map(f64::from),
            VarVec::U32(v) => v.get(row).copied().flatten().map(f64::from),
            VarVec::U64(v) => v.get(row).copied().flatten().map(|x| x as f64),
            VarVec::Bool(v) => v.get(row).copied().flatten().map(|b| b as u8 as f64),
            _ => None,
        }
    }
    fn f32(&self, name: &str, row: usize) -> Option<f32> {
        self.num(name, row).map(|v| v as f32)
    }
    fn i32(&self, name: &str, row: usize) -> Option<i32> {
        self.num(name, row).map(|v| v as i32)
    }
    fn bool(&self, name: &str, row: usize) -> Option<bool> {
        self.num(name, row).map(|v| v != 0.0)
    }
    fn xyz(&self, name: &str, row: usize) -> Option<[f32; 3]> {
        match self.col(name)? {
            VarVec::XYZVec(v) => v.get(row).copied().flatten(),
            _ => None,
        }
    }
    fn u64vec(&self, name: &str, row: usize) -> Option<&Vec<u64>> {
        match self.col(name)? {
            VarVec::U64Vec(v) => v.get(row),
            _ => None,
        }
    }
    fn steamid(&self, row: usize) -> Option<u64> {
        match self.df.get(&STEAMID_ID)?.data.as_ref()? {
            VarVec::U64(v) => v.get(row).copied().flatten(),
            _ => None,
        }
    }
    fn name(&self, row: usize) -> Option<String> {
        match self.df.get(&NAME_ID)?.data.as_ref()? {
            VarVec::String(v) => v.get(row).cloned().flatten(),
            _ => None,
        }
    }
    fn team_at_row(&self, row: usize) -> Option<u8> {
        self.i32("team_num", row).and_then(|t| u8::try_from(t).ok())
    }
    fn team(&self, tick: i32, sid: u64) -> Option<u8> {
        self.team_at_row(self.row(tick, sid)?)
    }
    fn first_team(&self, sid: u64) -> Option<u8> {
        (0..self.len)
            .filter(|r| self.steamid(*r) == Some(sid))
            .find_map(|r| self.team_at_row(r).filter(|t| matches!(*t, TEAM_T | TEAM_CT)))
    }
    fn pos_at_row(&self, row: usize) -> Option<Vec3> {
        Some(Vec3 { x: self.f32("X", row)?, y: self.f32("Y", row)?, z: self.f32("Z", row)? })
    }
    fn pos(&self, tick: i32, sid: u64) -> Option<Vec3> {
        self.pos_at_row(self.row(tick, sid)?)
    }
}
