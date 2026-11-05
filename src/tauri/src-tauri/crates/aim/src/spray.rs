//! Spray control (docs/roadmap.md 2.4).
//!
//! * A **spray** is a run of shots of one spray-analysed weapon by one player where consecutive
//!   shots are at most `cycle × 1.5` apart, with at least [`MIN_SPRAY_SHOTS`] shots.
//! * The **bullet direction** of a shot is `view angles + 2 × aim punch` at the fire tick.
//! * Every shot is expressed relative to the **first** shot (screen terms, degrees, v > 0 = up,
//!   h > 0 = right):
//!   - `recoil` = where recoil alone moved the bullet: `-2 × Δpunch`;
//!   - `aim` = how the player moved the crosshair: `-Δview`;
//!   - `err = recoil + aim` = where the bullet actually landed relative to the first one. Perfect
//!     compensation gives 0. `err_v > 0` → bullet above the first shot → **under**-compensated
//!     (压少了); `err_v < 0` → **over**-compensated (压多了).
//! * `control` (0–100) = mean over shots with `|recoil| ≥ 1°` of `clamp(1 − |err| / |recoil|, 0, 1)`,
//!   i.e. the share of recoil the player cancelled out; independent of spray length and weapon.
//! * `break_shot` = first bullet (1-based) whose `|err|` exceeds [`BREAK_DEG`].
//! * A spray is split where the bullet direction jumps more than [`FLICK_DEG`] between two
//!   consecutive shots (spray transfer to another enemy / flick): compensation error is
//!   meaningless across a deliberate re-aim.

use crate::geom::{angles_to, norm_pitch, offset, wrap180};
use crate::weapons::{canonical, WeaponInfo};
use crate::StateLookup;
use demo_parse::{ParsedDemo, Shot, Vec3};
use serde::Serialize;
use std::collections::HashMap;

