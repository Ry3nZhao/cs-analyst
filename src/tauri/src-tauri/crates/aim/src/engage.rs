//! Visibility-based metrics (docs/roadmap.md 2.3): time to damage, reaction time, crosshair
//! placement. All are anchored on **contacts**: the first damage player A deals to enemy V, where
//! a new contact starts when A had not damaged V in the previous [`CONTACT_GAP_SECS`].
//!
//! * **Visibility onset** of X seeing Y: walking back from the contact's first damage, the start
//!   of the run of ticks in which X is in Y's spotted-by mask (gaps up to [`MERGE_GAP_TICKS`] are
//!   bridged; X must be seeing Y within [`GRACE_TICKS`] of the damage). If the run reaches the
//!   start of the window the onset is unknown (censored) and the contact gives no sample.
//! * **TTD** (attacker) = first damage tick − onset.
//! * **Reaction time** (attacker) = first gun shot at/after onset − onset.
//! * **Crosshair placement** (attacker *and* victim of the contact, each seeing the other) = angle
//!   between the observer's view and the direction from the observer's eye to the target's head
//!   at the onset tick, split into vertical (v > 0 crosshair above the head) and horizontal
//!   (h > 0 crosshair right of the head). Onsets with an angle above [`MAX_CP_DEG`] (target
//!   spotted outside the screen) are dropped.

use crate::geom::{angles_to, distance, norm_pitch, offset};
use crate::weapons::canonical;
use demo_parse::engagement::{AnchorKind, EngagementWindow, TickState};
use demo_parse::ParsedDemo;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub const CONTACT_GAP_SECS: f32 = 3.0;
pub const MERGE_GAP_TICKS: usize = 8;
pub const GRACE_TICKS: usize = 8;
pub const MAX_CP_DEG: f32 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Observer dealt the contact's first damage.
    Attacker,
    /// Observer received it.
    Victim,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Attacker => "attacker",
            Role::Victim => "victim",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngagementSample {
    pub round: u32,
    pub observer: u64,
    pub target: u64,
    pub role: Role,
    /// Attacker's weapon (canonical); `None` for victim samples.
    pub weapon: Option<String>,
    pub onset_tick: i32,
    /// Tick of the contact's first damage.
    pub damage_tick: i32,
    pub ttd_ms: Option<f32>,
    pub reaction_ms: Option<f32>,
    pub cp_deg: Option<f32>,
    pub cp_v: Option<f32>,
    pub cp_h: Option<f32>,
    pub distance: Option<f32>,
    /// Attacker role: the observer killed the target in this contact. Victim role: the observer died.
    pub killed: bool,
}

enum Onset {
    NotVisible,
    Censored,
    At(usize),
}

fn sees(track: &[Option<TickState>], i: usize) -> bool {
    track[i].is_some_and(|s| s.sees_other)
}

fn onset(track: &[Option<TickState>], end: usize, clipped: bool) -> Onset {
    let lo = end.saturating_sub(GRACE_TICKS);
    let Some(last) = (lo..=end).rev().find(|&i| sees(track, i)) else { return Onset::NotVisible };
    let (mut i, mut first, mut gap) = (last, last, 0);
    while i > 0 {
        i -= 1;
        if sees(track, i) {
            first = i;
            gap = 0;
        } else {
            gap += 1;
            if gap > MERGE_GAP_TICKS {
                break;
            }
        }
    }
    // A visible tick just before the window could extend the run: unknown onset.
    if first == 0 || (!clipped && first <= MERGE_GAP_TICKS) {
        Onset::Censored
    } else {
        Onset::At(first)
    }
}

/// Crosshair placement of `obs` looking at `tgt`: (total, v, h, distance).
fn placement(obs: &TickState, tgt: &TickState) -> Option<(f32, f32, f32, f32)> {
    if !obs.alive || !tgt.alive {
        return None;
    }
    let (eye, head) = (obs.eye(), tgt.eye());
    let (rp, ry) = angles_to(eye, head);
    let (v, h, total) = offset(norm_pitch(obs.pitch), obs.yaw, rp, ry);
    Some((total, v, h, distance(eye, head)))
}

/// Share of first-damage contacts (gun damage between enemies) where the attacker was seeing the
/// victim within [`GRACE_TICKS`]. ESL / some server demos update the spotted mask only sparsely,
/// which shows up as a low value here.
pub fn visibility_quality(windows: &[EngagementWindow], tick_rate: f32) -> Option<f32> {
    let contacts = contacts(windows, tick_rate);
    if contacts.is_empty() {
        return None;
    }
    let seen = contacts
        .iter()
        .filter(|c| {
            let w = c.first;
            let end = w.attacker_track.len() - 1;
            (end.saturating_sub(GRACE_TICKS)..=end).any(|i| sees(&w.attacker_track, i))
        })
        .count();
    Some(seen as f32 / contacts.len() as f32)
}

