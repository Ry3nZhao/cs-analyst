//! Aim analysis for CS Analyst (docs/roadmap.md 2.3 / 2.4).
//!
//! Input: the regular [`demo_parse::ParsedDemo`] plus the full-rate engagement windows from
//! [`demo_parse::parse_engagements_file`]. Output: per-contact samples (TTD, reaction, crosshair
//! placement), sprays with per-bullet compensation error, and per player × weapon summaries.
//!
//! Conventions: angles in degrees; offsets in screen terms (v > 0 up, h > 0 right); times in ms.

pub mod dist;
pub mod engage;
pub mod geom;
pub mod shots;
pub mod spray;
pub mod weapons;

pub use dist::{histogram, ratio, Dist};
pub use engage::{EngagementSample, Role};
pub use shots::ShotCounters;
pub use spray::{aggregate, BulletAgg, Spray, SprayShot, WeaponPattern};

use demo_parse::engagement::TickState;
use demo_parse::{Engagements, ParsedDemo};
use serde::Serialize;
use std::collections::HashMap;

/// Bump when an algorithm or constant changes; stored per match so results can be recomputed.
pub const AIM_VERSION: u32 = 1;
/// Visibility metrics are only produced when at least this share of first-damage contacts has
/// the attacker seeing the victim (see [`engage::visibility_quality`]).
pub const MIN_VISIBILITY_QUALITY: f32 = 0.7;
/// Weapon key of the all-weapons summary row.
pub const ALL_WEAPONS: &str = "*";

pub(crate) type StateLookup = HashMap<(i32, u64), TickState>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AimReport {
    pub tick_rate: f32,
    pub visibility_quality: Option<f32>,
    pub visibility_reliable: bool,
    pub engagements: Vec<EngagementSample>,
    pub sprays: Vec<Spray>,
    /// One row per player × weapon, plus an [`ALL_WEAPONS`] row per player.
    pub players: Vec<PlayerAim>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAim {
    pub steamid: u64,
    pub weapon: String,
    pub counters: ShotCounters,
    pub ttd: Dist,
    pub reaction: Dist,
    pub cp: Dist,
    /// Signed vertical crosshair offset (v > 0 above the head).
    pub cp_v: Dist,
    pub cp_v_abs: Dist,
    pub cp_h_abs: Dist,
    pub sprays: u32,
    /// Mean spray control (0–100).
    pub spray_control: Option<f32>,
    /// Mean spray error (degrees).
    pub spray_err: Option<f32>,
}

impl PlayerAim {
    /// Build a summary from samples / sprays / counters that already belong to one player
    /// (and one weapon, unless `weapon` is [`ALL_WEAPONS`]).
    pub fn build(
        steamid: u64,
        weapon: &str,
        counters: ShotCounters,
        samples: &[&EngagementSample],
        sprays: &[&Spray],
    ) -> Self {
        let pick = |f: fn(&EngagementSample) -> Option<f32>| Dist::of(samples.iter().filter_map(|s| f(s)));
        let ctrl: Vec<f32> = sprays.iter().filter_map(|s| s.control).collect();
        let n = sprays.len();
        Self {
            steamid,
            weapon: weapon.to_string(),
            counters,
            ttd: pick(|x| x.ttd_ms),
            reaction: pick(|x| x.reaction_ms),
            cp: pick(|x| x.cp_deg),
            cp_v: pick(|x| x.cp_v),
            cp_v_abs: pick(|x| x.cp_v.map(f32::abs)),
            cp_h_abs: pick(|x| x.cp_h.map(f32::abs)),
            sprays: n as u32,
            spray_control: (!ctrl.is_empty()).then(|| ctrl.iter().sum::<f32>() / ctrl.len() as f32),
            spray_err: (n > 0).then(|| sprays.iter().map(|s| s.mean_err).sum::<f32>() / n as f32),
        }
    }
}

pub fn analyze(demo: &ParsedDemo, eng: &Engagements) -> AimReport {
    let tr = eng.tick_rate;
    let mut states: StateLookup = HashMap::new();
    for w in &eng.windows {
        for (i, t) in w.ticks().enumerate() {
            if let Some(s) = w.attacker_track[i] {
                states.entry((t, w.attacker)).or_insert(s);
            }
            if let Some(s) = w.victim_track[i] {
                states.entry((t, w.victim)).or_insert(s);
            }
        }
    }

    let visibility_quality = engage::visibility_quality(&eng.windows, tr);
    let visibility_reliable = visibility_quality.is_some_and(|q| q >= MIN_VISIBILITY_QUALITY);
    let engagements = if visibility_reliable { engage::samples(demo, &eng.windows, tr) } else { vec![] };
    let shot_velocity: HashMap<(i32, u64), demo_parse::Vec3> =
        eng.shot_sights.iter().filter_map(|s| Some(((s.tick, s.shooter), s.velocity?))).collect();
    let sprays = spray::detect(demo, tr, &states, &shot_velocity);
    let counters = shots::count(demo, eng, tr, visibility_reliable);

    let mut players = vec![];
    for p in &demo.players {
        let sid = p.steamid;
        let mut weapons: Vec<&'static str> = counters.keys().filter(|(s, _)| *s == sid).map(|(_, w)| *w).collect();
        weapons.sort_unstable();
        let mut all = ShotCounters::default();
        for w in &weapons {
            let c = counters[&(sid, *w)].clone();
            all.add(&c);
            let samples: Vec<&EngagementSample> =
                engagements.iter().filter(|e| e.observer == sid && e.weapon.as_deref() == Some(*w)).collect();
            let sp: Vec<&Spray> = sprays.iter().filter(|s| s.steamid == sid && s.weapon == *w).collect();
            players.push(PlayerAim::build(sid, w, c, &samples, &sp));
        }
        let samples: Vec<&EngagementSample> = engagements.iter().filter(|e| e.observer == sid).collect();
        let sp: Vec<&Spray> = sprays.iter().filter(|s| s.steamid == sid).collect();
        players.push(PlayerAim::build(sid, ALL_WEAPONS, all, &samples, &sp));
    }

    AimReport { tick_rate: tr, visibility_quality, visibility_reliable, engagements, sprays, players }
}
