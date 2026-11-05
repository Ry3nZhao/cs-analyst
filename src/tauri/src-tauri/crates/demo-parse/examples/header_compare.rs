//! 对比旧 `dem-parser.ts`（下载模块）与 `read_demo_file_meta` 的结果，按 `matches` 表里的字段口径。
//!
//! 输入是旧实现导出的 JSON：`[{ "file": "...", "header": <analyzeDem 结果> }]`。
//! 用法：`cargo run -p demo-parse --release --example header_compare -- baseline.json [more.json]`

use demo_parse::read_demo_file_meta;
use serde_json::Value;
use std::path::Path;

/// `matches` 表里的元数据列（空串视为 NULL，与 orchestrator 一致）。
#[derive(Debug, PartialEq)]
struct Row {
    match_id: Option<String>,
    map_name: Option<String>,
    duration_seconds: Option<i64>,
    game_mode: Option<String>,
    game_type: Option<i64>,
    played_at: Option<String>,
    checksum: Option<String>,
    is_valid: bool,
    detected_platform: Option<String>,
    demo_file_stamp: Option<String>,
    network_protocol: Option<i64>,
    server_name: Option<String>,
    client_name: Option<String>,
    game_directory: Option<String>,
    fullpackets_version: Option<i64>,
    allow_clientside_entities: Option<bool>,
    demo_header_field_9: Option<i64>,
    demo_header_field_10: Option<String>,
    demo_version_name: Option<String>,
    demo_version_guid: Option<String>,
    playback_time: Option<f64>,
    playback_ticks: Option<i64>,
    playback_frames: Option<i64>,
    dem_game_info: Option<String>,
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).filter(|x| !x.is_empty()).map(String::from)
}
fn i(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(|x| x.as_i64())
}

/// 旧路径：orchestrator 原来的 `parse_dem_metadata`。
fn old_row(h: &Value) -> Row {
    Row {
        match_id: s(h, "matchId"),
        map_name: s(h, "mapName"),
        duration_seconds: i(h, "durationSeconds"),
        game_mode: s(h, "gameMode"),
        game_type: i(h, "gameType"),
        played_at: s(h, "playedAt"),
        checksum: s(h, "sha256"),
        is_valid: h.get("isValid").and_then(|v| v.as_bool()).unwrap_or(false),
        detected_platform: s(h, "detectedPlatform"),
        demo_file_stamp: s(h, "demoFileStamp"),
        network_protocol: i(h, "networkProtocol"),
        server_name: s(h, "serverName"),
        client_name: s(h, "clientName"),
        game_directory: s(h, "gameDirectory"),
        fullpackets_version: i(h, "fullpacketsVersion"),
        allow_clientside_entities: h.get("allowClientsideEntities").and_then(|x| x.as_bool()),
        demo_header_field_9: i(h, "demoHeaderField9"),
        demo_header_field_10: s(h, "demoHeaderField10"),
        demo_version_name: s(h, "demoVersionName"),
        demo_version_guid: s(h, "demoVersionGuid"),
        playback_time: h.get("playbackTime").and_then(|x| x.as_f64()),
        playback_ticks: i(h, "playbackTicks"),
        playback_frames: i(h, "playbackFrames"),
        dem_game_info: h.get("demGameInfo").cloned().filter(|v| !v.is_null()).map(|v| v.to_string()),
    }
}

fn ne(v: Option<String>) -> Option<String> {
    v.filter(|x| !x.is_empty())
}

/// 新路径：orchestrator 现在的 `metadata_from_header`。
fn new_row(path: &Path) -> Row {
    let m = read_demo_file_meta(path).expect("read header");
    Row {
        match_id: ne(m.match_id),
        map_name: ne(m.map_name),
        duration_seconds: m.duration_seconds,
        game_mode: ne(m.game_mode),
        game_type: m.game_type,
        played_at: ne(m.played_at),
        checksum: ne(Some(m.sha256)),
        is_valid: m.is_valid,
        detected_platform: ne(Some(m.detected_platform)),
        demo_file_stamp: ne(m.demo_file_stamp),
        network_protocol: m.network_protocol,
        server_name: ne(m.server_name),
        client_name: ne(m.client_name),
        game_directory: ne(m.game_directory),
        fullpackets_version: m.fullpackets_version,
        allow_clientside_entities: m.allow_clientside_entities,
        demo_header_field_9: m.demo_header_field_9,
        demo_header_field_10: ne(m.demo_header_field_10),
        demo_version_name: ne(m.demo_version_name),
        demo_version_guid: ne(m.demo_version_guid),
        playback_time: m.playback_time,
        playback_ticks: m.playback_ticks,
        playback_frames: m.playback_frames,
        dem_game_info: m.dem_game_info.map(|v| v.to_string()),
    }
}

fn main() {
    let mut total = 0;
    let mut diffs = 0;
    for input in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&input).expect("read baseline");
        let entries: Vec<Value> = serde_json::from_str(&text).expect("baseline json");
        for e in entries {
            let file = e["file"].as_str().unwrap();
            let t = std::time::Instant::now();
            let new = new_row(Path::new(file));
            let ms = t.elapsed().as_millis();
            let old = old_row(&e["header"]);
            total += 1;
            if old == new {
                println!("OK   {ms:>5}ms {file}");
            } else {
                diffs += 1;
                println!("DIFF {ms:>5}ms {file}\n  old: {old:?}\n  new: {new:?}");
            }
        }
    }
    println!("{total} files, {diffs} differences");
    if diffs > 0 {
        std::process::exit(1);
    }
}
