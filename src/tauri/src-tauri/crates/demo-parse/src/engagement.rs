//! Engagement windows: full-rate (every tick) state of both players for the last N seconds before
//! every kill / damage, plus what each shooter could see when firing.
//!
//! This is a separate, self-contained parse (events pass + tick pass restricted to the window
//! ticks); it does not change [`crate::parse_demo_file`]'s output. Used by the aim analysis (TTD,
//! crosshair placement, reaction time, visible accuracy, target-relative spray error).

use super::{run_parser, Ev, ParseError, Timeline, TickTable, Vec3};
use ahash::{AHashMap, AHashSet};
use parser::second_pass::parser_settings::create_huffman_lookup_table;
use serde::Serialize;
use std::path::Path;

/// Standing eye height above the origin (CS2).
pub const EYE_HEIGHT_STANDING: f32 = 64.0;
/// Fully crouched eye height above the origin (CS2).
pub const EYE_HEIGHT_CROUCHED: f32 = 46.0;

#[derive(Debug, Clone, Copy)]
pub struct EngagementOptions {
    /// History kept before each anchor event, in seconds (clamped to the round's freeze-time end).
    pub window_secs: f32,
    /// Also materialize every `weapon_fire` tick and report which enemies the shooter saw.
    pub shot_sight: bool,
}

impl Default for EngagementOptions {
    fn default() -> Self {
        Self { window_secs: 3.0, shot_sight: true }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Engagements {
    pub tick_rate: f32,
    pub windows: Vec<EngagementWindow>,
    pub shot_sights: Vec<ShotSight>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    Kill,
    Damage,
}

/// One kill / damage event and the per-tick state of attacker and victim leading up to it.
#[derive(Debug, Clone, Serialize)]
pub struct EngagementWindow {
    pub kind: AnchorKind,
    pub round: u32,
    /// Tick of the kill / damage event (last tick of the window).
    pub tick: i32,
    pub attacker: u64,
    pub victim: u64,
    pub weapon: String,
    pub headshot: bool,
    pub hitgroup: Option<String>,
    pub dmg_health: i32,
    pub attacker_team: Option<u8>,
    pub victim_team: Option<u8>,
    /// First tick of the window; `true` when it was cut by the round's freeze-time end (i.e. the
    /// window covers everything since the players could move).
    pub start_tick: i32,
    pub clipped_by_round_start: bool,
    /// `attacker_track[i]` / `victim_track[i]` are the states at tick `start_tick + i`.
    pub attacker_track: Vec<Option<TickState>>,
    pub victim_track: Vec<Option<TickState>>,
}

impl EngagementWindow {
    pub fn ticks(&self) -> impl Iterator<Item = i32> + '_ {
        (0..self.attacker_track.len() as i32).map(move |i| self.start_tick + i)
    }
}

/// One player's state at one tick, seen from an engagement window.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TickState {
    pub pos: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    /// Position delta from the previous tick × tick rate (u/s). Computed here rather than taken
    /// from the parser's `velocity_*`, which is only correct when the previous ticks were
    /// materialized too.
    pub velocity: Vec3,
    /// 0 = standing, 1 = fully crouched.
    pub duck_amount: f32,
    pub alive: bool,
    pub health: i32,
    /// The *other* player of the window is in this player's `m_bSpottedByMask`, i.e. this player
    /// currently sees the other one (engine's approximate visibility, used for radar spotting).
    pub sees_other: bool,
}

impl TickState {
    /// Eye position, accounting for crouch.
    pub fn eye(&self) -> Vec3 {
        let h = EYE_HEIGHT_STANDING - (EYE_HEIGHT_STANDING - EYE_HEIGHT_CROUCHED) * self.duck_amount.clamp(0.0, 1.0);
        Vec3 { x: self.pos.x, y: self.pos.y, z: self.pos.z + h }
    }
}

/// Which enemies were visible to the shooter on a `weapon_fire` tick.
#[derive(Debug, Clone, Serialize)]
pub struct ShotSight {
    pub round: u32,
    pub tick: i32,
    pub shooter: u64,
    pub weapon: String,
    /// Alive enemies whose spotted-by mask contains the shooter.
    pub visible_enemies: Vec<u64>,
    /// Shooter velocity at the fire tick (position delta from the previous tick × tick rate).
    pub velocity: Option<Vec3>,
}

const WINDOW_PROPS: &[&str] = &[
    "X",
    "Y",
    "Z",
    "pitch",
    "yaw",
    "health",
    "is_alive",
    "team_num",
    "duck_amount",
    "approximate_spotted_by",
];

const WINDOW_EVENTS: &[&str] = &[
    "round_start",
    "round_end",
    "round_freeze_end",
    "player_death",
    "player_hurt",
    "weapon_fire",
];

pub fn parse_engagements_file(path: &Path, opts: EngagementOptions) -> Result<Engagements, ParseError> {
    // `.dem` or zstd-compressed `.dem.zst`
    let bytes = crate::source::read_demo(path)?;
    parse_engagements_bytes(&bytes, opts)
}

