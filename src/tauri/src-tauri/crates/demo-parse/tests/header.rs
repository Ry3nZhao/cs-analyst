//! `read_demo_file_meta` 样例回归：数值来自旧 `dem-parser.ts` 的输出（迁移时逐字段对比过，
//! 见 `examples/header_compare.rs`）。样例不存在时跳过。

use demo_parse::read_demo_file_meta;
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

#[test]
fn official_demo_header() {
    let Some(p) = sample("Official/match730_003831848039841005782_0410013497_154_competitive.dem") else { return };
    let m = read_demo_file_meta(&p).unwrap();
    assert!(m.is_valid);
    assert_eq!(m.detected_platform, "valve");
    assert_eq!(m.sha256, "3b2c8b9a5ace83ae8ccb4e8a2246744c2cc6229f0c93d771e25b58ee4c30a2a0");
    assert_eq!(m.map_name.as_deref(), Some("de_ancient"));
    assert_eq!(m.game_mode.as_deref(), Some("competitive"));
    assert_eq!(m.game_type, Some(730));
    assert_eq!(m.match_id, None); // 没有 .dem.info，matchId 来自发现阶段
    assert_eq!(m.duration_seconds, Some(1693));
    assert_eq!(m.playback_time, Some(1692.578125));
    assert_eq!(m.playback_ticks, Some(108325));
    assert_eq!(m.playback_frames, Some(108311));
    assert_eq!(m.network_protocol, Some(14171));
    assert_eq!(m.demo_version_name.as_deref(), Some("valve_demo_2"));
    assert_eq!(m.demo_version_guid.as_deref(), Some("8e9d71ab-04a1-4c01-bb61-acfede27c046"));
    assert_eq!(m.allow_clientside_entities, Some(true));
    assert_eq!(m.dem_game_info, None);
}

#[test]
fn platform_demo_header() {
    let Some(p) = sample("Platforms/g161-n-20260708151346440195771.dem") else { return };
    let m = read_demo_file_meta(&p).unwrap();
    assert!(m.is_valid);
    assert_eq!(m.detected_platform, "imported");
    assert_eq!(m.map_name.as_deref(), Some("de_dust2"));
    assert_eq!(m.game_mode, None);
    assert_eq!(m.game_type, None);
    assert_eq!(m.sha256.len(), 64);
}

#[test]
fn not_a_demo() {
    let dir = std::env::temp_dir().join(format!("demo-parse-header-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("bad.dem");
    std::fs::write(&p, b"not a demo").unwrap();
    let m = read_demo_file_meta(&p).unwrap();
    assert!(!m.is_valid);
    assert_eq!(m.sha256, "");
    let _ = std::fs::remove_dir_all(&dir);
}
