//! Engagement-window API against a real demo (skipped when the sample folder is missing).

use demo_parse::engagement::AnchorKind;
use demo_parse::{parse_demo_file, parse_engagements_file, EngagementOptions};
use std::path::PathBuf;

fn sample(rel: &str) -> Option<PathBuf> {
    let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_default();
    let p = PathBuf::from(root).join(rel);
    p.exists().then_some(p).or_else(|| {
        eprintln!("skip: {rel} not found");
        None
    })
}

#[test]
fn engagement_windows_valve() {
    let Some(path) = sample("Official/match730_003831848039841005782_0410013497_154_competitive.dem") else { return };
    let demo = parse_demo_file(&path).unwrap();
    let e = parse_engagements_file(&path, EngagementOptions::default()).unwrap();
    assert_eq!(e.tick_rate, 64.0);

    // One window per kill / player_hurt with two distinct players (no world / self damage).
    let kills = e.windows.iter().filter(|w| w.kind == AnchorKind::Kill).count();
    let pvp_kills = demo.kills.iter().filter(|k| k.attacker.is_some() && k.victim.is_some() && k.attacker != k.victim).count();
    assert_eq!(kills, pvp_kills);
    assert_eq!(e.windows.len(), 595);

    for w in &e.windows {
        let n = w.attacker_track.len();
        assert_eq!(n, w.victim_track.len());
        assert_eq!(w.start_tick + n as i32 - 1, w.tick);
        assert!(n <= 3 * 64 + 1);
        assert!(w.clipped_by_round_start || n == 3 * 64 + 1);
        let round = demo.rounds.iter().find(|r| r.number == w.round).unwrap();
        assert!(w.start_tick >= round.freeze_end_tick.unwrap_or(round.start_tick));
    }
    // Full-rate tracks: attacker state present on virtually every tick.
    let total: usize = e.windows.iter().map(|w| w.attacker_track.len()).sum();
    let present: usize = e.windows.iter().map(|w| w.attacker_track.iter().filter(|s| s.is_some()).count()).sum();
    assert!(present * 1000 >= total * 999);
    // Most enemy kills happen while the attacker sees the victim.
    let seen = e
        .windows
        .iter()
        .filter(|w| w.kind == AnchorKind::Kill && w.attacker_track.last().unwrap().is_some_and(|s| s.sees_other))
        .count();
    assert!(seen * 10 >= kills * 7, "{seen}/{kills}");

    // One sight per weapon_fire, velocities in a physical range (CS2 max run speed 250 u/s, more
    // only when falling / boosted).
    assert_eq!(e.shot_sights.len(), demo.shots.len());
    let speeds: Vec<f32> = e.shot_sights.iter().filter_map(|s| s.velocity).map(|v| v.x.hypot(v.y)).collect();
    assert!(speeds.len() * 100 >= e.shot_sights.len() * 99);
    let fast = speeds.iter().filter(|v| **v > 320.0).count();
    assert!(fast * 200 <= speeds.len(), "{fast} shots faster than 320 u/s");
    assert!(e.shot_sights.iter().any(|s| !s.visible_enemies.is_empty()));
}
