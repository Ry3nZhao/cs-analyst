//! Regression tests against real demos. Set `CS_ANALYST_SAMPLE_DEMOS` to the sample root
//! (default `<sample-dir>`); tests are skipped when a file is missing.
//!
//! Run with: `cargo test -p demo-parse --release -- --nocapture`

use demo_parse::{parse_demo_file, ParsedDemo, TEAM_CT, TEAM_T};
use std::path::PathBuf;

fn sample(rel: &str) -> Option<PathBuf> {
    let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_else(|_| "<sample-dir>".into());
    let p = PathBuf::from(root).join(rel);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("skip: {} not found", p.display());
        None
    }
}

struct Expect {
    map: &'static str,
    rounds: usize,
    t_wins: usize,
    kills: usize,
}

fn summarize(d: &ParsedDemo) -> String {
    let with_punch = d.shots.iter().filter(|s| s.punch_pitch.is_some()).count();
    format!(
        "map={:?} rounds={} players={} kills={} damages={} shots={} (punch {}) grenades={} blinds={} bombs={} purchases={} econ={} replay_frames={}",
        d.header.map_name,
        d.rounds.len(),
        d.players.len(),
        d.kills.len(),
        d.damages.len(),
        d.shots.len(),
        with_punch,
        d.grenades.len(),
        d.blinds.len(),
        d.bombs.len(),
        d.purchases.len(),
        d.economies.len(),
        d.replay.n_frames(),
    )
}

fn check(rel: &str, exp: Expect) {
    let Some(path) = sample(rel) else { return };
    let t = std::time::Instant::now();
    let d = parse_demo_file(&path).expect("parse");
    eprintln!("{rel}: {} ({:.1}s)", summarize(&d), t.elapsed().as_secs_f64());

    assert_eq!(d.header.map_name.as_deref(), Some(exp.map));
    assert_eq!(d.rounds.len(), exp.rounds, "round count");
    assert_eq!(d.rounds.iter().filter(|r| r.winner == Some(TEAM_T)).count(), exp.t_wins, "T round wins");
    assert_eq!(d.kills.len(), exp.kills, "kill count");

    // Rounds are 1..=n, contiguous and ordered in time.
    for (i, r) in d.rounds.iter().enumerate() {
        assert_eq!(r.number as usize, i + 1);
        assert!(r.start_tick < r.end_tick);
    }

    // 10 real players, 5 per starting side.
    assert_eq!(d.players.len(), 10);
    assert!(d.players.iter().all(|p| p.steamid > 76561197960265728));
    assert_eq!(d.players.iter().filter(|p| p.team_start == Some(TEAM_T)).count(), 5);
    assert_eq!(d.players.iter().filter(|p| p.team_start == Some(TEAM_CT)).count(), 5);

    // Every event lands in a known round.
    let n = d.rounds.len() as u32;
    assert!(d.kills.iter().all(|k| (1..=n).contains(&k.round)));
    assert!(d.damages.iter().all(|x| (1..=n).contains(&x.round)));

    // Shots carry shooter state: position, view angles and recoil.
    let rifle: Vec<_> = d.shots.iter().filter(|s| s.weapon == "ak47" || s.weapon.starts_with("m4a")).collect();
    assert!(!rifle.is_empty());
    let joined = rifle.iter().filter(|s| s.pos.is_some() && s.pitch.is_some() && s.punch_pitch.is_some()).count();
    assert!(joined * 100 >= rifle.len() * 95, "only {joined}/{} rifle shots joined to tick state", rifle.len());

    // Kills have positions and teams for nearly all entries (world/bomb kills have no attacker).
    let positioned = d.kills.iter().filter(|k| k.victim_pos.is_some() && k.victim_team.is_some()).count();
    assert!(positioned * 100 >= d.kills.len() * 95);

    // Economy row per player per round (players may disconnect, allow a little slack).
    assert!(d.economies.len() >= d.rounds.len() * 9);

    // Replay: frames every 4 ticks, consistent column lengths, alive players have positions.
    let r = &d.replay;
    assert!(r.n_frames() > 1000);
    assert_eq!(r.x.len(), r.n_frames() * r.n_players());
    assert_eq!(r.spotted_by.len(), r.x.len());
    assert!(r.spotted_by.iter().any(|m| *m != 0), "visibility mask never set");
}

#[test]
fn valve_matchmaking() {
    check(
        "Official/match730_003831848039841005782_0410013497_154_competitive.dem",
        Expect { map: "de_ancient", rounds: 17, t_wins: 10, kills: 126 },
    );
}

#[test]
fn third_party_platform() {
    check("Platforms/g161-n-20260709134236148390830.dem", Expect { map: "de_dust2", rounds: 15, t_wins: 5, kills: 106 });
}

#[test]
fn pro_match() {
    check(
        "Profs/2026major/stage1/nrg-vs-big-m1-dust2-p1.dem",
        Expect { map: "de_dust2", rounds: 17, t_wins: 9, kills: 107 },
    );
}
