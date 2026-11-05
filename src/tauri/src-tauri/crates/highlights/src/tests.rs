//! 合成数据：每条规则的命中与边界（样例 demo 的回归测试在 src/highlights/tests.rs）。

use crate::*;

const T1: u64 = 76561198000000001;
const T2: u64 = 76561198000000002;
const C1: u64 = 76561198000000011;
const C2: u64 = 76561198000000012;

fn round(n: u32, winner: u8) -> RoundIn {
    RoundIn { number: n, start_tick: 0, freeze_end_tick: Some(1000), end_tick: 9000, winner: Some(winner) }
}

fn players(n: u32) -> Vec<PlayerRoundIn> {
    [(T1, TEAM_T), (T2, TEAM_T), (C1, TEAM_CT), (C2, TEAM_CT)]
        .iter()
        .map(|(s, side)| PlayerRoundIn { round: n, steamid: *s, side: *side, ..Default::default() })
        .collect()
}

fn team(s: u64) -> u8 {
    if s == T1 || s == T2 {
        TEAM_T
    } else {
        TEAM_CT
    }
}

fn kill(r: u32, tick: i32, a: u64, v: u64, weapon: &str) -> KillIn {
    KillIn {
        round: r,
        tick,
        attacker: Some(a),
        victim: Some(v),
        weapon: weapon.into(),
        distance: Some(5.0),
        attacker_team: Some(team(a)),
        victim_team: Some(team(v)),
        ..Default::default()
    }
}

fn dmg(r: u32, tick: i32, a: u64, v: u64, h: i32, after: i32) -> DamageIn {
    DamageIn {
        round: r,
        tick,
        attacker: Some(a),
        victim: Some(v),
        weapon: "ak47".into(),
        dmg_health: h,
        health_after: after,
        attacker_team: Some(team(a)),
        victim_team: Some(team(v)),
    }
}

fn bomb(r: u32, tick: i32, p: u64, kind: &str) -> BombIn {
    BombIn { round: r, tick, player: Some(p), kind: kind.into() }
}

fn base(rounds: u32) -> MatchInput {
    MatchInput {
        tick_rate: 64.0,
        rounds: (1..=rounds).map(|n| round(n, TEAM_CT)).collect(),
        players: (1..=rounds).flat_map(players).collect(),
        blind_data: true,
        ..Default::default()
    }
}

fn kinds(out: &[Highlight]) -> Vec<(u32, Kind, u64)> {
    out.iter().map(|h| (h.round, h.kind, h.steamid)).collect()
}

#[test]
fn defuses() {
    let mut m = base(2);
    // 第 1 回合：T 两人都活着，C1 没有击杀就拆掉了 → 忍者拆包
    m.bombs.extend([bomb(1, 3000, T1, "planted"), bomb(1, 5000, C1, "defuse_begin"), bomb(1, 5320, C1, "defused")]);
    // 第 2 回合：C1 下包后杀了 T1，T2 还活着 → 关键拆包
    m.bombs.extend([bomb(2, 3000, T1, "planted"), bomb(2, 5000, C1, "defuse_begin"), bomb(2, 5320, C1, "defused")]);
    m.kills.push(kill(2, 4000, C1, T1, "m4a1"));
    let out = detect(&m);
    let ks = kinds(&out);
    assert!(ks.contains(&(1, Kind::NinjaDefuse, C1)), "{ks:?}");
    assert!(ks.contains(&(2, Kind::KeyDefuse, C1)), "{ks:?}");
    let ninja = out.iter().find(|h| h.kind == Kind::NinjaDefuse).unwrap();
    assert_eq!((ninja.start_tick, ninja.end_tick), (5000, 5320));
    assert_eq!(ninja.detail["tAlive"], 2);
}

#[test]
fn lost_hp_duel_and_third_party() {
    let mut m = base(2);
    for r in 1..=2 {
        // C2 早先被打到 30 血；T2（满血）和 C2 对枪，T2 开了枪但被 C2 打死
        m.damages.push(dmg(r, 2000, T1, C2, 70, 30));
        m.shots.extend((0..3).map(|i| ShotIn { round: r, tick: 4890 + i * 6, shooter: T2, weapon: "ak47".into() }));
        m.damages.push(dmg(r, 4900, C2, T2, 30, 70));
        m.kills.push(kill(r, 5000, C2, T2, "m4a1"));
    }
    // 第 2 回合有第三人（C1）也打了 T2 → 不算 1v1
    m.damages.push(dmg(2, 4950, C1, T2, 20, 50));
    let out = detect(&m);
    let duels: Vec<_> = out.iter().filter(|h| h.kind == Kind::LostHpDuel).collect();
    assert_eq!(duels.len(), 1, "{:?}", kinds(&out));
    assert_eq!((duels[0].round, duels[0].steamid), (1, T2));
    assert_eq!(duels[0].detail["hp"], 100);
    assert_eq!(duels[0].detail["enemyHp"], 30);
}

