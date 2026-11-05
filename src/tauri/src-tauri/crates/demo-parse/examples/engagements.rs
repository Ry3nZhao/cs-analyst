//! Engagement-window summary: `cargo run -p demo-parse --release --example engagements -- <demo.dem>`
use demo_parse::engagement::AnchorKind;
use std::collections::BTreeMap;

fn main() {
    let path = std::env::args().nth(1).expect("usage: engagements <demo.dem>");
    let t = std::time::Instant::now();
    let e = demo_parse::parse_engagements_file(std::path::Path::new(&path), Default::default()).expect("parse");
    let d = demo_parse::parse_demo_file(std::path::Path::new(&path)).expect("parse");
    println!("parsed in {:.2}s: {} windows, {} shot sights", t.elapsed().as_secs_f64(), e.windows.len(), e.shot_sights.len());

    let kills = e.windows.iter().filter(|w| w.kind == AnchorKind::Kill).count();
    let seen_at_anchor = e
        .windows
        .iter()
        .filter(|w| w.attacker_track.iter().rev().take(8).any(|s| s.is_some_and(|s| s.sees_other)))
        .count();
    let missing = e.windows.iter().flat_map(|w| w.attacker_track.iter()).filter(|s| s.is_none()).count();
    let total = e.windows.iter().map(|w| w.attacker_track.len()).sum::<usize>();
    println!("kills={kills} damages={} attacker sees victim within 8 ticks of anchor: {seen_at_anchor}", e.windows.len() - kills);
    println!("missing attacker states: {missing}/{total}");

    let mut hg: BTreeMap<String, usize> = BTreeMap::new();
    for w in &e.windows {
        *hg.entry(format!("{:?}", w.hitgroup)).or_default() += 1;
    }
    println!("hitgroups: {hg:?}");

    // Damage events with a weapon_fire by the attacker on the same tick.
    let shot_ticks: std::collections::HashSet<(i32, u64)> = d.shots.iter().map(|s| (s.tick, s.shooter)).collect();
    let gun_dmg: Vec<_> = d
        .damages
        .iter()
        .filter(|x| !matches!(x.weapon.as_str(), "hegrenade" | "inferno" | "molotov" | "incgrenade" | "knife" | "world" | ""))
        .collect();
    for dt in 0..3 {
        let same = gun_dmg.iter().filter(|x| x.attacker.is_some_and(|a| shot_ticks.contains(&(x.tick - dt, a)))).count();
        println!("gun damages with shot at tick-{dt}: {same}/{}", gun_dmg.len());
    }
    let vis = e.shot_sights.iter().filter(|s| !s.visible_enemies.is_empty()).count();
    println!("shots with visible enemy: {vis}/{}", e.shot_sights.len());
    let mut weapons: BTreeMap<&str, usize> = BTreeMap::new();
    for s in &d.shots {
        *weapons.entry(s.weapon.as_str()).or_default() += 1;
    }
    println!("shot weapons: {weapons:?}");
    if std::env::var("DUMP").is_ok() { dump_vis(&e, 12); }
}

#[allow(dead_code)]
fn dump_vis(e: &demo_parse::Engagements, n: usize) {
    for w in e.windows.iter().filter(|w| w.kind == AnchorKind::Kill).take(n) {
        let a: String = w.attacker_track.iter().map(|s| match s { Some(s) if s.sees_other => '#', Some(_) => '.', None => ' ' }).collect();
        let v: String = w.victim_track.iter().map(|s| match s { Some(s) if s.sees_other => '#', Some(_) => '.', None => ' ' }).collect();
        println!("R{} t={} {} {}\n  A {}\n  V {}", w.round, w.tick, w.weapon, w.headshot, a, v);
    }
}
