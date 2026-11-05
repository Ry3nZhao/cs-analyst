//! Aim report for one demo: `cargo run -p aim --release --example report -- <demo.dem> [steamid]`
use aim::{ratio, ALL_WEAPONS};
use std::path::Path;

fn f(x: Option<f32>) -> String {
    x.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into())
}
fn pct(n: u32, d: u32) -> String {
    ratio(n, d).map(|r| format!("{:.0}%", r * 100.0)).unwrap_or_else(|| "-".into())
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: report <demo.dem> [steamid]");
    let focus: Option<u64> = std::env::args().nth(2).and_then(|s| s.parse().ok());
    let t = std::time::Instant::now();
    let demo = demo_parse::parse_demo_file(Path::new(&path)).unwrap();
    let eng = demo_parse::parse_engagements_file(Path::new(&path), Default::default()).unwrap();
    let r = aim::analyze(&demo, &eng);
    println!(
        "{:.2}s visibility={:?} reliable={} samples={} sprays={}",
        t.elapsed().as_secs_f64(),
        r.visibility_quality,
        r.visibility_reliable,
        r.engagements.len(),
        r.sprays.len()
    );
    let all = |f: fn(&aim::EngagementSample) -> Option<f32>| aim::Dist::of(r.engagements.iter().filter_map(f));
    println!("ALL ttd={:?}", all(|x| x.ttd_ms));
    println!("ALL reaction={:?}", all(|x| x.reaction_ms));
    println!("ALL cp={:?}", all(|x| x.cp_deg));
    println!("ALL cp hist(1°)={:?}", aim::histogram(r.engagements.iter().filter_map(|x| x.cp_deg), 1.0, 40));
    println!("ALL ttd hist(100ms)={:?}", aim::histogram(r.engagements.iter().filter_map(|x| x.ttd_ms), 100.0, 31));
    let sc: Vec<f32> = r.sprays.iter().filter_map(|s| s.control).collect();
    println!("ALL spray control={:?} err={:?}", aim::Dist::of(sc), aim::Dist::of(r.sprays.iter().map(|s| s.mean_err)));
    println!(
        "{:<20} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>5} {:>5} {:>5} {:>5} {:>4} {:>6}",
        "player", "ttd", "react", "cp", "cpV", "cpH", "cpVs", "acc", "vacc", "1st", "cs", "spr", "ctrl"
    );
    for p in r.players.iter().filter(|p| p.weapon == ALL_WEAPONS) {
        let name = demo.players.iter().find(|x| x.steamid == p.steamid).map(|x| x.name.as_str()).unwrap_or("");
        let c = &p.counters;
        println!(
            "{:<20} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>5} {:>5} {:>5} {:>5} {:>4} {:>6}",
            name.chars().take(20).collect::<String>(),
            f(p.ttd.median),
            f(p.reaction.median),
            f(p.cp.median),
            f(p.cp_v_abs.median),
            f(p.cp_h_abs.median),
            f(p.cp_v.mean),
            pct(c.hits, c.shots),
            pct(c.vis_hits.unwrap_or(0), c.vis_shots.unwrap_or(0)),
            pct(c.first_hits.unwrap_or(0), c.first_shots.unwrap_or(0)),
            pct(c.cs_ok, c.cs_shots),
            p.sprays,
            f(p.spray_control),
        );
    }
    for w in aim::weapons::SPRAY_WEAPONS {
        let sp: Vec<_> = r.sprays.iter().filter(|s| s.weapon == *w).collect();
        if sp.is_empty() {
            continue;
        }
        let pat = aim::aggregate(w, sp.iter().copied());
        let curve: Vec<String> = pat.bullets.iter().take(12).map(|b| format!("{}:{:.1}/{:.1}/{}", b.idx, b.err, b.err_median, b.n)).collect();
        println!("{w}: sprays={} ctrl={} break={:?} {}", pat.sprays, f(pat.control), pat.break_shot, curve.join(" "));
    }
    if let Some(sid) = focus {
        for s in r.sprays.iter().filter(|s| s.steamid == sid && s.weapon == "ak47").take(3) {
            println!("spray R{} t={}..{} hits={} target={:?} ctrl={} break={:?}", s.round, s.start_tick, s.end_tick, s.hits, s.target, f(s.control), s.break_shot);
            for sh in &s.shots {
                println!(
                    "  #{:<2} t={} punch=({:.2},{:.2}) recoil=({:.2},{:.2}) aim=({:.2},{:.2}) err=({:.2},{:.2}) hit={} tgt=({},{})",
                    sh.idx, sh.tick, sh.punch_pitch, sh.punch_yaw, sh.recoil_v, sh.recoil_h, sh.aim_v, sh.aim_h, sh.err_v, sh.err_h, sh.hit, f(sh.tgt_v), f(sh.tgt_h)
                );
            }
        }
        for e in r.engagements.iter().filter(|e| e.observer == sid).take(8) {
            println!("eng {:?}", e);
        }
    }
}
