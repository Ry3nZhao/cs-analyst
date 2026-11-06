//! Shared helpers for sample-demo tests.
#![allow(dead_code)]

use ahash::AHashMap;
use parser::first_pass::parser_settings::{rm_user_friendly_names, ParserInputs};
use parser::parse_demo::{Parser, ParsingMode};
use parser::second_pass::parser_settings::create_huffman_lookup_table;
use parser::second_pass::variants::{PropColumn, VarVec};
use std::collections::HashMap;
use std::path::PathBuf;

pub const OFFICIAL: &str = "Official/match730_003831848039841005782_0410013497_154_competitive.dem";
pub const PLATFORM: &str = "Platforms/g161-n-20260709134236148390830.dem";
pub const PRO: &str = "Profs/2026major/stage1/nrg-vs-big-m1-dust2-p1.dem";

pub fn sample(rel: &str) -> Option<PathBuf> {
    let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_default();
    let p = PathBuf::from(root).join(rel);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("skip: {} not found", p.display());
        None
    }
}

/// In-game scoreboard values (CCSPlayerController_ActionTrackingServices) at `tick`.
#[derive(Debug, Default, Clone)]
pub struct Board {
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub headshot_kills: i64,
    pub damage: i64,
    pub utility_damage: i64,
    pub enemies_flashed: i64,
    pub mvps: i64,
}

const PROPS: &[&str] = &[
    "kills_total",
    "deaths_total",
    "assists_total",
    "headshot_kills_total",
    "damage_total",
    "utility_damage_total",
    "enemies_flashed_total",
    "mvps",
];

fn col_i64(c: &PropColumn, i: usize) -> Option<i64> {
    match c.data.as_ref()? {
        VarVec::I32(v) => v.get(i).copied().flatten().map(|x| x as i64),
        VarVec::U32(v) => v.get(i).copied().flatten().map(|x| x as i64),
        VarVec::U64(v) => v.get(i).copied().flatten().map(|x| x as i64),
        VarVec::F32(v) => v.get(i).copied().flatten().map(|x| x as i64),
        _ => None,
    }
}

/// Scoreboard per steamid at the given tick.
pub fn scoreboard(path: &std::path::Path, tick: i32) -> HashMap<u64, Board> {
    scoreboard_at(path, vec![tick]).into_iter().map(|((_, sid), b)| (sid, b)).collect()
}

/// Scoreboard per (tick, steamid) at the given ticks.
pub fn scoreboard_at(path: &std::path::Path, ticks: Vec<i32>) -> HashMap<(i32, u64), Board> {
    let bytes = std::fs::read(path).unwrap();
    let huf = create_huffman_lookup_table();
    let og: Vec<String> = PROPS.iter().map(|s| s.to_string()).collect();
    let real = rm_user_friendly_names(&og).unwrap();
    let mut map = AHashMap::default();
    for (r, o) in real.iter().zip(&og) {
        map.insert(r.clone(), o.clone());
    }
    let inputs = ParserInputs {
        real_name_to_og_name: map,
        wanted_players: vec![],
        wanted_player_props: real,
        wanted_other_props: vec![],
        wanted_events: vec![],
        wanted_prop_states: AHashMap::default(),
        parse_ents: true,
        wanted_ticks: ticks,
        parse_projectiles: false,
        parse_grenades: false,
        only_header: true,
        list_props: false,
        only_convars: false,
        huffman_lookup_table: &huf,
        order_by_steamid: false,
        fallback_bytes: None,
    };
    let out = Parser::new(inputs, ParsingMode::Normal).parse_demo(&bytes).unwrap();
    let df = &out.df;
    let ids: HashMap<String, u32> =
        out.prop_controller.prop_infos.iter().map(|p| (p.prop_friendly_name.clone(), p.id)).collect();
    let sid_col = &df[&parser::first_pass::prop_controller::STEAMID_ID];
    let n = match sid_col.data.as_ref() {
        Some(VarVec::U64(v)) => v.len(),
        _ => 0,
    };
    let mut res = HashMap::new();
    for i in 0..n {
        let Some(sid) = col_i64(sid_col, i) else { continue };
        let Some(tick) = col_i64(&df[&parser::first_pass::prop_controller::TICK_ID], i) else { continue };
        let g = |name: &str| ids.get(name).and_then(|id| df.get(id)).and_then(|c| col_i64(c, i)).unwrap_or(-1);
        res.insert(
            (tick as i32, sid as u64),
            Board {
                kills: g("kills_total"),
                deaths: g("deaths_total"),
                assists: g("assists_total"),
                headshot_kills: g("headshot_kills_total"),
                damage: g("damage_total"),
                utility_damage: g("utility_damage_total"),
                enemies_flashed: g("enemies_flashed_total"),
                mvps: g("mvps"),
            },
        );
    }
    res
}

