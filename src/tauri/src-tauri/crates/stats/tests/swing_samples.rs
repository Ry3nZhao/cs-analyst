//! Swing on the three sample demos (skipped when the sample folder is missing).
//!
//! Invariants checked on every round, then a few players' match Swing pinned and the ranking
//! compared with ADR / clutches. Run with: `cargo test -p stats --release --test swing_samples -- --nocapture`

mod common;
use common::*;
use stats::{MatchStats, PlayerMatchStats, TEAM_CT, TEAM_T};

const ME: u64 = 76561198000000042;

fn load(rel: &str) -> Option<MatchStats> {
    let path = sample(rel)?;
    let d = demo_parse::parse_demo_file(&path).expect("parse");
    Some(stats::compute((&d).into()))
}

fn player(s: &MatchStats, sid: u64) -> &PlayerMatchStats {
    s.players.iter().find(|p| p.steamid == sid).expect("player")
}

fn check_invariants(s: &MatchStats) {
    assert_eq!(s.win_probability.len(), s.rounds.len());
    for (curve, r) in s.win_probability.iter().zip(&s.rounds) {
        assert_eq!(curve.round, r.number);
        let rows: Vec<_> = s.player_rounds.iter().filter(|p| p.round == r.number).collect();
        let side_sum = |side: u8| rows.iter().filter(|p| p.side == side).map(|p| p.swing as f64).sum::<f64>();
        let (t, ct) = (side_sum(TEAM_T), side_sum(TEAM_CT));
        // Zero-sum per event → the two sides cancel out.
        assert!((t + ct).abs() < 0.05, "round {} T {t:.3} + CT {ct:.3}", r.number);
        let Some(w) = r.winner_side else { continue };
        // The curve converges to the result.
        let last = curve.points.last().unwrap();
        assert_eq!(last.kind, "end");
        assert_eq!(last.p_after, if w == TEAM_T { 1.0 } else { 0.0 }, "round {}", r.number);
        // Winners' total is positive and equals the probability they gained since freeze end.
        let gain_t = 100.0 * (last.p_after as f64 - curve.p_start as f64);
        assert!((t - gain_t).abs() < 0.05, "round {} T swing {t:.3} vs Δp {gain_t:.3}", r.number);
        let winners = if w == TEAM_T { t } else { ct };
        assert!(winners > 0.0, "round {} winners' swing {winners}", r.number);
        // Probabilities stay in [0, 1]; points are in tick order.
        for p in &curve.points {
            assert!((0.0..=1.0).contains(&p.p_before) && (0.0..=1.0).contains(&p.p_after));
        }
        assert!(curve.points.windows(2).all(|w| w[0].tick <= w[1].tick));
    }
    // Match Swing = mean of the per-round values.
    for p in &s.players {
        let sum: f32 = s.player_rounds.iter().filter(|r| r.steamid == p.steamid).map(|r| r.swing).sum();
        assert!((sum - p.swing_total).abs() < 1e-3);
        assert!((p.derived.swing - p.swing_total / p.rounds_played as f32).abs() < 1e-4);
    }
}

/// Print the ranking, return players sorted by Swing (desc).
fn ranking(s: &MatchStats) -> Vec<&PlayerMatchStats> {
    let mut v: Vec<&PlayerMatchStats> = s.players.iter().collect();
    v.sort_by(|a, b| b.derived.swing.total_cmp(&a.derived.swing));
    for p in &v {
        let cw: u32 = p.clutch_wins.iter().sum();
        eprintln!(
            "{:>17} {:<16} {:?} swing {:>+6.2}  ADR {:>5.1}  K/D {:>2}/{:<2}  rating {:.2}  clutch wins {}",
            p.steamid,
            p.name.chars().take(16).collect::<String>(),
            p.team.unwrap(),
            p.derived.swing,
            p.derived.adr,
            p.kills,
            p.deaths,
            p.derived.rating,
            cw
        );
    }
    v
}