/// Bullet-direction jump between consecutive shots that splits a spray.
pub const FLICK_DEG: f32 = 5.0;
/// Shorter runs are bursts / taps, not sprays.
pub const MIN_SPRAY_SHOTS: usize = 4;
/// Two shots belong to the same spray if they are at most this many fire cycles apart.
pub const SPRAY_GAP_CYCLES: f32 = 1.5;
/// Error at which a spray counts as "broken" (degrees): about one body width (32 u) at 25 m.
pub const BREAK_DEG: f32 = 2.0;
/// Only shots with at least this much recoil count towards `control`.
pub const CONTROL_MIN_RECOIL_DEG: f32 = 1.0;
/// Aggregated "break shot" needs this many sprays reaching the bullet.
pub const MIN_BREAK_SAMPLES: u32 = 3;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spray {
    pub steamid: u64,
    pub round: u32,
    pub team: Option<u8>,
    pub weapon: String,
    pub start_tick: i32,
    pub end_tick: i32,
    pub hits: u32,
    pub head_hits: u32,
    /// Enemy that took the most damage from this spray.
    pub target: Option<u64>,
    /// The spray killed `target`.
    pub killed: bool,
    /// Horizontal speed at the first shot (u/s).
    pub start_speed: Option<f32>,
    /// Mean |err| over shots 2..n (degrees).
    pub mean_err: f32,
    /// Mean signed vertical / horizontal error over shots 2..n (v > 0 under-compensated).
    pub mean_err_v: f32,
    pub mean_err_h: f32,
    pub control: Option<f32>,
    pub break_shot: Option<u32>,
    pub shots: Vec<SprayShot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SprayShot {
    /// 1-based bullet number in the spray.
    pub idx: u32,
    pub tick: i32,
    pub pitch: f32,
    pub yaw: f32,
    pub punch_pitch: f32,
    pub punch_yaw: f32,
    pub recoil_v: f32,
    pub recoil_h: f32,
    pub aim_v: f32,
    pub aim_h: f32,
    pub err_v: f32,
    pub err_h: f32,
    pub hit: bool,
    pub hitgroup: Option<String>,
    /// Bullet relative to the target's head (v > 0 above, h > 0 right), when the target's
    /// position at this tick is known.
    pub tgt_v: Option<f32>,
    pub tgt_h: Option<f32>,
}

impl SprayShot {
    pub fn err(&self) -> f32 {
        self.err_v.hypot(self.err_h)
    }
    pub fn recoil(&self) -> f32 {
        self.recoil_v.hypot(self.recoil_h)
    }
}

/// Build a shot sequence's errors (relative to the first shot). `shots` = (tick, pitch, yaw,
/// punch_pitch, punch_yaw).
pub fn compensation(shots: &[(i32, f32, f32, f32, f32)]) -> Vec<SprayShot> {
    let Some(&(_, p0, y0, pp0, py0)) = shots.first() else { return vec![] };
    let p0 = norm_pitch(p0);
    shots
        .iter()
        .enumerate()
        .map(|(i, &(tick, p, y, pp, py))| {
            let p = norm_pitch(p);
            let recoil_v = -2.0 * (pp - pp0);
            let recoil_h = -2.0 * (py - py0);
            let aim_v = -(p - p0);
            let aim_h = -wrap180(y - y0);
            SprayShot {
                idx: i as u32 + 1,
                tick,
                pitch: p,
                yaw: y,
                punch_pitch: pp,
                punch_yaw: py,
                recoil_v,
                recoil_h,
                aim_v,
                aim_h,
                err_v: recoil_v + aim_v,
                err_h: recoil_h + aim_h,
                hit: false,
                hitgroup: None,
                tgt_v: None,
                tgt_h: None,
            }
        })
        .collect()
}

/// Spray-level summary fields from its shots.
pub fn summarize(shots: &[SprayShot]) -> (f32, f32, f32, Option<f32>, Option<u32>) {
    let rest = shots.iter().skip(1);
    let n = shots.len().saturating_sub(1).max(1) as f32;
    let mean_err = rest.clone().map(SprayShot::err).sum::<f32>() / n;
    let mean_v = rest.clone().map(|s| s.err_v).sum::<f32>() / n;
    let mean_h = rest.map(|s| s.err_h).sum::<f32>() / n;
    let terms: Vec<f32> = shots
        .iter()
        .filter(|s| s.recoil() >= CONTROL_MIN_RECOIL_DEG)
        .map(|s| (1.0 - s.err() / s.recoil()).clamp(0.0, 1.0))
        .collect();
    let control = (!terms.is_empty()).then(|| 100.0 * terms.iter().sum::<f32>() / terms.len() as f32);
    let break_shot = shots.iter().find(|s| s.err() > BREAK_DEG).map(|s| s.idx);
    (mean_err, mean_v, mean_h, control, break_shot)
}

pub(crate) fn detect(
    demo: &ParsedDemo,
    tick_rate: f32,
    states: &StateLookup,
    shot_velocity: &HashMap<(i32, u64), Vec3>,
) -> Vec<Spray> {
    // Enemy damage by (tick, attacker).
    let mut hits: HashMap<(i32, u64), Vec<&demo_parse::Damage>> = HashMap::new();
    for d in &demo.damages {
        if let (Some(a), Some(_)) = (d.attacker, d.victim) {
            if d.attacker_team.is_some() && d.attacker_team != d.victim_team {
                hits.entry((d.tick, a)).or_default().push(d);
            }
        }
    }

    let mut by_shooter: HashMap<u64, Vec<(&Shot, &'static WeaponInfo)>> = HashMap::new();
    for s in &demo.shots {
        if let Some(w) = canonical(&s.weapon).filter(|w| w.spray) {
            by_shooter.entry(s.shooter).or_default().push((s, w));
        }
    }

    let mut out = vec![];
    let mut shooters: Vec<_> = by_shooter.into_iter().collect();
    shooters.sort_by_key(|(sid, _)| *sid);
    for (sid, shots) in shooters {
        let mut run: Vec<(&Shot, &'static WeaponInfo)> = vec![];
        let flush = |run: &mut Vec<(&Shot, &'static WeaponInfo)>, out: &mut Vec<Spray>| {
            let mut from = 0;
            for i in 1..=run.len() {
                if i == run.len() || flick(run[i - 1].0, run[i].0) {
                    if i - from >= MIN_SPRAY_SHOTS {
                        out.push(build(sid, &run[from..i], &hits, demo, states, shot_velocity));
                    }
                    from = i;
                }
            }
            run.clear();
        };
        for (s, w) in shots {
            let usable = s.pitch.is_some() && s.yaw.is_some() && s.punch_pitch.is_some() && s.punch_yaw.is_some();
            if let Some((prev, pw)) = run.last() {
                let same = pw.name == w.name
                    && prev.round == s.round
                    && s.tick > prev.tick
                    && (s.tick - prev.tick) as f32 <= w.cycle_ticks(tick_rate) * SPRAY_GAP_CYCLES;
                if !same || !usable {
                    flush(&mut run, &mut out);
                }
            }
            if usable {
                run.push((s, w));
            }
        }
        flush(&mut run, &mut out);
    }
    out.sort_by_key(|s| (s.start_tick, s.steamid));
    out
}

/// Bullet direction (view + 2 × punch) jumps more than [`FLICK_DEG`] between two shots.
fn flick(a: &Shot, b: &Shot) -> bool {
    let dir = |s: &Shot| (norm_pitch(s.pitch.unwrap()) + 2.0 * s.punch_pitch.unwrap(), s.yaw.unwrap() + 2.0 * s.punch_yaw.unwrap());
    let ((p0, y0), (p1, y1)) = (dir(a), dir(b));
    offset(p1, y1, p0, y0).2 > FLICK_DEG
}

fn build(
    sid: u64,
    run: &[(&Shot, &'static WeaponInfo)],
    hits: &HashMap<(i32, u64), Vec<&demo_parse::Damage>>,
    demo: &ParsedDemo,
    states: &StateLookup,
    shot_velocity: &HashMap<(i32, u64), Vec3>,
) -> Spray {
    let raw: Vec<_> = run
        .iter()
        .map(|(s, _)| (s.tick, s.pitch.unwrap(), s.yaw.unwrap(), s.punch_pitch.unwrap(), s.punch_yaw.unwrap()))
        .collect();
    let mut shots = compensation(&raw);

    // Hits and target.
    let mut dmg_by_victim: HashMap<u64, i32> = HashMap::new();
    for sh in &mut shots {
        if let Some(ds) = hits.get(&(sh.tick, sid)) {
            sh.hit = true;
            sh.hitgroup = ds[0].hitgroup.clone();
            for d in ds {
                *dmg_by_victim.entry(d.victim.unwrap()).or_default() += d.dmg_health;
            }
        }
    }
    let target = dmg_by_victim.iter().max_by_key(|(v, d)| (**d, std::cmp::Reverse(**v))).map(|(v, _)| *v);
    let (start_tick, end_tick) = (shots[0].tick, shots.last().unwrap().tick);

    // Bullet relative to the target's head.
    if let Some(t) = target {
        for sh in &mut shots {
            if let (Some(me), Some(them)) = (states.get(&(sh.tick, sid)), states.get(&(sh.tick, t))) {
                let (rp, ry) = angles_to(me.eye(), them.eye());
                let bp = sh.pitch + 2.0 * sh.punch_pitch;
                let by = sh.yaw + 2.0 * sh.punch_yaw;
                let (v, h, _) = offset(bp, by, rp, ry);
                sh.tgt_v = Some(v);
                sh.tgt_h = Some(h);
            }
        }
    }

    let (mean_err, mean_err_v, mean_err_h, control, break_shot) = summarize(&shots);
    let first = run[0].0;
    Spray {
        steamid: sid,
        round: first.round,
        team: first.team,
        weapon: run[0].1.name.to_string(),
        start_tick,
        end_tick,
        hits: shots.iter().filter(|s| s.hit).count() as u32,
        head_hits: shots.iter().filter(|s| s.hitgroup.as_deref() == Some("head")).count() as u32,
        target,
        killed: target.is_some_and(|t| {
            demo.kills.iter().any(|k| {
                k.attacker == Some(sid) && k.victim == Some(t) && k.tick >= start_tick && k.tick <= end_tick
            })
        }),
        start_speed: shot_velocity.get(&(first.tick, sid)).map(|v| v.x.hypot(v.y)),
        mean_err,
        mean_err_v,
        mean_err_h,
        control,
        break_shot,
        shots,
    }
}

// ---------------------------------------------------------------------------------------------
// Aggregation across sprays (per weapon)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulletAgg {
    pub idx: u32,
    /// Sprays that reached this bullet.
    pub n: u32,
    pub recoil_v: f32,
    pub recoil_h: f32,
    pub aim_v: f32,
    pub aim_h: f32,
    pub err_v: f32,
    pub err_h: f32,
    /// Mean |err|.
    pub err: f32,
    /// Median |err| (robust to the odd wild spray); used for `break_shot`.
    pub err_median: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponPattern {
    pub weapon: String,
    pub sprays: u32,
    pub shots: u32,
    pub hits: u32,
    /// Mean spray `control` (0–100).
    pub control: Option<f32>,
    /// Mean spray `mean_err` (degrees).
    pub mean_err: Option<f32>,
    /// First bullet whose median |err| exceeds [`BREAK_DEG`] (with ≥ [`MIN_BREAK_SAMPLES`] sprays).
    pub break_shot: Option<u32>,
    pub bullets: Vec<BulletAgg>,
}

/// Average pattern per bullet number for one weapon's sprays.
pub fn aggregate<'a>(weapon: &str, sprays: impl IntoIterator<Item = &'a Spray>) -> WeaponPattern {
    #[derive(Default)]
    struct Acc {
        n: u32,
        s: [f32; 7],
        errs: Vec<f32>,
    }
    let mut acc: Vec<Acc> = vec![];
    let (mut count, mut shots, mut hits) = (0u32, 0u32, 0u32);
    let (mut ctrl_sum, mut ctrl_n, mut err_sum) = (0f32, 0u32, 0f32);
    for sp in sprays {
        count += 1;
        shots += sp.shots.len() as u32;
        hits += sp.hits;
        err_sum += sp.mean_err;
        if let Some(c) = sp.control {
            ctrl_sum += c;
            ctrl_n += 1;
        }
        for sh in &sp.shots {
            let i = sh.idx as usize - 1;
            if acc.len() <= i {
                acc.resize_with(i + 1, Acc::default);
            }
            let a = &mut acc[i];
            a.n += 1;
            for (k, v) in [sh.recoil_v, sh.recoil_h, sh.aim_v, sh.aim_h, sh.err_v, sh.err_h, sh.err()].into_iter().enumerate() {
                a.s[k] += v;
            }
            a.errs.push(sh.err());
        }
    }
    let bullets: Vec<BulletAgg> = acc
        .iter()
        .enumerate()
        .filter(|(_, a)| a.n > 0)
        .map(|(i, a)| {
            let m = |k: usize| a.s[k] / a.n as f32;
            BulletAgg {
                idx: i as u32 + 1,
                n: a.n,
                recoil_v: m(0),
                recoil_h: m(1),
                aim_v: m(2),
                aim_h: m(3),
                err_v: m(4),
                err_h: m(5),
                err: m(6),
                err_median: crate::Dist::of(a.errs.iter().copied()).median.unwrap_or(0.0),
            }
        })
        .collect();
    let break_shot = bullets.iter().find(|b| b.n >= MIN_BREAK_SAMPLES && b.err_median > BREAK_DEG).map(|b| b.idx);
    WeaponPattern {
        weapon: weapon.to_string(),
        sprays: count,
        shots,
        hits,
        control: (ctrl_n > 0).then(|| ctrl_sum / ctrl_n as f32),
        mean_err: (count > 0).then(|| err_sum / count as f32),
        break_shot,
        bullets,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verified AK-47 punch sequence (docs/roadmap.md 6.1), bullets 1–7.
    const AK_PUNCH: [f32; 7] = [0.0, -0.10, -0.62, -1.44, -2.30, -3.10, -3.82];

    fn seq(view_pitch: impl Fn(usize) -> f32) -> Vec<(i32, f32, f32, f32, f32)> {
        AK_PUNCH.iter().enumerate().map(|(i, &pp)| (i as i32 * 6, view_pitch(i), 90.0, pp, 0.0)).collect()
    }

    #[test]
    fn perfect_compensation_has_zero_error() {
        // Pulling down by exactly 2 × punch keeps every bullet on the first one.
        let shots = compensation(&seq(|i| -2.0 * AK_PUNCH[i]));
        assert!(shots.iter().all(|s| s.err() < 1e-5));
        let (mean_err, _, _, control, break_shot) = summarize(&shots);
        assert!(mean_err < 1e-5);
        assert_eq!(control, Some(100.0));
        assert_eq!(break_shot, None);
        // Ideal pull-down at bullet 7 = 2 × 3.82 = 7.64°.
        assert!((shots[6].recoil_v - 7.64).abs() < 1e-4);
        assert!((shots[6].aim_v + 7.64).abs() < 1e-4);
    }

    #[test]
    fn no_compensation_is_under_and_over_is_negative() {
        let none = compensation(&seq(|_| 0.0));
        assert!((none[6].err_v - 7.64).abs() < 1e-4, "bullets climb: under-compensated, err_v > 0");
        let (_, mean_v, _, control, break_shot) = summarize(&none);
        assert!(mean_v > 0.0);
        assert_eq!(control, Some(0.0));
        assert_eq!(break_shot, Some(4)); // bullet 3: 1.24°, bullet 4: 2.88° > 2°
        let over = compensation(&seq(|i| -3.0 * AK_PUNCH[i]));
        assert!(over[6].err_v < 0.0, "pulled too far: over-compensated, err_v < 0");
    }

    #[test]
    fn yaw_wraps_and_right_is_positive() {
        // View turns from 179° to -179° (2° to the left in Source yaw, i.e. aim moved left).
        let shots = compensation(&[(0, 0.0, 179.0, 0.0, 0.0), (6, 0.0, -179.0, 0.0, 0.0)]);
        assert!((shots[1].aim_h + 2.0).abs() < 1e-4);
    }

    #[test]
    fn aggregate_break_shot_uses_median() {
        let mk = |pull: f32| {
            let shots = compensation(&seq(|i| -pull * AK_PUNCH[i]));
            let (mean_err, mean_err_v, mean_err_h, control, break_shot) = summarize(&shots);
            Spray {
                steamid: 1,
                round: 1,
                team: None,
                weapon: "ak47".into(),
                start_tick: 0,
                end_tick: 36,
                hits: 0,
                head_hits: 0,
                target: None,
                killed: false,
                start_speed: None,
                mean_err,
                mean_err_v,
                mean_err_h,
                control,
                break_shot,
                shots,
            }
        };
        // Two good sprays, one without any pull: the median ignores the outlier.
        let sprays = [mk(2.0), mk(2.0), mk(0.0)];
        let p = aggregate("ak47", sprays.iter());
        assert_eq!(p.sprays, 3);
        assert_eq!(p.bullets.len(), 7);
        assert_eq!(p.bullets[6].n, 3);
        assert_eq!(p.break_shot, None);
        assert!((p.bullets[6].err - 7.64 / 3.0).abs() < 1e-3);
        let p = aggregate("ak47", [mk(0.0), mk(0.0), mk(2.0)].iter());
        assert_eq!(p.break_shot, Some(4));
    }
}
