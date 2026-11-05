//! Print a parsed demo summary: `cargo run -p demo-parse --release --example summary -- <demo.dem>`
fn main() {
    let path = std::env::args().nth(1).expect("usage: summary <demo.dem>");
    let d = demo_parse::parse_demo_file(std::path::Path::new(&path)).expect("parse");
    println!("map={:?} server={:?} last_tick={}", d.header.map_name, d.header.server_name, d.header.last_tick);
    for p in &d.players {
        let k = d.kills.iter().filter(|k| k.attacker == Some(p.steamid) && k.victim_team != k.attacker_team).count();
        let dth = d.kills.iter().filter(|k| k.victim == Some(p.steamid)).count();
        println!("  {:>17} {:<20} start={:?} end={:?} rank={:?}/{:?} K={} D={}", p.steamid, p.name, p.team_start, p.team_end, p.rank, p.rank_type, k, dth);
    }
    for r in &d.rounds {
        let kills = d.kills.iter().filter(|k| k.round == r.number).count();
        println!("  R{:>2} start={} freeze={:?} end={} winner={:?} reason={:?} kills={}", r.number, r.start_tick, r.freeze_end_tick, r.end_tick, r.winner, r.reason, kills);
    }
}
