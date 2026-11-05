//! Print the computed scoreboard: `cargo run -p stats --release --example explore -- <demo.dem>...`
fn main() {
    for path in std::env::args().skip(1) {
        let d = demo_parse::parse_demo_file(std::path::Path::new(&path)).expect("parse");
        let s = stats::compute((&d).into());
        println!("== {path}\nscore A {} : {} B  winner={:?} blind={}", s.score_a, s.score_b, s.winner, s.blind_data_available);
        println!("{:>17} {:<18} T  K  D  A  ADR   KAST  HS%  RAT  2k 3k 4k 5k MVP OK/OD(T) OK/OD(CT) TrK/Opp/Att TrD/Opp  clutch(att/win 1..5)  FL EF  EBs  TF HEd FIRE unused", "steamid", "name");
        for p in &s.players {
            let dd = &p.derived;
            println!("{:>17} {:<18} {} {:>2} {:>2} {:>2} {:>5.1} {:>5.1} {:>4.0} {:.2} {:>2} {:>2} {:>2} {:>2} {:>3} {}/{} {}/{} {}/{}/{} {}/{} {:?}/{:?} {:>2} {:?} {:?} {:?} {:>3} {:>3} {:>5}",
                p.steamid, p.name.chars().take(18).collect::<String>(), p.team.unwrap().as_str(), p.kills, p.deaths, p.assists, dd.adr, dd.kast, dd.hs_pct, dd.rating,
                p.multi_kills[2], p.multi_kills[3], p.multi_kills[4], p.multi_kills[5], p.mvps,
                p.opening_kills_t, p.opening_deaths_t, p.opening_kills_ct, p.opening_deaths_ct,
                p.trade_kills, p.trade_kill_opportunities, p.trade_kill_attempts, p.traded_deaths, p.traded_death_opportunities,
                &p.clutch_attempts[1..], &p.clutch_wins[1..], p.flashes_thrown, p.enemies_flashed, p.enemy_blind_seconds.map(|v| (v*10.0).round()/10.0), p.teammates_flashed, p.he_damage, p.fire_damage, p.unused_utility_value);
        }
        for t in s.team_rounds.iter() { print!("R{}{}:{}{}:{}:{} ", t.round, t.team.as_str(), t.side, if t.won {"W"} else {"L"}, t.buy_type.as_str(), t.equip_value); if t.team == stats::Team::B { println!(" score {}", t.score); } }
        for r in &s.rounds { println!("R{} sideA={} {}-{} {:?} mvp={:?} kills={} opening={:?}", r.number, r.side_a, r.score_a, r.score_b, r.reason, r.mvp, r.kills.len(), r.kills.iter().find(|k| k.opening).map(|k| (k.attacker, k.victim))); }
    }
}