#[test]
fn whiffed_spray_and_team_flash() {
    let mut m = base(1);
    // T2 AK 连发 7 发全空，0.2 秒后被 5 米外的 C1 打死
    m.shots.extend((0..7).map(|i| ShotIn { round: 1, tick: 3000 + i * 6, shooter: T2, weapon: "weapon_ak47".into() }));
    m.kills.push(kill(1, 3050, C1, T2, "m4a1"));
    // 同样的连发但有一发打中 → 不算
    m.shots.extend((0..7).map(|i| ShotIn { round: 1, tick: 6000 + i * 6, shooter: T1, weapon: "ak47".into() }));
    m.damages.push(dmg(1, 6012, T1, C2, 27, 73));
    m.kills.push(kill(1, 6050, C2, T1, "m4a1"));
    // T1 的闪光：队友 T2 白 3 秒，敌人 C1 只白 0.5 秒
    let b = |v: u64, d: f32| BlindIn {
        round: 1,
        tick: 2000,
        attacker: Some(T1),
        victim: Some(v),
        duration: d,
        attacker_team: Some(TEAM_T),
        victim_team: Some(team(v)),
    };
    m.blinds.extend([b(T2, 3.0), b(C1, 0.5)]);
    let out = detect(&m);
    let ks = kinds(&out);
    assert!(ks.contains(&(1, Kind::WhiffedSpray, T2)), "{ks:?}");
    assert!(!ks.contains(&(1, Kind::WhiffedSpray, T1)), "{ks:?}");
    assert!(ks.contains(&(1, Kind::TeamFlash, T1)), "{ks:?}");
    let w = out.iter().find(|h| h.kind == Kind::WhiffedSpray).unwrap();
    assert_eq!(w.detail["shots"], 7);

    // 没有致盲数据（ESL）时不出闪到队友
    m.blind_data = false;
    assert!(!detect(&m).iter().any(|h| h.kind == Kind::TeamFlash));
    assert!(!available_kinds(&m).contains(&Kind::TeamFlash));
}

#[test]
fn kills_quick_special_team_damage() {
    let mut m = base(1);
    let mut k1 = kill(1, 2000, C1, T1, "awp");
    k1.noscope = true;
    k1.distance = Some(20.0);
    let mut k2 = kill(1, 2030, C1, T2, "m4a1");
    k2.penetrated = true;
    k2.thrusmoke = true;
    m.kills.extend([k1, k2]);
    // 队友伤害：C2 用燃烧瓶烧了 C1 30 血
    let mut d = dmg(1, 1500, C2, C1, 30, 70);
    d.weapon = "inferno".into();
    m.damages.push(d);
    // 比赛结束（最后一回合 end_tick）之后的击杀不算
    m.kills.push(kill(1, 9500, T1, C2, "ak47"));
    let out = detect(&m);
    let ks = kinds(&out);
    for k in [Kind::QuickKills, Kind::NoscopeKill, Kind::Wallbang, Kind::SmokeKill] {
        assert!(ks.contains(&(1, k, C1)), "{k:?} missing: {ks:?}");
    }
    assert!(ks.contains(&(1, Kind::TeamDamage, C2)), "{ks:?}");
    assert!(!ks.iter().any(|(_, _, p)| *p == T1), "post-match kill ignored: {ks:?}");
    // 一次击杀有两个标记 → 两条，各自带上全部标记
    let wb = out.iter().find(|h| h.kind == Kind::Wallbang).unwrap();
    assert_eq!(wb.detail["flags"], serde_json::json!(["wallbang", "smoke_kill"]));
    // 分数都在 0–100
    assert!(out.iter().all(|h| (0.0..=100.0).contains(&h.score)));
}

#[test]
fn swing_events_use_side_delta() {
    let mut m = base(1);
    m.win_prob = vec![
        // C1 杀 T1：T 胜率 0.5 → 0.2，CT +30pp → 关键击杀；T1 −30pp → 大幅负 Swing
        WinProbIn { round: 1, seq: 1, tick: 2000, kind: "kill".into(), player: Some(C1), victim: Some(T1), t_alive: 1, ct_alive: 2, p_before: 0.5, p_after: 0.2 },
        // 小变化：都不算
        WinProbIn { round: 1, seq: 2, tick: 2500, kind: "kill".into(), player: Some(T2), victim: Some(C2), t_alive: 1, ct_alive: 1, p_before: 0.2, p_after: 0.3 },
    ];
    let out = detect(&m);
    let ks = kinds(&out);
    assert_eq!(ks, vec![(1, Kind::SwingKill, C1), (1, Kind::SwingLoss, T1)]);
    assert_eq!(out[0].detail["deltaPp"], 30.0);
    assert_eq!(out[1].detail["deltaPp"], -30.0);
}

#[test]
fn kind_table_is_consistent() {
    for k in Kind::ALL {
        assert_eq!(Kind::parse(k.as_str()), Some(*k));
        assert!(k.default_weight() > 0.0);
    }
    assert_eq!(Kind::ALL.len(), 19);
}

#[test]
fn unused_utility_needs_purchase_this_round() {
    let mut m = base(2);
    for pr in m.players.iter_mut().filter(|p| p.steamid == T1) {
        pr.died = true;
        pr.death_tick = Some(3000);
        pr.unused_utility_value = 2000; // stats 的推算值（带进了以前回合的道具）
    }
    let buy = |r: u32, tick: i32, item: &str, cost: i32| PurchaseIn { round: r, tick, player: T1, item: item.into(), cost: Some(cost), was_sold: false };
    // 第 1 回合买了闪光和烟；第 2 回合什么都没买；第 2 回合死后成批重发的购买（超出购买时间）不算
    m.purchases.extend([buy(1, 900, "Flashbang", 200), buy(1, 905, "Smoke Grenade", 300), buy(2, 3000, "Flashbang", 200)]);
    let out = detect(&m);
    let hits: Vec<_> = out.iter().filter(|h| h.kind == Kind::UnusedUtilityDeath).collect();
    assert_eq!(hits.len(), 1, "{:?}", kinds(&out));
    assert_eq!((hits[0].round, hits[0].detail["value"].as_i64()), (1, Some(500)));
}
