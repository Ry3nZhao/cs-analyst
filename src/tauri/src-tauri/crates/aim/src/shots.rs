//! Per-shot counters: accuracy, visible accuracy, first-bullet accuracy, counter-strafing.
//!
//! * A shot **hits** when the shooter damages an enemy on the fire tick (hitscan: `player_hurt`
//!   lands on the same tick as `weapon_fire`); a shotgun blast counts once.
//! * **Visible** shots: at least one alive enemy has the shooter in its spotted-by mask.
//! * **First bullet** of a burst: no shot with the same weapon in the preceding `cycle × 1.5`.
//!   First-bullet accuracy only counts first bullets fired while an enemy was visible.
//! * **Counter-strafe**: over all first bullets, horizontal speed at the fire tick ≤ 34 % of the
//!   weapon's max speed (scoped speed for scoped snipers / AUG / SG 553).

use crate::weapons::canonical;
use demo_parse::{Engagements, ParsedDemo};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotCounters {
    pub shots: u32,
    pub hits: u32,
    pub head_hits: u32,
    /// `None` when the demo's visibility data is unreliable.
    pub vis_shots: Option<u32>,
    pub vis_hits: Option<u32>,
    pub first_shots: Option<u32>,
    pub first_hits: Option<u32>,
    pub cs_shots: u32,
    pub cs_ok: u32,
}

impl ShotCounters {
    pub fn add(&mut self, o: &ShotCounters) {
        fn opt(a: &mut Option<u32>, b: Option<u32>) {
            if let Some(b) = b {
                *a = Some(a.unwrap_or(0) + b);
            }
        }
        self.shots += o.shots;
        self.hits += o.hits;
        self.head_hits += o.head_hits;
        opt(&mut self.vis_shots, o.vis_shots);
        opt(&mut self.vis_hits, o.vis_hits);
        opt(&mut self.first_shots, o.first_shots);
        opt(&mut self.first_hits, o.first_hits);
        self.cs_shots += o.cs_shots;
        self.cs_ok += o.cs_ok;
    }
}

/// Counters keyed by (steamid, canonical weapon).
pub(crate) fn count(
    demo: &ParsedDemo,
    eng: &Engagements,
    tick_rate: f32,
    visibility_reliable: bool,
) -> HashMap<(u64, &'static str), ShotCounters> {
    // (tick, attacker) → hit an enemy (value: one of the hits was to the head).
    let mut hits: HashMap<(i32, u64), bool> = HashMap::new();
    for d in &demo.damages {
        let Some(a) = d.attacker else { continue };
        if d.victim.is_none()
            || d.attacker_team.is_none()
            || d.attacker_team == d.victim_team
            || canonical(&d.weapon).is_none()
        {
            continue;
        }
        let head = hits.entry((d.tick, a)).or_insert(false);
        *head |= d.hitgroup.as_deref() == Some("head");
    }
    let sights: HashMap<(i32, u64), &demo_parse::engagement::ShotSight> =
        eng.shot_sights.iter().map(|s| ((s.tick, s.shooter), s)).collect();

    let mut out: HashMap<(u64, &'static str), ShotCounters> = HashMap::new();
    let mut last_shot: HashMap<(u64, &'static str), i32> = HashMap::new();
    for s in &demo.shots {
        let Some(w) = canonical(&s.weapon) else { continue };
        let c = out.entry((s.shooter, w.name)).or_default();
        let hit = hits.get(&(s.tick, s.shooter)).copied();
        let sight = sights.get(&(s.tick, s.shooter));
        let vis = sight.is_some_and(|x| !x.visible_enemies.is_empty());
        let first = match last_shot.insert((s.shooter, w.name), s.tick) {
            Some(prev) => (s.tick - prev) as f32 > w.cycle_ticks(tick_rate) * crate::spray::SPRAY_GAP_CYCLES,
            None => true,
        };

        c.shots += 1;
        c.hits += hit.is_some() as u32;
        c.head_hits += (hit == Some(true)) as u32;
        if visibility_reliable {
            *c.vis_shots.get_or_insert(0) += vis as u32;
            *c.vis_hits.get_or_insert(0) += (vis && hit.is_some()) as u32;
            *c.first_shots.get_or_insert(0) += (vis && first) as u32;
            *c.first_hits.get_or_insert(0) += (vis && first && hit.is_some()) as u32;
        }
        if first {
            // `Shot::velocity` from the sampled parse is unreliable; use the engagement pass's.
            if let Some(v) = sight.and_then(|x| x.velocity) {
                c.cs_shots += 1;
                c.cs_ok += (v.x.hypot(v.y) <= w.accurate_speed(s.is_scoped == Some(true))) as u32;
            }
        }
    }
    out
}