struct Contact<'a> {
    first: &'a EngagementWindow,
    killed: bool,
}

fn contacts(windows: &[EngagementWindow], tick_rate: f32) -> Vec<Contact<'_>> {
    let gap = (CONTACT_GAP_SECS * tick_rate) as i32;
    let enemy = |w: &EngagementWindow| w.attacker_team.is_some() && w.victim_team.is_some() && w.attacker_team != w.victim_team;
    let mut dmg: Vec<&EngagementWindow> = windows
        .iter()
        .filter(|w| w.kind == AnchorKind::Damage && enemy(w) && canonical(&w.weapon).is_some())
        .collect();
    dmg.sort_by_key(|w| (w.round, w.attacker, w.victim, w.tick));
    let kills: HashSet<(u64, u64, i32)> =
        windows.iter().filter(|w| w.kind == AnchorKind::Kill).map(|w| (w.attacker, w.victim, w.tick)).collect();

    let mut out: Vec<Contact> = vec![];
    let mut last: Option<(u32, u64, u64, i32)> = None;
    for w in dmg {
        let key = (w.round, w.attacker, w.victim);
        let continues = last.is_some_and(|(r, a, v, t)| (r, a, v) == key && w.tick - t <= gap);
        last = Some((w.round, w.attacker, w.victim, w.tick));
        let killed = kills.contains(&(w.attacker, w.victim, w.tick));
        if continues {
            if let Some(c) = out.last_mut() {
                c.killed |= killed;
            }
        } else {
            out.push(Contact { first: w, killed });
        }
    }
    out
}

pub(crate) fn samples(demo: &ParsedDemo, windows: &[EngagementWindow], tick_rate: f32) -> Vec<EngagementSample> {
    let ms = |ticks: i32| ticks as f32 * 1000.0 / tick_rate;
    // Gun-shot ticks per shooter (sorted: events are in tick order).
    let mut shot_ticks: HashMap<u64, Vec<i32>> = HashMap::new();
    for s in &demo.shots {
        if canonical(&s.weapon).is_some() {
            shot_ticks.entry(s.shooter).or_default().push(s.tick);
        }
    }

    let contacts = contacts(windows, tick_rate);
    let mut out = vec![];
    let mut seen_onsets: HashSet<(u64, u64, i32)> = HashSet::new();

    // Attacker samples first so they win the de-duplication of crosshair onsets.
    for c in &contacts {
        let w = c.first;
        let end = w.attacker_track.len() - 1;
        let Onset::At(i) = onset(&w.attacker_track, end, w.clipped_by_round_start) else { continue };
        let onset_tick = w.start_tick + i as i32;
        let reaction = shot_ticks.get(&w.attacker).and_then(|ts| {
            let k = ts.partition_point(|t| *t < onset_tick);
            ts.get(k).filter(|t| **t <= w.tick).map(|t| ms(t - onset_tick))
        });
        let cp = match (&w.attacker_track[i], &w.victim_track[i]) {
            (Some(a), Some(v)) => placement(a, v),
            _ => None,
        };
        let cp = cp.filter(|c| c.0 <= MAX_CP_DEG);
        seen_onsets.insert((w.attacker, w.victim, onset_tick));
        out.push(EngagementSample {
            round: w.round,
            observer: w.attacker,
            target: w.victim,
            role: Role::Attacker,
            weapon: canonical(&w.weapon).map(|x| x.name.to_string()),
            onset_tick,
            damage_tick: w.tick,
            ttd_ms: Some(ms(w.tick - onset_tick)),
            reaction_ms: reaction,
            cp_deg: cp.map(|c| c.0),
            cp_v: cp.map(|c| c.1),
            cp_h: cp.map(|c| c.2),
            distance: cp.map(|c| c.3),
            killed: c.killed,
        });
    }

    for c in &contacts {
        let w = c.first;
        let end = w.victim_track.len() - 1;
        let Onset::At(i) = onset(&w.victim_track, end, w.clipped_by_round_start) else { continue };
        let onset_tick = w.start_tick + i as i32;
        if !seen_onsets.insert((w.victim, w.attacker, onset_tick)) {
            continue;
        }
        let cp = match (&w.victim_track[i], &w.attacker_track[i]) {
            (Some(v), Some(a)) => placement(v, a),
            _ => None,
        };
        let Some(cp) = cp.filter(|c| c.0 <= MAX_CP_DEG) else { continue };
        out.push(EngagementSample {
            round: w.round,
            observer: w.victim,
            target: w.attacker,
            role: Role::Victim,
            weapon: None,
            onset_tick,
            damage_tick: w.tick,
            ttd_ms: None,
            reaction_ms: None,
            cp_deg: Some(cp.0),
            cp_v: Some(cp.1),
            cp_h: Some(cp.2),
            distance: Some(cp.3),
            killed: c.killed,
        });
    }
    out.sort_by_key(|s| (s.onset_tick, s.observer));
    out
}