/// Spearman rank correlation between Swing and another per-player value.
fn rank_corr(v: &[&PlayerMatchStats], f: impl Fn(&PlayerMatchStats) -> f32) -> f32 {
    let rank = |vals: Vec<f32>| -> Vec<f32> {
        let mut idx: Vec<usize> = (0..vals.len()).collect();
        idx.sort_by(|a, b| vals[*a].total_cmp(&vals[*b]));
        let mut r = vec![0.0; vals.len()];
        for (i, j) in idx.into_iter().enumerate() {
            r[j] = i as f32;
        }
        r
    };
    let a = rank(v.iter().map(|p| p.derived.swing).collect());
    let b = rank(v.iter().map(|p| f(p)).collect());
    let n = v.len() as f32;
    let d2: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).powi(2)).sum();
    1.0 - 6.0 * d2 / (n * (n * n - 1.0))
}

fn check(rel: &str, fixed: &[(u64, f32)]) {
    let Some(s) = load(rel) else { return };
    eprintln!("== {rel} ({}:{})", s.score_a, s.score_b);
    check_invariants(&s);
    let v = ranking(&s);
    let rho = rank_corr(&v, |p| p.derived.rating);
    eprintln!("Spearman(swing, HLTV 2.0) = {rho:.2}, Spearman(swing, ADR) = {:.2}", rank_corr(&v, |p| p.derived.adr));
    assert!(rho > 0.6, "Swing should broadly agree with HLTV 2.0 ({rho})");
    // Winners average a positive Swing, losers a negative one.
    let team_avg = |won: bool| {
        let t: Vec<_> = s.players.iter().filter(|p| (p.wins > 0) == won).collect();
        t.iter().map(|p| p.derived.swing).sum::<f32>() / t.len() as f32
    };
    assert!(team_avg(true) > 0.0 && team_avg(false) < 0.0);
    // The best / worst by Swing are among the best / worst three by HLTV 2.0.
    let mut by_rating: Vec<u64> = v.iter().map(|p| p.steamid).collect();
    by_rating.sort_by(|a, b| player(&s, *b).derived.rating.total_cmp(&player(&s, *a).derived.rating));
    assert!(by_rating[..3].contains(&v[0].steamid), "top Swing {} not in rating top 3", v[0].name);
    assert!(by_rating[7..].contains(&v[9].steamid), "bottom Swing {} not in rating bottom 3", v[9].name);
    for (sid, swing) in fixed {
        let p = player(&s, *sid);
        assert!((p.derived.swing - swing).abs() < 0.01, "{} swing {} (pinned {swing})", p.name, p.derived.swing);
    }
}

#[test]
fn valve_matchmaking() {
    // SHADOW 19/8 ADR 114; JustSad 4/16 ADR 28. 0VERK1LL (ADR 42) is lifted by a won clutch.
    check(OFFICIAL, &[(76561199190641063, 7.93), (ME, -1.30), (76561198808257079, -13.23), (76561198833107204, 2.74)]);
}

#[test]
fn third_party_platform() {
    check(PLATFORM, &[(76561199752844438, 6.19), (ME, 1.25), (76561198771652651, -4.64)]);
}

#[test]
fn pro_match() {
    check(PRO, &[(76561197961491680, 8.63), (76561198191236195, -8.57)]);
}

#[test]
fn json_shape() {
    let Some(s) = load(PLATFORM) else { return };
    let v = serde_json::to_value(&s).unwrap();
    assert!(v["players"][0]["derived"]["swing"].is_number());
    assert!(v["playerRounds"][0]["swing"].is_number());
    let pt = &v["winProbability"][0]["points"][0];
    assert_eq!(pt["kind"], "start");
    assert!(pt["pBefore"].is_number() && pt["tAlive"].is_number());
    assert!(v.get("swingSamples").is_none());
    assert!(ME > 0);
}
