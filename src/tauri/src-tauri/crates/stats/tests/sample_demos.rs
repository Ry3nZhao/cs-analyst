//! Regression tests on the three sample demos (skipped when the sample folder is missing).
//! Sample folder and main player come from `CS_ANALYST_SAMPLE_DEMOS` / `CS_ANALYST_SAMPLE_STEAMID` (see `demo_parse::sample`).
//!
//! Besides fixed numbers, every player is cross-checked against the in-game scoreboard stored in
//! the demo (`CCSPlayerController_ActionTrackingServices`, read at the last tick).
//!
//! Run with: `cargo test -p stats --release -- --nocapture`

mod common;
use common::*;
use demo_parse::ParsedDemo;
use stats::{BuyType, MatchStats, PlayerMatchStats, Team, TEAM_CT, TEAM_T};

const ME: u64 = demo_parse::sample::PLAYER;

fn load(rel: &str) -> Option<(std::path::PathBuf, ParsedDemo, MatchStats)> {
    let path = sample(rel)?;
    let d = demo_parse::parse_demo_file(&path).expect("parse");
    let s = stats::compute((&d).into());
    Some((path, d, s))
}

fn player(s: &MatchStats, sid: u64) -> &PlayerMatchStats {
    s.players.iter().find(|p| p.steamid == sid).expect("player")
}

/// (steamid, K, D, A, damage, KAST rounds)
type Fixed = (u64, u32, u32, u32, u32, u32);

struct Expect {
    score: (u32, u32),
    blind: bool,
    fixed: [Fixed; 2],
    /// Players whose kill count legitimately differs from the game's by one (see docs/stats.md).
    kill_tolerance: u32,
    /// Allowed relative damage deviation from the game scoreboard.
    damage_tolerance: f32,
}

