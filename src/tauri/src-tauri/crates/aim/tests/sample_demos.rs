//! Regression tests against real demos. Set `CS_ANALYST_SAMPLE_DEMOS` to the sample root
//! (`CS_ANALYST_SAMPLE_DEMOS`, see `demo_parse::sample`); tests are skipped when a file is missing.
//!
//! Run with: `cargo test -p aim -- --nocapture`

use aim::{AimReport, Role, ALL_WEAPONS};
use demo_parse::ParsedDemo;
use std::path::PathBuf;

const ME: u64 = demo_parse::sample::PLAYER;
const VALVE: &str = "Official/match730_003831848039841005782_0410013497_154_competitive.dem";
const PLATFORM: &str = "Platforms/g161-n-20260709134236148390830.dem";
const PRO: &str = "Profs/2026major/stage1/nrg-vs-big-m3-mirage.dem";

fn run(rel: &str) -> Option<(ParsedDemo, AimReport)> {
    let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_default();
    let p = PathBuf::from(root).join(rel);
    if !p.exists() {
        eprintln!("skip: {} not found", p.display());
        return None;
    }
    let demo = demo_parse::parse_demo_file(&p).expect("parse");
    let eng = demo_parse::parse_engagements_file(&p, Default::default()).expect("engagements");
    let r = aim::analyze(&demo, &eng);
    eprintln!(
        "{rel}: visibility={:?} samples={} sprays={}",
        r.visibility_quality,
        r.engagements.len(),
        r.sprays.len()
    );
    Some((demo, r))
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn ak_control(r: &AimReport) -> f32 {
    let c: Vec<f32> = r.sprays.iter().filter(|s| s.weapon == "ak47").filter_map(|s| s.control).collect();
    c.iter().sum::<f32>() / c.len() as f32
}

fn median_spray_err(r: &AimReport) -> f32 {
    aim::Dist::of(r.sprays.iter().map(|s| s.mean_err)).median.unwrap()
}

/// A fixed AK-47 spray of the user (round 3, 8 bullets): punch sequence and compensation error.
#[test]
fn ak_spray_sample() {
    let Some((_, r)) = run(VALVE) else { return };
    let s = r.sprays.iter().find(|s| s.steamid == ME && s.start_tick == 15230).expect("pinned spray");
    assert_eq!(s.weapon, "ak47");
    assert_eq!((s.round, s.end_tick, s.shots.len()), (3, 15275, 8));
    for x in &s.shots {
        eprintln!(
            "#{} punch=({:.3},{:.3}) recoil=({:.3},{:.3}) aim=({:.3},{:.3}) err=({:.3},{:.3}) hit={}",
            x.idx, x.punch_pitch, x.punch_yaw, x.recoil_v, x.recoil_h, x.aim_v, x.aim_h, x.err_v, x.err_h, x.hit
        );
    }
    eprintln!("control={:?} break={:?} mean_err={}", s.control, s.break_shot, s.mean_err);

    // Bullets 1–7 follow the verified fixed AK recoil (docs/roadmap.md 6.1).
    let punch = [0.0, -0.10, -0.62, -1.44, -2.30, -3.10, -3.82];
    for (x, p) in s.shots.iter().zip(punch) {
        assert!(close(x.punch_pitch, p, 0.006), "bullet {} punch {} vs {p}", x.idx, x.punch_pitch);
        assert!(close(x.recoil_v, -2.0 * p, 0.012));
    }
    // Compensation error relative to bullet 1 (v > 0 = under-compensated, h > 0 = right).
    let err = [(0.0, 0.0), (-0.17, 1.12), (-0.83, 1.02), (-0.41, -0.10), (1.19, -0.12), (2.36, 0.45), (2.95, 1.55), (2.58, 0.61)];
    for (x, (v, h)) in s.shots.iter().zip(err) {
        assert!(close(x.err_v, v, 0.01) && close(x.err_h, h, 0.01), "bullet {}: ({}, {}) vs ({v}, {h})", x.idx, x.err_v, x.err_h);
        assert!(close(x.err_v, x.recoil_v + x.aim_v, 1e-4));
    }
    // Pulled 0.4–0.8° short of the ideal 7.64° by bullet 7: under-compensated late in the spray.
    assert!(s.shots[6].err_v > 0.0);
    assert_eq!(s.break_shot, Some(6));
    assert_eq!(s.hits, 3);
    assert!(close(s.control.unwrap(), 57.9, 0.5), "control {:?}", s.control);
}

#[test]
fn valve_visibility_metrics() {
    let Some((demo, r)) = run(VALVE) else { return };
    assert!(r.visibility_reliable);
    assert!(close(r.visibility_quality.unwrap(), 0.782, 0.005));
    assert_eq!(r.engagements.len(), 218);
    assert_eq!(r.sprays.len(), 99);

    for e in &r.engagements {
        assert!(e.onset_tick <= e.damage_tick);
        if let (Some(t), Some(re)) = (e.ttd_ms, e.reaction_ms) {
            assert!(re <= t, "reaction after damage: {e:?}");
        }
        if let Some(cp) = e.cp_deg {
            assert!(cp <= aim::engage::MAX_CP_DEG);
            // Small angles: total ≈ hypot(v, h), a bit less off the horizon (yaw shrinks by cos pitch).
            let hyp = e.cp_v.unwrap().hypot(e.cp_h.unwrap());
            if cp < 10.0 {
                assert!(cp <= hyp + 0.05 && cp >= 0.8 * hyp, "{cp} vs {hyp}");
            }
        }
        assert_eq!(e.ttd_ms.is_some(), e.role == Role::Attacker);
    }
    let all = |f: fn(&aim::EngagementSample) -> Option<f32>| aim::Dist::of(r.engagements.iter().filter_map(f));
    let (ttd, cp, react) = (all(|e| e.ttd_ms), all(|e| e.cp_deg), all(|e| e.reaction_ms));
    eprintln!("ttd={ttd:?}\ncp={cp:?}\nreaction={react:?}");
    // Plausible ranges for public matchmaking (spotted-mask onset lags real visibility a little,
    // so these read a bit faster / tighter than tools that ray-cast against map geometry).
    assert!((150.0..=600.0).contains(&ttd.median.unwrap()));
    assert!((1.0..=10.0).contains(&cp.median.unwrap()));
    assert!(react.median.unwrap() <= ttd.median.unwrap());
    assert!(close(ttd.median.unwrap(), 265.625, 0.01));
    assert!(close(cp.median.unwrap(), 3.25, 0.01));

    // Hand-checked contacts (crates/aim/examples/inspect.rs): onset, TTD, reaction, crosshair.
    let find = |tick: i32| r.engagements.iter().find(|e| e.observer == ME && e.damage_tick == tick).unwrap();
    let e = find(12729);
    assert_eq!((e.onset_tick, e.weapon.as_deref(), e.killed), (12715, Some("hkp2000"), true));
    assert_eq!((e.ttd_ms, e.reaction_ms), (Some(218.75), Some(218.75)));
    assert!(close(e.cp_deg.unwrap(), 0.97, 0.01) && close(e.cp_v.unwrap(), -0.29, 0.01) && close(e.cp_h.unwrap(), 0.93, 0.01));
    let e = find(33113);
    assert_eq!((e.onset_tick, e.weapon.as_deref()), (33088, Some("m4a1")));
    assert_eq!((e.ttd_ms, e.reaction_ms), (Some(390.625), Some(296.875)));
    assert!(close(e.cp_deg.unwrap(), 3.02, 0.01) && close(e.cp_v.unwrap(), -1.28, 0.01) && close(e.cp_h.unwrap(), 2.73, 0.01));

    // Summary rows: one per player × weapon + one '*' row; weapon rows add up to '*'.
    for p in &demo.players {
        let rows: Vec<_> = r.players.iter().filter(|x| x.steamid == p.steamid).collect();
        let all = rows.iter().find(|x| x.weapon == ALL_WEAPONS).unwrap();
        let shots: u32 = rows.iter().filter(|x| x.weapon != ALL_WEAPONS).map(|x| x.counters.shots).sum();
        assert_eq!(all.counters.shots, shots);
        assert!(all.counters.vis_shots.is_some());
        assert!(all.counters.cs_ok <= all.counters.cs_shots);
    }
    let me = r.players.iter().find(|x| x.steamid == ME && x.weapon == ALL_WEAPONS).unwrap();
    eprintln!("me: {me:?}");
    assert!(close(me.ttd.median.unwrap(), 195.3, 0.1));
    assert!(close(me.cp.median.unwrap(), 1.4, 0.05));
}

#[test]
fn platform_demo_weapon_names() {
    let Some((demo, r)) = run(PLATFORM) else { return };
    assert!(r.visibility_reliable);
    // 5E skins (`5e_2023pass1_usp_silencer` in kill events) count as the base weapon.
    let skinned: Vec<_> = demo.kills.iter().filter(|d| d.weapon.starts_with("5e_")).collect();
    assert!(!skinned.is_empty());
    assert!(skinned.iter().all(|d| aim::weapons::canonical(&d.weapon).is_some()));
    let gun_shots = demo.shots.iter().filter(|s| aim::weapons::canonical(&s.weapon).is_some()).count() as u32;
    let counted: u32 = r.players.iter().filter(|p| p.weapon == ALL_WEAPONS).map(|p| p.counters.shots).sum();
    assert_eq!(counted, gun_shots);
    assert!(r.players.iter().any(|p| p.weapon == "usp_silencer"));
    let me = r.players.iter().find(|x| x.steamid == ME && x.weapon == ALL_WEAPONS).expect("user plays in the platform demo");
    assert!(me.ttd.n > 0 && me.sprays > 0);
}

/// Pro (ESL) demos: the spotted mask is too sparse for visibility metrics, but spray control and
/// spray error are clearly better than in public matches.
#[test]
fn pro_vs_public() {
    let Some((_, pro)) = run(PRO) else { return };
    let Some((_, valve)) = run(VALVE) else { return };
    let Some((_, platform)) = run(PLATFORM) else { return };
    assert!(!pro.visibility_reliable && pro.visibility_quality.unwrap() < 0.5);
    assert!(pro.engagements.is_empty());
    assert!(pro.players.iter().all(|p| p.counters.vis_shots.is_none() && p.ttd.n == 0));

    let (cp, cv, cpl) = (ak_control(&pro), ak_control(&valve), ak_control(&platform));
    let (ep, ev, epl) = (median_spray_err(&pro), median_spray_err(&valve), median_spray_err(&platform));
    eprintln!("AK control pro={cp:.1} valve={cv:.1} platform={cpl:.1}; median spray err pro={ep:.2} valve={ev:.2} platform={epl:.2}");
    assert!(cp > cv + 5.0 && cp > cpl + 5.0);
    assert!(ep < ev - 0.3 && ep < epl - 0.3);

    // Aggregated AK pattern: pros "break" (median error > 2°) later than the public lobbies.
    let brk = |r: &AimReport| aim::aggregate("ak47", r.sprays.iter().filter(|s| s.weapon == "ak47")).break_shot.unwrap_or(99);
    eprintln!("AK break shot pro={} valve={} platform={}", brk(&pro), brk(&valve), brk(&platform));
    assert!(brk(&pro) > brk(&valve) && brk(&pro) > brk(&platform));
}
