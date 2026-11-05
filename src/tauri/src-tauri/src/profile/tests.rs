//! Sample regression: the main player's three sample demos (Valve Premier + two platform demos).

use super::*;
use crate::database::Database;
use parking_lot::Mutex;
use rusqlite::params;
use std::path::PathBuf;
use std::sync::Arc;

const ME: u64 = 76561198000000042;

const SAMPLES: &[(&str, &str, &str)] = &[
    ("valve", "Official/match730_003831848039841005782_0410013497_154_competitive.dem", "2026-07-08 10:00:00"),
    ("p0708", "Platforms/g161-n-20260708160255640981434.dem", "2026-07-08 16:02:55"),
    ("p0709", "Platforms/g161-n-20260709134236148390830.dem", "2026-07-09 13:42:36"),
];

fn sample_root() -> PathBuf {
    PathBuf::from(std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_else(|_| "<sample-dir>".into()))
}

/// Ingest the samples like the analysis worker does: events → stats → identities → aim.
fn build_db() -> Option<(Arc<Mutex<Database>>, PathBuf)> {
    let paths: Vec<PathBuf> = SAMPLES.iter().map(|(_, rel, _)| sample_root().join(rel)).collect();
    if let Some(p) = paths.iter().find(|p| !p.exists()) {
        eprintln!("skip: {} not found", p.display());
        return None;
    }
    let dir = std::env::temp_dir().join(format!("cs-analyst-profile-test-{}", uuid::Uuid::new_v4()));
    let db = Database::open(dir.clone()).unwrap();
    // Parse in parallel (each demo takes a few seconds).
    let parsed: Vec<demo_parse::ParsedDemo> = std::thread::scope(|s| {
        let hs: Vec<_> = paths.iter().map(|p| s.spawn(move || demo_parse::parse_demo_file(p).unwrap())).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (((id, _, at), path), demo) in SAMPLES.iter().zip(&paths).zip(&parsed) {
        db.conn
            .execute(
                "INSERT INTO matches (id, platform, match_id, status, file_path, played_at, analysis_status)
                 VALUES (?1, 'imported', ?1, 'completed', ?2, ?3, 'done')",
                params![id, path.to_string_lossy(), at],
            )
            .unwrap();
        crate::database::analysis::replace_match_analysis(&db, id, demo, None, 1).unwrap();
        crate::stats::on_match_ingested(&db, id, demo);
        crate::identity::update_for_match(&db.conn, id).unwrap();
    }
    let db = Arc::new(Mutex::new(db));
    std::thread::scope(|s| {
        for (((id, _, _), path), demo) in SAMPLES.iter().zip(&paths).zip(&parsed) {
            let db = db.clone();
            s.spawn(move || crate::aim::analyze_match(&db, id, path, Some(demo)));
        }
    });
    Some((db, dir))
}

fn metric<'a>(p: &'a Profile, key: &str) -> &'a MetricSummary {
    p.metrics.iter().find(|m| m.key == key).unwrap()
}

fn close(a: Option<f64>, b: f64, eps: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() < eps)
}