fn check(rel: &str, e: Expect) {
    let Some((path, d, s)) = load(rel) else { return };
    let board = scoreboard(&path, d.header.last_tick);
    eprintln!("== {rel} ({}:{})", s.score_a, s.score_b);

    // ---- match-level ----
    assert_eq!((s.score_a, s.score_b), e.score, "score A:B");
    assert_eq!(s.blind_data_available, e.blind);
    assert_eq!(s.players.len(), 10);
    assert_eq!(s.players.iter().filter(|p| p.team == Some(Team::A)).count(), 5);
    assert_eq!(s.rounds.len(), d.rounds.len());
    assert_eq!(s.team_rounds.len(), 2 * d.rounds.len());
    // Score after the last round equals the number of rounds each team won.
    let wins = |t| s.team_rounds.iter().filter(|r| r.team == t && r.won).count() as u32;
    assert_eq!((wins(Team::A), wins(Team::B)), e.score);
    // Round 1 is a pistol round on both sides; team A starts on CT.
    assert!(s.team_rounds.iter().filter(|r| r.round == 1).all(|r| r.buy_type == BuyType::Pistol));
    assert_eq!(s.rounds[0].side_a, TEAM_CT);
    // Halftime swap detected: team A plays T in round 13 and that round is a pistol round.
    let r13 = s.team_rounds.iter().find(|r| r.round == 13 && r.team == Team::A).unwrap();
    assert_eq!((r13.side, r13.buy_type), (TEAM_T, BuyType::Pistol));

    // Every round with a kill has exactly one opening kill.
    let opening: u32 = s.players.iter().map(|p| p.opening_kills_t + p.opening_kills_ct).sum();
    let opening_deaths: u32 = s.players.iter().map(|p| p.opening_deaths_t + p.opening_deaths_ct).sum();
    assert_eq!(opening, d.rounds.len() as u32);
    assert_eq!(opening_deaths, opening);
    // Kill accounting: every enemy kill appears once in the kill feed.
    let kills: u32 = s.players.iter().map(|p| p.kills).sum();
    let feed_kills = s.rounds.iter().flat_map(|r| &r.kills).filter(|k| k.kind == "kill").count() as u32;
    assert_eq!(kills, feed_kills);
    // Trade kills and traded deaths are the two ends of the same events.
    let trade_kills = s.rounds.iter().flat_map(|r| &r.kills).filter(|k| k.trade).count() as u32;
    assert_eq!(s.players.iter().map(|p| p.trade_kills).sum::<u32>(), trade_kills);
    // At most one clutch per round.
    let clutches: u32 = s.players.iter().map(|p| p.clutch_attempts.iter().sum::<u32>()).sum();
    assert!(clutches <= d.rounds.len() as u32);

    for p in &s.players {
        let x = &p.derived;
        // Rounds without a T/CT economy snapshot (disconnected) are not played; one platform player
        // misses rounds 2–3.
        assert!(p.rounds_played + 2 >= d.rounds.len() as u32 && p.rounds_played <= d.rounds.len() as u32, "{} rounds", p.name);
        assert_eq!(p.rounds_t + p.rounds_ct, p.rounds_played);
        assert!((x.adr - p.damage as f32 / p.rounds_played as f32).abs() < 1e-3);
        assert!((x.kast - 100.0 * p.kast_rounds as f32 / p.rounds_played as f32).abs() < 1e-3);
        assert!(p.kast_rounds >= p.rounds_survived);
        assert!((0.0..3.0).contains(&x.rating), "{} rating {}", p.name, x.rating);
        assert!(p.headshot_kills <= p.kills);
        assert_eq!(p.enemies_flashed.is_some(), e.blind, "blind metrics are None, not 0, without blind data");
        assert_eq!(p.matches, 1);

        // Kills independently recounted from the raw events.
        let raw_k = d
            .kills
            .iter()
            .filter(|k| {
                k.attacker == Some(p.steamid)
                    && k.victim.is_some()
                    && k.victim != k.attacker
                    && k.attacker_team.is_some()
                    && k.attacker_team != k.victim_team
                    && k.tick <= d.rounds.last().unwrap().end_tick
            })
            .count() as u32;
        assert_eq!(p.kills, raw_k, "{} kills vs raw events", p.name);

        // In-game scoreboard.
        let b = board.get(&p.steamid).cloned().unwrap_or_default();
        eprintln!(
            "{:>17} {:<16} K{:>2}/{:<2} D{:>2}/{:<2} A{:>2}/{:<2} HS{:>2}/{:<2} DMG{:>5}/{:<5} KAST{:>5.1} ADR{:>6.1} RATING {:.2}",
            p.steamid,
            p.name.chars().take(16).collect::<String>(),
            p.kills,
            b.kills,
            p.deaths,
            b.deaths,
            p.assists,
            b.assists,
            p.headshot_kills,
            b.headshot_kills,
            p.damage,
            b.damage,
            x.kast,
            x.adr,
            x.rating
        );
        assert!((p.kills as i64 - b.kills).unsigned_abs() as u32 <= e.kill_tolerance, "{} kills vs game", p.name);
        // The game also counts `world` self-kills after the match ended; we drop post-match events.
        assert!(p.deaths as i64 <= b.deaths && b.deaths - p.deaths as i64 <= 1, "{} deaths vs game", p.name);
        assert!((p.assists as i64 - b.assists).abs() <= 1, "{} assists vs game", p.name);
        assert!((p.headshot_kills as i64 - b.headshot_kills).abs() <= e.kill_tolerance as i64, "{} HS vs game", p.name);
        let dev = (p.damage as f32 - b.damage as f32).abs() / (b.damage.max(1) as f32);
        assert!(dev <= e.damage_tolerance, "{} damage {} vs game {}", p.name, p.damage, b.damage);
    }

    for (sid, k, dth, a, dmg, kast) in e.fixed {
        let p = player(&s, sid);
        assert_eq!((p.kills, p.deaths, p.assists, p.damage, p.kast_rounds), (k, dth, a, dmg, kast), "fixed numbers for {}", p.name);
    }
}

#[test]
fn valve_matchmaking() {
    check(
        OFFICIAL,
        Expect {
            score: (4, 13),
            blind: true,
            fixed: [(ME, 10, 13, 4, 1665, 8), (76561198377969427, 21, 10, 4, 2114, 14)],
            kill_tolerance: 0,
            damage_tolerance: 0.02,
        },
    );
}

#[test]
fn third_party_platform() {
    check(
        PLATFORM,
        Expect {
            score: (13, 2),
            blind: true,
            fixed: [(ME, 15, 7, 3, 1318, 13), (76561199752844438, 17, 7, 4, 1768, 15)],
            kill_tolerance: 1,
            damage_tolerance: 0.10,
        },
    );
}

#[test]
fn pro_match_without_blind_events() {
    check(
        PRO,
        Expect {
            score: (5, 12),
            blind: false,
            fixed: [(76561197961491680, 20, 7, 0, 1676, 13), (76561198060483793, 5, 12, 4, 990, 12)],
            kill_tolerance: 0,
            damage_tolerance: 0.02,
        },
    );
}

#[test]
fn json_shape() {
    let Some((_, _, s)) = load(PLATFORM) else { return };
    let v = serde_json::to_value(&s).unwrap();
    let p = &v["players"][0];
    assert!(p["steamid"].is_string(), "steamid is serialized as a string");
    assert!(p["derived"]["rating"].is_number());
    assert_eq!(v["teamRounds"][0]["buyType"], "pistol");
    assert!(v["rounds"][0]["kills"][0]["timeSecs"].is_number());
}
