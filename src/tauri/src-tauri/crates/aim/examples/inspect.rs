//! Manual spot check of one contact:
//! `cargo run -p aim --release --example inspect -- <demo.dem> <attacker steamid> <damage tick>`
//! Prints the attacker's view vs. the direction to the victim's head for every tick of the
//! window, visibility flags and shots, next to the computed sample.
use aim::geom::{angles_to, norm_pitch, offset};
use demo_parse::engagement::AnchorKind;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (path, sid, tick): (&str, u64, i32) = (&a[1], a[2].parse().unwrap(), a[3].parse().unwrap());
    let demo = demo_parse::parse_demo_file(Path::new(path)).unwrap();
    let eng = demo_parse::parse_engagements_file(Path::new(path), Default::default()).unwrap();
    let r = aim::analyze(&demo, &eng);
    let w = eng
        .windows
        .iter()
        .find(|w| w.kind == AnchorKind::Damage && w.attacker == sid && w.tick == tick)
        .expect("no damage window at that tick");
    let name = |s: u64| demo.players.iter().find(|p| p.steamid == s).map(|p| p.name.clone()).unwrap_or_default();
    println!("R{} {} → {} with {} at {} ({} {:?})", w.round, name(w.attacker), name(w.victim), w.weapon, w.tick, w.dmg_health, w.hitgroup);
    for s in r.engagements.iter().filter(|e| e.damage_tick == tick && (e.observer == sid || e.target == sid)) {
        println!("sample: {s:?}");
    }
    let shots: Vec<i32> = demo.shots.iter().filter(|s| s.shooter == sid).map(|s| s.tick).collect();
    for (i, t) in w.ticks().enumerate() {
        let (Some(me), Some(them)) = (w.attacker_track[i], w.victim_track[i]) else { continue };
        if (w.tick - t) > 64 && !me.sees_other {
            continue;
        }
        let (rp, ry) = angles_to(me.eye(), them.eye());
        let (v, h, tot) = offset(norm_pitch(me.pitch), me.yaw, rp, ry);
        println!(
            "  t={t} A_sees={} V_sees={} view=({:.2},{:.2}) head=({:.2},{:.2}) off v={:+.2} h={:+.2} |{:.2}| speed={:.0}{}",
            me.sees_other as u8,
            them.sees_other as u8,
            norm_pitch(me.pitch),
            me.yaw,
            rp,
            ry,
            v,
            h,
            tot,
            me.velocity.x.hypot(me.velocity.y),
            if shots.contains(&t) { "  SHOT" } else { "" }
        );
    }
}
