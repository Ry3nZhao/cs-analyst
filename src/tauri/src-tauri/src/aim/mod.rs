//! Aim analysis (W1-D, migration 005). Runs after every demo ingest (`analysis` worker) and as a
//! one-off startup backfill for matches analysed before this module existed or with an older
//! `aim::AIM_VERSION`. Algorithms live in `crates/aim`; this module only parses, stores and
//! queries.

pub mod commands;
pub mod repo;

use crate::database::Database;
use parking_lot::Mutex;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

fn log(msg: impl AsRef<str>) {
    eprintln!("[aim] {}", msg.as_ref());
}

/// Parse the demo's engagement windows (and the demo itself when `demo` is `None`), run the aim
/// analysis and store it. Parsing happens without holding the DB lock. Failures are recorded in
/// `match_aim` and logged, never propagated: aim data is optional for the rest of the app.
pub fn analyze_match(db: &Arc<Mutex<Database>>, match_id: &str, path: &Path, demo: Option<&demo_parse::ParsedDemo>) {
    let started = Instant::now();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| compute(path, demo))).unwrap_or_else(|p| {
        let what = p
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown panic".into());
        Err(format!("枪法分析崩溃: {what}"))
    });
    let ms = started.elapsed().as_millis() as i64;
    let db = db.lock();
    match result.and_then(|r| repo::replace_match_aim(&db, match_id, &r, ms).map(|_| r).map_err(|e| format!("写入数据库失败: {e}"))) {
        Ok(r) => log(format!(
            "done {match_id}: {} sprays, {} contacts, visibility {:.2} in {ms} ms",
            r.sprays.len(),
            r.engagements.len(),
            r.visibility_quality.unwrap_or(0.0)
        )),
        Err(msg) => {
            log(format!("failed {match_id}: {msg}"));
            let _ = repo::mark_aim_failed(&db, match_id, &msg);
        }
    }
}

fn compute(path: &Path, demo: Option<&demo_parse::ParsedDemo>) -> Result<::aim::AimReport, String> {
    let owned;
    let demo = match demo {
        Some(d) => d,
        None => {
            owned = demo_parse::parse_demo_file(path).map_err(|e| format!("解析失败: {e}"))?;
            &owned
        }
    };
    let eng = demo_parse::parse_engagements_file(path, Default::default()).map_err(|e| format!("交火窗口解析失败: {e}"))?;
    Ok(::aim::analyze(demo, &eng))
}

