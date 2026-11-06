//! Derived statistics (W1-B): compute with the `stats` crate after each ingest, persist to the
//! migration-003 tables, backfill on startup, and serve the match-detail / player-stats commands.
//! Swing (W2-B) rides along: migration-006 columns / tables, `get_round_win_probability`.

pub mod commands;
pub mod repo;

use crate::database::{Database, Result};
use parking_lot::Mutex;
use std::sync::Arc;

fn log(msg: impl AsRef<str>) {
    eprintln!("[stats] {}", msg.as_ref());
}

/// Called by the analysis worker right after a successful ingest (DB lock held). Failures are
/// logged only: event data stays valid and the startup backfill retries.
pub fn on_match_ingested(db: &Database, match_id: &str, demo: &demo_parse::ParsedDemo) {
    let model = repo::load_win_model(db).unwrap_or_default();
    let s = ::stats::compute_with_model(demo.into(), &model);
    if let Err(e) = repo::replace_match_stats(db, match_id, &s) {
        log(format!("write failed {match_id}: {e}"));
    }
}

/// Recompute a match's stats from its stored events. Returns `None` if it has no rounds.
pub fn recompute_from_db(db: &Database, match_id: &str) -> Result<Option<::stats::MatchStats>> {
    let ev = repo::load_events(db, match_id)?;
    if ev.rounds.is_empty() {
        return Ok(None);
    }
    let s = ::stats::compute_with_model(ev.view(), &repo::load_win_model(db)?);
    repo::replace_match_stats(db, match_id, &s)?;
    Ok(Some(s))
}