#[test]
fn sample_profile() {
    let Some((db, dir)) = build_db() else { return };
    let db = db.lock();

    let p = build_profile(&db, ME, &ProfileOptions { window: Some(2), last_matches: None }).unwrap();
    let j = serde_json::to_value(&p).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&j["metrics"]).unwrap());
    eprintln!("summary {} tier {} sample {}", j["summary"], j["tier"], j["sample"]);
    eprintln!("bests {}", j["bests"]);
    eprintln!("focus {}", j["focus"]);
    eprintln!("sides {}", j["sides"]);
    eprintln!("maps {}", j["maps"]);
    eprintln!("weapons {}", j["weapons"]);
    eprintln!("trend {}", serde_json::to_string(&j["trend"]).unwrap());

    // Identity / header.
    assert_eq!(j["steamid"], ME.to_string());
    assert_eq!(p.steamids, vec![ME.to_string()]);
    assert!(p.person_id.is_some());
    assert_eq!(p.summary.matches, 3);
    assert_eq!(p.trend.matches.iter().map(|m| m.match_id.as_str()).collect::<Vec<_>>(), ["valve", "p0708", "p0709"]);

    // Aggregates are sums over matches, equal to the stored per-match counts.
    let rows = load::player_rows(&db.conn, Some(&[ME])).unwrap();
    assert_eq!(rows.len(), 3);
    let dmg: u32 = rows.iter().map(|r| r.damage).sum();
    let rounds: u32 = rows.iter().map(|r| r.rounds).sum();
    assert_eq!(p.summary.rounds, rounds);
    assert!(close(metric(&p, "adr").value, dmg as f64 / rounds as f64, 1e-9));
    let kills: u32 = rows.iter().map(|r| r.kills).sum();
    let deaths: u32 = rows.iter().map(|r| r.deaths).sum();
    assert!(close(metric(&p, "kd").value, kills as f64 / deaths as f64, 1e-9));
    // Rolling window 2: last point = aggregate of the newest two matches.
    let adr_series = p.trend.series.iter().find(|s| s.key == "adr").unwrap();
    let last2 = (rows[1].damage + rows[2].damage) as f64 / (rows[1].rounds + rows[2].rounds) as f64;
    assert!(close(adr_series.rolling[2], last2, 1e-9));
    assert!(close(metric(&p, "adr").recent, last2, 1e-9));
    assert_eq!(adr_series.rolling[0], adr_series.values[0]);

    // Aim metrics: matches without reliable visibility are skipped, not zero.
    let ttd = p.trend.series.iter().find(|s| s.key == "ttd").unwrap();
    let reliable: Vec<bool> = rows.iter().map(|r| r.aim.as_ref().is_some_and(|a| a.visibility_reliable)).collect();
    for (v, ok) in ttd.values.iter().zip(&reliable) {
        assert_eq!(v.is_some(), *ok);
    }
    assert_eq!(metric(&p, "ttd").matches_with_data as usize, reliable.iter().filter(|r| **r).count());

    // Tier: Premier from the Valve demo; too few same-tier players → whole library.
    let tier = p.tier.as_ref().expect("premier tier");
    assert_eq!((tier.system, tier.score, tier.band_lo, tier.band_hi), (ScoreSystem::Premier, 3392, 1, Some(4999)));
    assert_eq!(p.scores.len(), 1);
    assert_eq!(p.scores[0].points.len(), 1);
    assert!(p.sample.tier_players < compute::MIN_TIER_SAMPLE as u32);
    let adr = metric(&p, "adr");
    assert_eq!(adr.percentile.scope, SampleScope::All);
    assert_eq!(adr.percentile.sample_size, p.sample.library_players);
    assert!(adr.percentile.low_confidence);

    // Percentile = share of the library players (per steamid aggregate) that are worse.
    let others: Vec<MatchRow> = load::player_rows(&db.conn, None).unwrap().into_iter().filter(|r| r.steamid != ME).collect();
    let refs: Vec<&MatchRow> = others.iter().collect();
    let sample = sample_values("adr", 10.0, &refs);
    let expect = compute::percentile(adr.value.unwrap(), &sample, true);
    assert_eq!(adr.percentile.value, expect);

    // Focus: the lowest percentiles below 50, ascending, at most three.
    assert!(p.focus.len() <= 3);
    for w in p.focus.windows(2) {
        assert!(w[0].percentile <= w[1].percentile);
    }
    assert!(p.focus.iter().all(|f| f.percentile < 50.0));

    // Bests link to real matches.
    let best_kills = p.bests.iter().find(|b| b.key == "kills").unwrap();
    assert_eq!(best_kills.value as u32, rows.iter().map(|r| r.kills).max().unwrap());
    assert!(SAMPLES.iter().any(|(id, _, _)| *id == best_kills.match_id));

    // Sides: rounds add up; maps cover all matches.
    let side_rounds: u32 = p.sides.iter().map(|s| s.rounds).sum();
    assert_eq!(side_rounds, rounds);
    assert_eq!(p.maps.iter().map(|m| m.matches).sum::<u32>(), 3);
    // Weapon kills = scoreboard kills.
    assert_eq!(p.weapons.iter().map(|w| w.kills).sum::<u32>(), kills);

    // Pinned sample values (3 demos, main player). Changing a metric definition must update these.
    let s = &p.summary;
    assert_eq!((s.matches, s.wins, s.losses, s.draws, s.kills, s.deaths, s.rounds), (3, 2, 1, 0, 36, 25, 45));
    let pinned: &[(&str, f64, Option<f64>)] = &[
        ("adr", 85.8222, Some(59.26)),
        ("kast", 71.1111, Some(55.56)),
        ("kd", 1.44, Some(70.37)),
        ("rating", 1.2727, Some(62.96)),
        ("swing", 0.4923, Some(48.15)),
        ("openingDuel", 40.0, Some(43.75)),
        ("clutch", 0.0, Some(35.0)),
        ("ttd", 208.1653, Some(72.22)),
        ("crosshairPlacement", 2.0276, Some(94.44)),
        ("counterStrafe", 50.7813, Some(33.33)),
        ("sprayError", 3.1864, Some(45.45)),
    ];
    for (key, v, pct) in pinned {
        let m = metric(&p, key);
        assert!(close(m.value, *v, 1e-3), "{key}: {:?}", m.value);
        match pct {
            Some(x) => assert!(close(m.percentile.value, *x, 0.01), "{key} percentile: {:?}", m.percentile.value),
            None => assert!(m.percentile.value.is_none()),
        }
    }
    assert_eq!((metric(&p, "ttd").percentile.sample_size, metric(&p, "adr").percentile.sample_size), (18, 27));
    assert_eq!(p.focus.iter().map(|f| f.key).collect::<Vec<_>>(), ["counterStrafe", "clutch", "openingDuel"]);
    let best = |k: &str| p.bests.iter().find(|b| b.key == k).unwrap();
    assert_eq!((best("kills").value, best("kills").match_id.as_str()), (15.0, "p0709"));
    assert!((best("adr").value - 97.9412).abs() < 1e-3 && best("adr").match_id == "valve");
    assert_eq!(best("swing").match_id, "p0708");
    assert_eq!(best("multiKill").value, 3.0);
    assert_eq!(p.sides.iter().map(|s| (s.side, s.rounds)).collect::<Vec<_>>(), [(2, 9), (3, 36)]);
    assert_eq!((p.sample.tier_players, p.sample.library_players), (4, 27));

    // last_matches narrows everything to the newest N.
    let p1 = build_profile(&db, ME, &ProfileOptions { window: None, last_matches: Some(1) }).unwrap();
    assert_eq!(p1.summary.matches, 1);
    assert_eq!(p1.trend.matches[0].match_id, "p0709");
    assert_eq!(p1.window, DEFAULT_WINDOW);

    // With every player tagged into the same Premier band the tier sample is used.
    db.conn.execute("UPDATE match_players SET rank = 3000, rank_type = 11 WHERE steamid <> ?", params![ME as i64]).unwrap();
    let pt = build_profile(&db, ME, &Default::default()).unwrap();
    assert_eq!(pt.sample.tier_players, pt.sample.library_players);
    assert_eq!(metric(&pt, "adr").percentile.scope, SampleScope::Tier);
    assert_eq!(metric(&pt, "adr").percentile.value, adr.percentile.value);

    // Unknown player: empty profile, no panic.
    let empty = build_profile(&db, 76561190000000000, &Default::default()).unwrap();
    assert_eq!(empty.summary.matches, 0);
    assert!(empty.metrics.iter().all(|m| m.value.is_none() && m.percentile.value.is_none()));
    assert!(empty.focus.is_empty() && empty.bests.is_empty());

    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}