/// Analyse, in the background, every successfully parsed match whose aim data is missing or
/// outdated. Call once from app setup.
pub fn start_backfill(db: Arc<Mutex<Database>>) {
    std::thread::Builder::new()
        .name("aim-backfill".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(8));
            let ids = repo::list_matches_needing_aim(&db.lock()).unwrap_or_default();
            if ids.is_empty() {
                return;
            }
            log(format!("backfill: {} match(es)", ids.len()));
            for id in ids {
                let path = crate::database::repo::get_match_abs_path(&db.lock(), &id).ok().flatten();
                match path {
                    Some(p) if p.exists() => analyze_match(&db, &id, &p, None),
                    _ => {
                        let _ = repo::mark_aim_failed(&db.lock(), &id, "demo 文件不存在");
                    }
                }
            }
        })
        .expect("spawn aim backfill");
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::path::PathBuf;

    const ME: &str = demo_parse::sample::PLAYER_STR;

    fn sample(rel: &str) -> Option<PathBuf> {
        let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_default();
        let p = PathBuf::from(root).join(rel);
        p.exists().then_some(p)
    }

    /// Store → query round trip on a real demo, plus failure / backfill bookkeeping.
    #[test]
    fn store_and_query_sample() {
        let Some(path) = sample("Official/match730_003831848039841005782_0410013497_154_competitive.dem") else {
            eprintln!("skip: sample demo not found");
            return;
        };
        let dir = std::env::temp_dir().join(format!("cs-analyst-aim-test-{}", uuid::Uuid::new_v4()));
        let db = Database::open(dir.clone()).unwrap();
        db.conn
            .execute(
                "INSERT INTO matches (id, platform, match_id, status, file_path, analysis_status, played_at)
                 VALUES ('m1', 'imported', 'm1', 'completed', ?, 'done', '2026-11-01 20:00:00')",
                params![path.to_string_lossy()],
            )
            .unwrap();
        assert_eq!(repo::list_matches_needing_aim(&db).unwrap(), vec!["m1".to_string()]);
        let db = Arc::new(Mutex::new(db));

        analyze_match(&db, "m1", &path, None);
        analyze_match(&db, "m1", &path, None); // re-run replaces, does not duplicate
        let db = db.lock();
        assert!(repo::list_matches_needing_aim(&db).unwrap().is_empty());
        let count = |t: &str| -> i64 { db.conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap() };
        assert_eq!(count("sprays"), 99);
        assert_eq!(count("aim_engagements"), 218);
        let (status, reliable): (String, bool) =
            db.conn.query_row("SELECT status, visibility_reliable FROM match_aim WHERE match_id='m1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((status.as_str(), reliable), ("done", true));

        let me: u64 = ME.parse().unwrap();
        let stats = commands::aim_stats(&db, me, &Default::default()).unwrap();
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["steamid"], ME);
        assert_eq!(json["matches"], 1);
        assert_eq!(json["reliableMatches"], 1);
        assert_eq!(json["overall"]["shots"], 248);
        assert!((json["overall"]["ttd"]["median"].as_f64().unwrap() - 195.3125).abs() < 1e-3);
        assert_eq!(json["overall"]["ttdHistogram"]["counts"].as_array().unwrap().len(), commands::TTD_BINS);
        assert_eq!(json["trend"].as_array().unwrap().len(), 1);
        let weapon_shots: u32 = stats.weapons.iter().map(|w| w.rates.shots).sum();
        assert_eq!(weapon_shots, stats.overall.rates.shots);
        let ak = stats.weapons.iter().find(|w| w.weapon == "ak47").expect("ak47 row");
        assert!(ak.sprays > 0);
        eprintln!("{}", serde_json::to_string_pretty(&json["overall"]).unwrap());

        // Weapon filter narrows the overall block.
        let f = commands::AimFilters { weapon: Some("ak47".into()), ..Default::default() };
        let ak_stats = commands::aim_stats(&db, me, &f).unwrap();
        assert_eq!(ak_stats.overall.rates.shots, ak.rates.shots);
        assert_eq!(ak_stats.overall.sprays, ak.sprays);
        // Filters that exclude the match.
        let f = commands::AimFilters { maps: Some(vec!["de_dust2".into()]), ..Default::default() };
        assert_eq!(commands::aim_stats(&db, me, &f).unwrap().matches, 0);

        let res = commands::sprays(&db, Some("m1"), me, Some("ak47")).unwrap();
        let pinned = res.sprays.iter().find(|s| s.start_tick == 15230).expect("pinned spray");
        assert_eq!(pinned.shots.len(), 8);
        assert!((pinned.shots[6].punch_pitch + 3.82).abs() < 0.006);
        assert!((pinned.shots[6].err_v - 2.954).abs() < 0.01);
        assert_eq!(res.patterns.len(), 1);
        assert_eq!(res.patterns[0].sprays as usize, res.sprays.len());
        let all = commands::sprays(&db, None, me, None).unwrap();
        assert!(all.sprays.len() >= res.sprays.len() && !all.truncated);
        assert!(commands::sprays(&db, None, me, Some("awp")).is_err());

        // A failure clears the rows and is not retried automatically.
        repo::mark_aim_failed(&db, "m1", "boom").unwrap();
        assert_eq!(count("sprays") + count("spray_shots") + count("aim_engagements") + count("player_match_aim"), 0);
        assert!(repo::list_matches_needing_aim(&db).unwrap().is_empty());
        // Deleting the match cascades.
        db.conn.execute("DELETE FROM matches WHERE id='m1'", []).unwrap();
        assert_eq!(count("match_aim"), 0);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