/// Recompute stats for analyzed matches whose stats are missing or outdated (`STATS_VERSION`
/// bump). Runs on a background thread; locks the database one match at a time.
pub fn spawn_backfill(db: Arc<Mutex<Database>>) {
    std::thread::Builder::new()
        .name("stats-backfill".into())
        .spawn(move || {
            let ids = repo::list_matches_needing_stats(&db.lock()).unwrap_or_default();
            if ids.is_empty() {
                return;
            }
            log(format!("backfilling {} match(es)", ids.len()));
            for id in ids {
                if let Err(e) = recompute_from_db(&db.lock(), &id) {
                    log(format!("backfill failed {id}: {e}"));
                }
            }
        })
        .expect("spawn stats backfill");
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::path::PathBuf;

    /// Ingest a real demo, then check: stored rows, DB reload gives identical stats, the two commands.
    #[test]
    fn ingest_recompute_and_commands() {
        let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_default();
        let path = PathBuf::from(root).join("Platforms/g161-n-20260709134236148390830.dem");
        if !path.exists() {
            eprintln!("skip: {} not found", path.display());
            return;
        }
        let demo = demo_parse::parse_demo_file(&path).unwrap();
        let dir = std::env::temp_dir().join(format!("cs-analyst-stats-test-{}-{}", std::process::id(), uuid::Uuid::new_v4()));
        let db = Database::open(dir.clone()).unwrap();
        let v: i32 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert!(v >= 3);
        db.conn
            .execute(
                "INSERT INTO matches (id, platform, match_id, status, file_path, map_name, played_at)
                 VALUES ('m1', '5eplay', 'm1', 'completed', 'demos/x.dem', 'de_dust2', '2026-07-09T13:42:36Z')",
                [],
            )
            .unwrap();
        crate::database::analysis::replace_match_analysis(&db, "m1", &demo, None, 1).unwrap();
        on_match_ingested(&db, "m1", &demo);

        let count = |t: &str| -> i64 { db.conn.query_row(&format!("SELECT COUNT(*) FROM {t} WHERE match_id='m1'"), [], |r| r.get(0)).unwrap() };
        assert_eq!(count("player_match_stats"), 10);
        assert_eq!(count("team_round_stats"), 30);
        assert_eq!(count("player_round_stats"), 148, "10 players × 15 rounds minus 2 disconnected rounds");
        assert!(repo::list_matches_needing_stats(&db).unwrap().is_empty());

        // Recomputing from stored rows reproduces the in-memory result.
        let from_demo = ::stats::compute((&demo).into());
        let from_db = recompute_from_db(&db, "m1").unwrap().unwrap();
        let key = |s: &::stats::MatchStats| serde_json::to_string(&s.players).unwrap();
        assert_eq!(key(&from_demo), key(&from_db));
        assert_eq!(serde_json::to_string(&from_demo.team_rounds).unwrap(), serde_json::to_string(&from_db.team_rounds).unwrap());

        // list_player_stats: stored row round-trips (all columns) and the summary matches.
        let list = commands::list_player_stats_impl(&db, demo_parse::sample::PLAYER, &Default::default()).unwrap();
        assert_eq!(list.total, 1);
        let row = &list.matches[0];
        let mine = from_demo.players.iter().find(|p| p.steamid == demo_parse::sample::PLAYER).unwrap();
        let mut expect = mine.clone();
        expect.multi_kills[0] = 0;
        expect.multi_kills[1] = 0;
        assert_eq!(serde_json::to_value(&row.stats).unwrap(), serde_json::to_value(&expect).unwrap());
        assert_eq!((row.score_own, row.score_opponent, row.result), (13, 2, "win"));
        assert_eq!(list.summary.as_ref().unwrap().kills, 15);
        let none = commands::list_player_stats_impl(
            &db,
            demo_parse::sample::PLAYER,
            &commands::PlayerStatsFilters { map_name: Some("de_inferno".into()), ..Default::default() },
        )
        .unwrap();
        assert_eq!((none.total, none.summary.is_none()), (0, true));

        // get_match_detail JSON.
        let detail = commands::build_detail_for_test(&db, "m1").unwrap().unwrap();
        let j = serde_json::to_value(&detail).unwrap();
        assert_eq!(j["mapName"], "de_dust2");
        assert_eq!(j["rounds"].as_array().unwrap().len(), 15);
        assert_eq!(j["economy"].as_array().unwrap().len(), 30);
        assert_eq!(j["teams"][0]["score"], 13);
        assert!(j["scoreboard"][0]["steamid"].is_string());
        assert!(j["damageMatrix"].as_array().unwrap().len() > 10);
        let duel_kills: u64 = j["duelMatrix"].as_array().unwrap().iter().map(|c| c["kills"].as_u64().unwrap()).sum();
        let board_kills: u64 = detail.scoreboard.iter().map(|p| p.kills as u64).sum();
        assert_eq!(duel_kills, board_kills);
        assert!(commands::build_detail_for_test(&db, "nope").unwrap().is_none());

        // Swing: stored columns / tables and the two command shapes.
        assert!(j["scoreboard"][0]["derived"]["swing"].is_number());
        assert!(j["playerRounds"][0]["swing"].is_number());
        let stored: f64 = db
            .conn
            .query_row("SELECT swing FROM player_match_stats WHERE match_id='m1' AND steamid=?", params![demo_parse::sample::PLAYER as i64], |r| r.get(0))
            .unwrap();
        assert!((stored as f32 - mine.derived.swing).abs() < 1e-4);
        let curve_rows = count("round_win_probability");
        let points: usize = from_demo.win_probability.iter().map(|r| r.points.len()).sum();
        assert_eq!(curve_rows as usize, points);
        assert!(count("win_prob_samples") > 5);
        let wp = commands::build_win_probability_for_test(&db, "m1").unwrap().unwrap();
        let w = serde_json::to_value(&wp).unwrap();
        assert_eq!(w["rounds"].as_array().unwrap().len(), 15);
        assert_eq!(w["playerRounds"].as_array().unwrap().len(), 148);
        assert_eq!(w["model"]["usesLocalData"], false, "one match is below the local threshold");
        assert_eq!(w["model"]["localRounds"], 15);
        let last = w["rounds"][0]["points"].as_array().unwrap().last().unwrap().clone();
        assert_eq!(last["kind"], "end");
        assert!(last["pAfter"] == 0.0 || last["pAfter"] == 1.0);
        assert!(w["playerRounds"][0]["steamid"].is_string());
        assert!(commands::build_win_probability_for_test(&db, "nope").unwrap().is_none());
        // Once the library is large enough the local samples change the model.
        let model = repo::load_win_model(&db).unwrap();
        assert!(!model.uses_local());
        let local = ::stats::WinModel::with_local(
            [::stats::BucketSample { t_alive: 5, ct_alive: 5, planted: false, rounds: 600, t_wins: 600 }],
            600,
        );
        assert!(local.base(5, 5, false) > ::stats::swing::prior_base(5, 5, false));

        // Outdated stats are picked up by the backfill query; cascade on delete.
        db.conn.execute("UPDATE player_match_stats SET stats_version = 0 WHERE match_id = 'm1'", []).unwrap();
        assert_eq!(repo::list_matches_needing_stats(&db).unwrap(), vec!["m1".to_string()]);
        db.conn.execute("DELETE FROM matches WHERE id = ?", params!["m1"]).unwrap();
        assert_eq!(count("player_round_stats"), 0);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