pub fn parse_engagements_bytes(bytes: &[u8], opts: EngagementOptions) -> Result<Engagements, ParseError> {
    let huf = create_huffman_lookup_table();
    let ev_out = run_parser(
        bytes,
        &huf,
        WINDOW_EVENTS.iter().map(|s| s.to_string()).collect(),
        vec![],
        super::GAME_RULE_PROPS,
        vec![],
    )?;
    let events: Vec<Ev> = ev_out.game_events.iter().map(Ev).collect();
    let timeline = Timeline::build(&events)?;
    let tick_rate = 64.0f32;
    let span = (opts.window_secs.max(0.0) * tick_rate).round() as i32;

    // Anchors and the ticks they need.
    struct Anchor<'a> {
        ev: &'a Ev<'a>,
        kind: AnchorKind,
        round: u32,
        attacker: u64,
        victim: u64,
        start: i32,
        clipped: bool,
    }
    let round_floor = |round: u32| {
        timeline.rounds.iter().find(|r| r.number == round).map(|r| r.freeze_end_tick.unwrap_or(r.start_tick))
    };
    let mut anchors = vec![];
    let mut fires = vec![];
    let mut wanted: AHashSet<i32> = AHashSet::new();
    for e in &events {
        let tick = e.0.tick;
        let Some(round) = timeline.round_of(tick) else { continue };
        match e.0.name.as_str() {
            name @ ("player_death" | "player_hurt") => {
                let (Some(attacker), Some(victim)) = (e.sid("attacker_steamid"), e.sid("user_steamid")) else {
                    continue;
                };
                if attacker == victim {
                    continue;
                }
                let floor = round_floor(round).unwrap_or(tick);
                let start = (tick - span).max(floor.min(tick));
                // One extra tick in front for velocity.
                wanted.extend(start - 1..=tick);
                let kind = if name == "player_death" { AnchorKind::Kill } else { AnchorKind::Damage };
                anchors.push(Anchor { ev: e, kind, round, attacker, victim, start, clipped: start > tick - span });
            }
            "weapon_fire" if opts.shot_sight => {
                if let Some(shooter) = e.sid("user_steamid") {
                    wanted.insert(tick);
                    wanted.insert(tick - 1);
                    fires.push((e, round, shooter));
                }
            }
            _ => {}
        }
    }

    let tick_out = run_parser(bytes, &huf, vec![], WINDOW_PROPS.to_vec(), &[], wanted.into_iter().collect())?;
    let ticks = TickTable::new(&tick_out);

    let velocity = |tick: i32, sid: u64| -> Option<Vec3> {
        let now = ticks.pos(tick, sid)?;
        let prev = ticks.pos(tick - 1, sid)?;
        Some(Vec3 { x: (now.x - prev.x) * tick_rate, y: (now.y - prev.y) * tick_rate, z: (now.z - prev.z) * tick_rate })
    };
    let state = |tick: i32, me: u64, other: u64| -> Option<TickState> {
        let row = ticks.row(tick, me)?;
        let spotted_other = ticks.row(tick, other).and_then(|r| ticks.u64vec("approximate_spotted_by", r));
        Some(TickState {
            pos: ticks.pos_at_row(row)?,
            pitch: ticks.f32("pitch", row)?,
            yaw: ticks.f32("yaw", row)?,
            velocity: velocity(tick, me).unwrap_or_default(),
            duck_amount: ticks.f32("duck_amount", row).unwrap_or(0.0),
            alive: ticks.bool("is_alive", row).unwrap_or(false),
            health: ticks.i32("health", row).unwrap_or(0),
            sees_other: spotted_other.is_some_and(|v| v.contains(&me)),
        })
    };

    let windows = anchors
        .iter()
        .map(|a| {
            let tick = a.ev.0.tick;
            EngagementWindow {
                kind: a.kind,
                round: a.round,
                tick,
                attacker: a.attacker,
                victim: a.victim,
                weapon: super::weapon_name(a.ev.str("weapon")),
                headshot: a.kind == AnchorKind::Kill && a.ev.bool("headshot")
                    || a.ev.str("hitgroup").as_deref() == Some("head"),
                hitgroup: a.ev.str("hitgroup"),
                dmg_health: a.ev.int("dmg_health").unwrap_or(0) as i32,
                attacker_team: ticks.team(tick, a.attacker),
                victim_team: ticks.team(tick, a.victim),
                start_tick: a.start,
                clipped_by_round_start: a.clipped,
                attacker_track: (a.start..=tick).map(|t| state(t, a.attacker, a.victim)).collect(),
                victim_track: (a.start..=tick).map(|t| state(t, a.victim, a.attacker)).collect(),
            }
        })
        .collect();

    // Everyone with a T/CT team at some materialized tick.
    let mut roster: Vec<u64> = vec![];
    {
        let mut seen = AHashSet::new();
        for row in 0..ticks.len() {
            if let Some(sid) = ticks.steamid(row) {
                if sid != 0 && seen.insert(sid) {
                    roster.push(sid);
                }
            }
        }
    }
    let mut shot_sights = Vec::with_capacity(fires.len());
    let mut team_cache: AHashMap<(i32, u64), Option<u8>> = AHashMap::default();
    for (e, round, shooter) in fires {
        let tick = e.0.tick;
        let mut team_of = |sid: u64| *team_cache.entry((tick, sid)).or_insert_with(|| ticks.team(tick, sid));
        let my_team = team_of(shooter);
        let visible_enemies = roster
            .iter()
            .copied()
            .filter(|&sid| sid != shooter)
            .filter(|&sid| {
                let t = team_of(sid);
                matches!(t, Some(super::TEAM_T | super::TEAM_CT)) && t != my_team
            })
            .filter(|&sid| {
                let Some(row) = ticks.row(tick, sid) else { return false };
                ticks.bool("is_alive", row) == Some(true)
                    && ticks.u64vec("approximate_spotted_by", row).is_some_and(|v| v.contains(&shooter))
            })
            .collect();
        shot_sights.push(ShotSight {
            round,
            tick,
            shooter,
            weapon: super::weapon_name(e.str("weapon")),
            visible_enemies,
            velocity: velocity(tick, shooter),
        });
    }

    Ok(Engagements { tick_rate, windows, shot_sights })
}
