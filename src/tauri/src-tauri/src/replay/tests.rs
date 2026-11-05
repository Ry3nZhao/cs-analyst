//! Sample regression: ingest a real demo, then check the replay payload and heatmap queries.

use super::{commands, heatmap, pack};
use crate::database::Database;
use parking_lot::Mutex;
use std::path::PathBuf;

const ME: &str = "76561198000000042";

fn sample(rel: &str) -> Option<PathBuf> {
    let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_else(|_| "<sample-dir>".into());
    let p = PathBuf::from(root).join(rel);
    p.exists().then_some(p)
}

fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}

#[test]
fn replay_and_heatmap_on_sample() {
    let Some(path) = sample("Platforms/g161-n-20260709134236148390830.dem") else {
        eprintln!("skip: sample demo not found");
        return;
    };
    let demo = demo_parse::parse_demo_file(&path).unwrap();
    let dir = std::env::temp_dir().join(format!("cs-analyst-replay-test-{}", uuid::Uuid::new_v4()));
    let db = Database::open(dir.clone()).unwrap();
    db.conn
        .execute(
            "INSERT INTO matches (id, platform, match_id, status, file_path, map_name, played_at)
             VALUES ('m1', '5eplay', 'm1', 'completed', 'demos/x.dem', 'de_dust2', '2026-07-09T13:42:36Z')",
            [],
        )
        .unwrap();
    let rel = "analysis/m1.replay.zst";
    demo.replay.write_to(&dir.join(rel)).unwrap();
    crate::database::analysis::replace_match_analysis(&db, "m1", &demo, Some(rel), 1).unwrap();
    crate::stats::on_match_ingested(&db, "m1", &demo);
    let db = Mutex::new(db);

    // ---- get_replay payload
    let buf = commands::build_replay(&db, "m1").unwrap();
    assert_eq!(&buf[..4], pack::MAGIC);
    assert_eq!(u32_at(&buf, 4), pack::PACK_VERSION);
    let hl = u32_at(&buf, 8) as usize;
    assert_eq!(hl % 4, 0);
    let header: serde_json::Value = serde_json::from_slice(&buf[12..12 + hl]).unwrap();
    let body = &buf[12 + hl..];
    let nf = header["nFrames"].as_u64().unwrap() as usize;
    let np = header["nPlayers"].as_u64().unwrap() as usize;
    assert_eq!(nf, demo.replay.n_frames());
    assert_eq!(np, 10);
    assert_eq!(header["tickStep"], 4);
    assert_eq!(header["mapName"], "de_dust2");
    assert_eq!(header["rounds"].as_array().unwrap().len(), 15);
    assert_eq!(header["kills"].as_array().unwrap().len(), demo.kills.len());
    assert_eq!(header["grenades"].as_array().unwrap().len(), demo.grenades.len());
    let last = &header["rounds"][14];
    assert!(last["scoreT"].is_u64() && last["scoreCt"].is_u64(), "score from team_round_stats");
    let players = header["players"].as_array().unwrap();
    assert!(players.iter().any(|p| p["steamid"] == ME && p["name"] == "PlayerOne"));
    let cols = &header["columns"];
    assert_eq!(cols["bodyLen"].as_u64().unwrap() as usize, body.len());
    // Spot-check a value in every column against the decoded file.
    let f = nf / 2;
    let i = f * np + 3;
    let off = |k: &str| cols[k].as_u64().unwrap() as usize;
    let f32_at = |k: &str, i: usize| f32::from_le_bytes(body[off(k) + i * 4..off(k) + i * 4 + 4].try_into().unwrap());
    assert_eq!(i32::from_le_bytes(body[off("ticks") + f * 4..off("ticks") + f * 4 + 4].try_into().unwrap()), demo.replay.ticks[f]);
    assert_eq!(f32_at("x", i), demo.replay.x[i]);
    assert_eq!(f32_at("y", i), demo.replay.y[i]);
    assert_eq!(f32_at("yaw", i), demo.replay.yaw[i]);
    assert_eq!(body[off("health") + i], demo.replay.health[i]);
    assert_eq!(body[off("flags") + i], demo.replay.flags[i]);
    assert_eq!(
        u16::from_le_bytes([body[off("spottedBy") + i * 2], body[off("spottedBy") + i * 2 + 1]]),
        demo.replay.spotted_by[i]
    );
    eprintln!("replay payload: {} frames, {} bytes", nf, buf.len());
    assert!(commands::build_replay(&db, "nope").is_err());

    // ---- get_heatmap_points
    let q = |f: heatmap::HeatmapFilters| heatmap::query(&db, &f).unwrap();
    let all = q(Default::default());
    let kills = all.kind.iter().filter(|k| **k == heatmap::KIND_KILL).count();
    let deaths = all.kind.iter().filter(|k| **k == heatmap::KIND_DEATH).count();
    assert_eq!(kills, deaths, "each counted kill has one victim point");
    assert_eq!(all.maps.len(), 1);
    assert_eq!(all.maps[0].map_name.as_deref(), Some("de_dust2"));
    assert_eq!(all.matches.len(), 1);
    eprintln!("heatmap: {kills} kills");

    let me = |kinds: &[&str]| heatmap::HeatmapFilters {
        steamids: Some(vec![ME.into()]),
        kinds: Some(kinds.iter().map(|s| s.to_string()).collect()),
        ..Default::default()
    };
    let my_kills = q(me(&["kill"]));
    let my_deaths = q(me(&["death"]));
    eprintln!("me: {} kills, {} deaths", my_kills.kind.len(), my_deaths.kind.len());
    assert_eq!(my_kills.kind.len(), 15, "PlayerOne kills (no suicides / team kills)");
    assert_eq!(my_deaths.kind.len(), 6);
    assert_eq!(kills, 98, "all non-suicide, non-team kills in the match");
    // Side filter partitions.
    let t = q(heatmap::HeatmapFilters { side: Some(2), ..me(&["kill"]) });
    let ct = q(heatmap::HeatmapFilters { side: Some(3), ..me(&["kill"]) });
    assert_eq!(t.kind.len() + ct.kind.len(), my_kills.kind.len());
    assert!(t.side.iter().all(|s| *s == 2));
    // Buy types partition too.
    let by_buy: usize = ["pistol", "eco", "force", "full"]
        .iter()
        .map(|b| q(heatmap::HeatmapFilters { buy_types: Some(vec![b.to_string()]), ..me(&["kill"]) }).kind.len())
        .sum();
    assert_eq!(by_buy, my_kills.kind.len());
    let pistol = q(heatmap::HeatmapFilters { rounds: Some(vec![1]), ..Default::default() });
    assert!(pistol.round.iter().all(|r| *r == 1));
    assert!(q(heatmap::HeatmapFilters { maps: Some(vec!["de_mirage".into()]), ..Default::default() }).kind.is_empty());

    // Standing positions sampled from the replay file.
    let pres = q(me(&["presence"]));
    assert!(pres.kind.len() > 500, "about one point per second alive: {}", pres.kind.len());
    assert!(pres.kind.iter().all(|k| *k == heatmap::KIND_PRESENCE));
    let capped = q(heatmap::HeatmapFilters { limit: Some(100), ..me(&["presence"]) });
    assert_eq!(capped.kind.len(), 100);
    assert!(capped.truncated);
    assert!(heatmap::query(&db, &heatmap::HeatmapFilters { steamids: Some(vec!["abc".into()]), ..Default::default() }).is_err());

    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}
