//! The rules. All thresholds are constants at the top; see docs/highlights.md for the table.

use crate::model::*;
use crate::Kind;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};

// ── 高光 ──
/// 短时间连杀：相邻两次击杀的最大间隔（秒）。
pub const QUICK_GAP_SECS: f32 = 1.2;
/// 只有两杀时，两次击杀必须在这个时间内（秒）。
pub const QUICK_PAIR_SPAN_SECS: f32 = 0.8;
/// 甩枪：对方进入视野时准星离对方头部的最小夹角（度）。
pub const FLICK_MIN_DEG: f64 = 15.0;
/// 甩枪：从看到对方到造成第一次伤害的最长时间（毫秒）。
pub const FLICK_MAX_TTD_MS: f64 = 500.0;
/// 甩枪：第一次伤害之后多久内完成爆头击杀（秒）。
pub const FLICK_KILL_SECS: f32 = 0.5;
/// 关键击杀：这一次击杀让本方回合胜率至少提高多少（百分点）。
pub const SWING_KILL_MIN_PP: f64 = 25.0;
/// 极限拆包：拆完时 C4 剩余时间不超过（秒）。
pub const CLOSE_DEFUSE_SECS: f32 = 1.0;
pub const BOMB_TIME_SECS: f32 = 40.0;

// ── 失误 ──
/// 死亡时道具没用：身上道具价值下限（$），一颗闪光 = 200。
pub const UNUSED_UTILITY_MIN: i32 = 200;
/// 购买时间：freeze end 之后多少秒内的购买算本回合买的（同 stats 的购买口径）。
pub const BUY_WINDOW_SECS: f32 = 21.0;
const UTILITY_ITEMS: &[(&str, i32)] = &[
    ("Flashbang", 200),
    ("Smoke Grenade", 300),
    ("High Explosive Grenade", 300),
    ("Molotov", 400),
    ("Incendiary Grenade", 500),
    ("Decoy Grenade", 50),
];
/// 闪到队友：单个队友被致盲的最短时间（秒）。
pub const TEAM_FLASH_MIN_SECS: f32 = 1.5;
/// 队友伤害：一回合内对队友造成的最少伤害。
pub const TEAM_DAMAGE_MIN: i32 = 25;
/// 血量优势输掉对枪：交火开始时自己比对方至少多多少血。
pub const HP_ADV_MIN: i32 = 40;
/// 对枪窗口：击杀之前多少秒内的伤害和开枪算这次对枪（秒）。
pub const DUEL_WINDOW_SECS: f32 = 4.0;
/// 近距离连发全空：至少几发。
pub const WHIFF_MIN_SHOTS: usize = 5;
/// 近距离连发全空：连发结束后多少秒内被打死（秒）。
pub const WHIFF_DEATH_SECS: f32 = 2.0;
/// 近距离连发全空：被击杀时与凶手的最远距离（米）。
pub const WHIFF_MAX_DIST_M: f32 = 12.0;
/// 大幅负 Swing：单个事件让本方回合胜率至少下降多少（百分点）。
pub const SWING_LOSS_MIN_PP: f64 = 25.0;

const SNIPERS: &[&str] = &["awp", "ssg08", "scar20", "g3sg1"];
const AUTOMATICS: &[&str] = &[
    "ak47", "m4a1", "m4a1_silencer", "galilar", "famas", "aug", "sg556", "mp9", "mac10", "mp7", "mp5sd", "ump45", "p90",
    "bizon", "negev", "m249",
];
const GRENADE_WEAPONS: &[&str] = &["hegrenade", "inferno", "molotov", "incgrenade", "flashbang", "smokegrenade", "decoy"];

fn sid(s: u64) -> String {
    s.to_string()
}

fn opt_sid(s: Option<u64>) -> serde_json::Value {
    s.map(|v| json!(v.to_string())).unwrap_or(serde_json::Value::Null)
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Canonical gun name, e.g. `5e_2023pass1_ak47` → `ak47`. `None` for knives, grenades, world…
pub fn gun(weapon: &str) -> Option<&'static str> {
    aim::weapons::canonical(weapon).map(|w| w.name)
}

fn is_grenade(weapon: &str) -> bool {
    let w = weapon.strip_prefix("weapon_").unwrap_or(weapon);
    GRENADE_WEAPONS.iter().any(|g| w == *g || w.ends_with(&format!("_{g}")))
}

fn weapon_label(weapon: &str) -> String {
    gun(weapon).map(str::to_string).unwrap_or_else(|| weapon.strip_prefix("weapon_").unwrap_or(weapon).to_string())
}

pub struct Ctx<'a> {
    pub inp: &'a MatchInput,
    tr: f32,
    rounds: BTreeMap<u32, &'a RoundIn>,
    last_round: u32,
    side: HashMap<(u32, u64), u8>,
    kills: BTreeMap<u32, Vec<&'a KillIn>>,
    damages: BTreeMap<u32, Vec<&'a DamageIn>>,
    shots: BTreeMap<(u32, u64), Vec<&'a ShotIn>>,
}

impl<'a> Ctx<'a> {
    pub fn new(inp: &'a MatchInput) -> Self {
        let tr = if inp.tick_rate > 1.0 { inp.tick_rate } else { 64.0 };
        let rounds: BTreeMap<u32, &RoundIn> = inp.rounds.iter().map(|r| (r.number, r)).collect();
        let last_round = rounds.keys().next_back().copied().unwrap_or(0);
        let side = inp.players.iter().map(|p| ((p.round, p.steamid), p.side)).collect();
        let mut kills: BTreeMap<u32, Vec<&KillIn>> = BTreeMap::new();
        for k in &inp.kills {
            kills.entry(k.round).or_default().push(k);
        }
        let mut damages: BTreeMap<u32, Vec<&DamageIn>> = BTreeMap::new();
        for d in &inp.damages {
            damages.entry(d.round).or_default().push(d);
        }
        let mut shots: BTreeMap<(u32, u64), Vec<&ShotIn>> = BTreeMap::new();
        for s in &inp.shots {
            shots.entry((s.round, s.shooter)).or_default().push(s);
        }
        for v in kills.values_mut() {
            v.sort_by_key(|k| k.tick);
        }
        for v in damages.values_mut() {
            v.sort_by_key(|d| d.tick);
        }
        for v in shots.values_mut() {
            v.sort_by_key(|s| s.tick);
        }
        let mut c = Ctx { inp, tr, rounds, last_round, side, kills, damages, shots };
        // Events after the final round end (e.g. platform servers' `world` suicides) are dropped.
        let last_end = c.rounds.get(&c.last_round).map(|r| r.end_tick);
        if let Some(end) = last_end {
            if let Some(v) = c.kills.get_mut(&c.last_round) {
                v.retain(|k| k.tick <= end);
            }
            if let Some(v) = c.damages.get_mut(&c.last_round) {
                v.retain(|d| d.tick <= end);
            }
        }
        c
    }

    fn ticks(&self, secs: f32) -> i32 {
        (secs * self.tr).round() as i32
    }

    fn ms(&self, ticks: i32) -> f64 {
        (ticks as f64 * 1000.0 / self.tr as f64).round()
    }

    fn side_of(&self, round: u32, p: u64) -> Option<u8> {
        self.side.get(&(round, p)).copied()
    }

    fn kills_in(&self, round: u32) -> &[&'a KillIn] {
        self.kills.get(&round).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn damages_in(&self, round: u32) -> &[&'a DamageIn] {
        self.damages.get(&round).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn team(&self, round: u32, p: Option<u64>, recorded: Option<u8>) -> Option<u8> {
        recorded.filter(|t| *t == TEAM_T || *t == TEAM_CT).or_else(|| p.and_then(|p| self.side_of(round, p)))
    }

    /// Attacker and victim of a kill on an enemy (no suicides, no team kills).
    fn enemy_kill(&self, k: &KillIn) -> Option<(u64, u64)> {
        let (a, v) = (k.attacker?, k.victim?);
        if a == v {
            return None;
        }
        let (ta, tv) = (self.team(k.round, Some(a), k.attacker_team)?, self.team(k.round, Some(v), k.victim_team)?);
        (ta != tv).then_some((a, v))
    }

    fn enemy_damage(&self, d: &DamageIn) -> Option<(u64, u64)> {
        let (a, v) = (d.attacker?, d.victim?);
        if a == v {
            return None;
        }
        let (ta, tv) = (self.team(d.round, Some(a), d.attacker_team)?, self.team(d.round, Some(v), d.victim_team)?);
        (ta != tv).then_some((a, v))
    }

    /// Health of `p` just before `tick` (100 at round start).
    fn hp_before(&self, round: u32, p: u64, tick: i32) -> i32 {
        self.damages_in(round)
            .iter()
            .take_while(|d| d.tick < tick)
            .filter(|d| d.victim == Some(p))
            .last()
            .map(|d| d.health_after.max(0))
            .unwrap_or(100)
    }

    /// Players of `side` still alive just after `tick` (participants minus deaths up to `tick`).
    fn alive_after(&self, round: u32, side: u8, tick: i32) -> i32 {
        let mut alive: Vec<u64> =
            self.inp.players.iter().filter(|p| p.round == round && p.side == side).map(|p| p.steamid).collect();
        for k in self.kills_in(round).iter().take_while(|k| k.tick <= tick) {
            if let Some(v) = k.victim {
                alive.retain(|x| *x != v);
            }
        }
        alive.len() as i32
    }

    fn round_lost(&self, round: u32, side: u8) -> bool {
        self.rounds.get(&round).and_then(|r| r.winner).is_some_and(|w| (w == TEAM_T || w == TEAM_CT) && w != side)
    }

    fn kill_json(&self, k: &KillIn) -> serde_json::Value {
        json!({
            "tick": k.tick,
            "victim": opt_sid(k.victim),
            "weapon": weapon_label(&k.weapon),
            "headshot": k.headshot,
        })
    }

    fn push(&self, out: &mut Vec<Highlight>, kind: Kind, round: u32, start: i32, end: i32, p: u64, score: f64, detail: serde_json::Value) {
        out.push(Highlight {
            round,
            start_tick: start.min(end),
            end_tick: end.max(start),
            steamid: p,
            kind,
            score: round1(clamp(score, 0.0, 100.0)),
            detail,
        });
    }

    /// Enemy kills by `p` in `round`, in tick order.
    fn kills_by(&self, round: u32, p: u64) -> Vec<&'a KillIn> {
        self.kills_in(round).iter().copied().filter(|k| self.enemy_kill(k).is_some_and(|(a, _)| a == p)).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// 高光
// ---------------------------------------------------------------------------------------------

/// 3K / 4K / ACE.
pub fn multi_kill(c: &Ctx, out: &mut Vec<Highlight>) {
    for pr in c.inp.players.iter().filter(|p| p.kills >= 3) {
        let ks = c.kills_by(pr.round, pr.steamid);
        let n = ks.len();
        if n < 3 {
            continue;
        }
        let score = match n {
            3 => 50.0,
            4 => 75.0,
            _ => 100.0,
        } + 2.0 * ks.iter().filter(|k| k.headshot).count() as f64;
        let detail = json!({
            "count": n,
            "spanMs": c.ms(ks[n - 1].tick - ks[0].tick),
            "kills": ks.iter().map(|k| c.kill_json(k)).collect::<Vec<_>>(),
        });
        c.push(out, Kind::MultiKill, pr.round, ks[0].tick, ks[n - 1].tick, pr.steamid, score, detail);
    }
}

/// 短时间连杀：相邻击杀间隔 ≤ 1.2 秒；两杀要在 0.8 秒内，三杀以上不限总时长。
pub fn quick_kills(c: &Ctx, out: &mut Vec<Highlight>) {
    let gap = c.ticks(QUICK_GAP_SECS);
    let pair = c.ticks(QUICK_PAIR_SPAN_SECS);
    let mut by_player: BTreeMap<(u32, u64), Vec<&KillIn>> = BTreeMap::new();
    for ks in c.kills.values() {
        for k in ks {
            if let Some((a, _)) = c.enemy_kill(k) {
                by_player.entry((k.round, a)).or_default().push(k);
            }
        }
    }
    for ((round, p), ks) in by_player {
        let mut i = 0;
        while i < ks.len() {
            let mut j = i;
            while j + 1 < ks.len() && ks[j + 1].tick - ks[j].tick <= gap {
                j += 1;
            }
            let n = j - i + 1;
            let span = ks[j].tick - ks[i].tick;
            if n >= 3 || (n == 2 && span <= pair) {
                let span_s = span as f64 / c.tr as f64;
                let score = 35.0 + 15.0 * (n as f64 - 2.0) + 10.0 * (1.0 - span_s / (n as f64 - 1.0)).max(0.0);
                let detail = json!({
                    "count": n,
                    "spanMs": c.ms(span),
                    "kills": ks[i..=j].iter().map(|k| c.kill_json(k)).collect::<Vec<_>>(),
                });
                c.push(out, Kind::QuickKills, round, ks[i].tick, ks[j].tick, p, score, detail);
            }
            i = j + 1;
        }
    }
}

/// 残局 1vN 获胜（口径与 stats 的 clutch 一致，这里补上残局开始的 tick）。
pub fn clutch_win(c: &Ctx, out: &mut Vec<Highlight>) {
    for pr in c.inp.players.iter().filter(|p| p.clutch_won == Some(true)) {
        let Some(vs) = pr.clutch_vs.filter(|v| *v >= 1) else { continue };
        let Some(round) = c.rounds.get(&pr.round) else { continue };
        let mut start = round.freeze_end_tick.unwrap_or(round.start_tick);
        // First moment the player is the last one alive on their side.
        for k in c.kills_in(pr.round) {
            if k.victim.is_some_and(|v| c.side_of(pr.round, v) == Some(pr.side))
                && c.alive_after(pr.round, pr.side, k.tick) == 1
            {
                start = k.tick;
                break;
            }
        }
        let clutch_kills: Vec<_> = c.kills_by(pr.round, pr.steamid).into_iter().filter(|k| k.tick >= start).collect();
        let end = clutch_kills.last().map(|k| k.tick).unwrap_or(round.end_tick).max(start);
        let score = [0.0, 35.0, 55.0, 72.0, 88.0, 100.0][vs.clamp(1, 5) as usize];
        let detail = json!({
            "vs": vs,
            "kills": clutch_kills.len(),
            "side": pr.side,
            "killList": clutch_kills.iter().map(|k| c.kill_json(k)).collect::<Vec<_>>(),
        });
        c.push(out, Kind::ClutchWin, pr.round, start, end, pr.steamid, score, detail);
    }
}

/// 大角度甩枪爆头：对方进入视野时准星离头 ≥ 15°，500 ms 内造成伤害，并在 0.5 秒内爆头击杀。
pub fn flick_headshot(c: &Ctx, out: &mut Vec<Highlight>) {
    let Some(engs) = &c.inp.engagements else { return };
    let mut seen = std::collections::HashSet::new();
    let kill_win = c.ticks(FLICK_KILL_SECS);
    for e in engs.iter().filter(|e| e.role == "attacker" && e.killed) {
        let (Some(cp), Some(ttd)) = (e.cp_deg, e.ttd_ms) else { continue };
        if !(FLICK_MIN_DEG..=90.0).contains(&cp) || ttd > FLICK_MAX_TTD_MS {
            continue;
        }
        let Some(k) = c.kills_in(e.round).iter().find(|k| {
            k.attacker == Some(e.observer) && k.victim == Some(e.target)
                && k.tick >= e.damage_tick
                && k.tick - e.damage_tick <= kill_win
                && k.headshot
        }) else {
            continue;
        };
        if gun(&k.weapon).is_none() || c.enemy_kill(k).is_none() || !seen.insert((k.round, k.tick, e.observer)) {
            continue;
        }
        let score = 45.0 + (cp - FLICK_MIN_DEG) * 1.5 + (FLICK_MAX_TTD_MS - ttd) / 25.0;
        let detail = json!({
            "angle": round1(cp),
            "ttdMs": ttd.round(),
            "weapon": weapon_label(&k.weapon),
            "victim": opt_sid(k.victim),
            "distance": k.distance.map(|d| round1(d as f64)),
        });
        c.push(out, Kind::FlickHeadshot, e.round, e.onset_tick, k.tick, e.observer, score, detail);
    }
}

/// 穿墙 / 穿烟 / 盲杀 / 跳杀 / 无镜狙：击杀事件自带的标记。一次击杀有多个标记时每个标记一条。
pub fn special_kills(c: &Ctx, out: &mut Vec<Highlight>) {
    for ks in c.kills.values() {
        for k in ks {
            let Some((a, _)) = c.enemy_kill(k) else { continue };
            let g = gun(&k.weapon);
            let sniper = g.is_some_and(|g| SNIPERS.contains(&g));
            let hs = if k.headshot { 10.0 } else { 0.0 };
            let dist = k.distance.unwrap_or(0.0) as f64;
            let mut flags: Vec<(Kind, f64)> = Vec::new();
            if k.penetrated && g.is_some() {
                flags.push((Kind::Wallbang, 45.0 + hs));
            }
            if k.thrusmoke && g.is_some() {
                flags.push((Kind::SmokeKill, 40.0 + hs));
            }
            if k.attacker_blind && !is_grenade(&k.weapon) {
                flags.push((Kind::BlindKill, 45.0 + hs));
            }
            if k.attacker_in_air && !is_grenade(&k.weapon) {
                flags.push((Kind::AirborneKill, 40.0 + hs + if sniper { 20.0 } else { 0.0 }));
            }
            if k.noscope && sniper {
                flags.push((Kind::NoscopeKill, 50.0 + hs + dist.min(30.0)));
            }
            if flags.is_empty() {
                continue;
            }
            let names: Vec<&str> = flags.iter().map(|(k, _)| k.as_str()).collect();
            for (kind, score) in &flags {
                let detail = json!({
                    "weapon": weapon_label(&k.weapon),
                    "victim": opt_sid(k.victim),
                    "headshot": k.headshot,
                    "distance": k.distance.map(|d| round1(d as f64)),
                    "flags": names,
                });
                c.push(out, *kind, k.round, k.tick, k.tick, a, *score, detail);
            }
        }
    }
}

/// 高 Swing 关键击杀 / 单次事件的大幅负 Swing，来自入库的胜率曲线（`round_win_probability`）。
/// 用的是事件让**本方**胜率变了多少（不是个人分成），这样和曲线上看到的跳变一致。
pub fn swing_events(c: &Ctx, out: &mut Vec<Highlight>) {
    for w in &c.inp.win_prob {
        let d_t = (w.p_after - w.p_before) * 100.0;
        let own = |side: u8| if side == TEAM_T { d_t } else { -d_t };
        let own_before = |side: u8| if side == TEAM_T { w.p_before } else { 1.0 - w.p_before };
        match w.kind.as_str() {
            "kill" => {
                if let Some(a) = w.player {
                    if let Some(sa) = c.side_of(w.round, a) {
                        let gain = own(sa);
                        if gain >= SWING_KILL_MIN_PP {
                            let detail = json!({
                                "deltaPp": round1(gain),
                                "before": round1(own_before(sa) * 100.0),
                                "after": round1((own_before(sa) * 100.0 + gain).clamp(0.0, 100.0)),
                                "victim": opt_sid(w.victim),
                                "tAlive": w.t_alive, "ctAlive": w.ct_alive,
                            });
                            c.push(out, Kind::SwingKill, w.round, w.tick, w.tick, a, 10.0 + gain * 1.6, detail);
                        }
                    }
                }
                if let Some(v) = w.victim {
                    swing_loss(c, out, w, v, "death", &own, &own_before);
                }
            }
            "team_kill" => {
                if let Some(a) = w.player {
                    swing_loss(c, out, w, a, "team_kill", &own, &own_before);
                }
            }
            "suicide" | "world" => {
                if let Some(v) = w.victim {
                    swing_loss(c, out, w, v, "suicide", &own, &own_before);
                }
            }
            _ => {}
        }
    }
}

fn swing_loss(
    c: &Ctx,
    out: &mut Vec<Highlight>,
    w: &WinProbIn,
    p: u64,
    what: &str,
    own: &dyn Fn(u8) -> f64,
    own_before: &dyn Fn(u8) -> f64,
) {
    let Some(side) = c.side_of(w.round, p) else { return };
    let loss = -own(side);
    if loss < SWING_LOSS_MIN_PP {
        return;
    }
    let detail = json!({
        "deltaPp": round1(-loss),
        "before": round1(own_before(side) * 100.0),
        "after": round1((own_before(side) * 100.0 - loss).clamp(0.0, 100.0)),
        "event": what,
        "killer": if what == "death" { opt_sid(w.player) } else { serde_json::Value::Null },
        "victim": opt_sid(w.victim),
        "tAlive": w.t_alive, "ctAlive": w.ct_alive,
    });
    c.push(out, Kind::SwingLoss, w.round, w.tick, w.tick, p, 10.0 + loss * 1.6, detail);
}

/// 关键拆包（T 还有人活着、或 C4 剩 1 秒以内）/ 忍者拆包（T 还有人活着，拆包者下包后没有击杀）。
pub fn defuses(c: &Ctx, out: &mut Vec<Highlight>) {
    for b in c.inp.bombs.iter().filter(|b| b.kind == "defused") {
        let Some(p) = b.player else { continue };
        let before: Vec<&BombIn> = c.inp.bombs.iter().filter(|x| x.round == b.round && x.tick <= b.tick).collect();
        let Some(plant) = before.iter().filter(|x| x.kind == "planted").map(|x| x.tick).max() else { continue };
        let begin = before
            .iter()
            .filter(|x| x.kind == "defuse_begin" && x.player == Some(p))
            .map(|x| x.tick)
            .max()
            .unwrap_or(b.tick - c.ticks(5.0));
        let remaining = BOMB_TIME_SECS - (b.tick - plant) as f32 / c.tr;
        let t_alive = c.alive_after(b.round, TEAM_T, b.tick);
        let kills_after_plant = c.kills_by(b.round, p).iter().filter(|k| k.tick >= plant && k.tick <= b.tick).count();
        let close = remaining <= CLOSE_DEFUSE_SECS;
        let detail = json!({
            "tAlive": t_alive,
            "remainingSecs": round1(remaining.max(0.0) as f64),
            "killsAfterPlant": kills_after_plant,
        });
        if t_alive >= 1 && kills_after_plant == 0 {
            let score = 65.0 + 10.0 * (t_alive - 1) as f64 + if close { 10.0 } else { 0.0 };
            c.push(out, Kind::NinjaDefuse, b.round, begin, b.tick, p, score, detail);
        } else if t_alive >= 1 || close {
            let score = 45.0 + 10.0 * t_alive as f64 + if close { 20.0 } else { 0.0 };
            c.push(out, Kind::KeyDefuse, b.round, begin, b.tick, p, score, detail);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 失误
// ---------------------------------------------------------------------------------------------

fn death_kill<'a>(c: &Ctx<'a>, round: u32, p: u64, tick: i32) -> Option<&'a KillIn> {
    c.kills_in(round).iter().copied().find(|k| k.victim == Some(p) && k.tick == tick)
}

/// 首死且没被补。
pub fn untraded_opening_death(c: &Ctx, out: &mut Vec<Highlight>) {
    for pr in c.inp.players.iter().filter(|p| p.opening_death && !p.traded) {
        let Some(t) = pr.death_tick else { continue };
        let k = death_kill(c, pr.round, pr.steamid, t);
        let lost = c.round_lost(pr.round, pr.side);
        let detail = json!({
            "killer": opt_sid(k.and_then(|k| k.attacker)),
            "weapon": k.map(|k| weapon_label(&k.weapon)),
            "roundLost": lost,
            "side": pr.side,
        });
        let score = 40.0 + if lost { 15.0 } else { 0.0 };
        c.push(out, Kind::UntradedOpeningDeath, pr.round, t, t, pr.steamid, score, detail);
    }
}

/// 这一回合在购买时间内买的道具总价（`item_purchase` 在死亡 / 回合末会成批重发，所以只看
/// freeze end 后 [`BUY_WINDOW_SECS`] 秒以内的购买，同 stats）。
fn utility_bought(c: &Ctx, round: u32, p: u64) -> i32 {
    let Some(r) = c.rounds.get(&round) else { return 0 };
    let until = r.freeze_end_tick.unwrap_or(r.start_tick) + c.ticks(BUY_WINDOW_SECS);
    let mut buys: Vec<&PurchaseIn> = c
        .inp
        .purchases
        .iter()
        .filter(|b| b.round == round && b.player == p && !b.was_sold && b.tick <= until)
        .filter(|b| UTILITY_ITEMS.iter().any(|(name, _)| b.item == *name))
        .collect();
    buys.sort_by_key(|b| b.tick);
    // 重发的批次：同一 tick 多件、而且每件之前都买过 → 丢掉
    let mut kept: Vec<&PurchaseIn> = Vec::new();
    let mut i = 0;
    while i < buys.len() {
        let j = buys[i..].iter().take_while(|b| b.tick == buys[i].tick).count() + i;
        let batch = &buys[i..j];
        let repeat = batch.len() >= 2 && batch.iter().all(|b| kept.iter().any(|k| k.item == b.item));
        if !repeat {
            kept.extend(batch);
        }
        i = j;
    }
    // 携带上限：闪光 2 颗，其他各 1 颗（燃烧瓶 / 燃烧弹共用一格）
    let mut total = 0;
    let mut slots: HashMap<&str, i32> = HashMap::new();
    for b in kept {
        let slot = if b.item == "Molotov" || b.item == "Incendiary Grenade" { "fire" } else { b.item.as_str() };
        let limit = if b.item == "Flashbang" { 2 } else { 1 };
        let n = slots.entry(slot).or_default();
        if *n < limit {
            *n += 1;
            total += b.cost.unwrap_or_else(|| UTILITY_ITEMS.iter().find(|(n, _)| b.item == *n).map(|x| x.1).unwrap_or(0));
        }
    }
    total
}

/// 死亡时道具一颗没用：这一回合买了道具、一颗没扔就死了。
/// 道具价值取 stats 推算的「身上未用道具」和本回合购买额里较小的一个（推算值会把以前回合的带进来）。
pub fn unused_utility_death(c: &Ctx, out: &mut Vec<Highlight>) {
    for pr in c.inp.players.iter().filter(|p| p.died && p.utility_thrown == 0 && p.unused_utility_value >= UNUSED_UTILITY_MIN) {
        let Some(t) = pr.death_tick else { continue };
        if c.rounds.get(&pr.round).is_some_and(|r| t > r.end_tick) {
            continue; // 回合结束后被补的不算
        }
        let bought = utility_bought(c, pr.round, pr.steamid);
        let value = pr.unused_utility_value.min(bought);
        if value < UNUSED_UTILITY_MIN {
            continue;
        }
        let k = death_kill(c, pr.round, pr.steamid, t);
        let detail = json!({
            "value": value,
            "killer": opt_sid(k.and_then(|k| k.attacker)),
        });
        let score = 20.0 + value.min(1000) as f64 / 25.0;
        c.push(out, Kind::UnusedUtilityDeath, pr.round, t, t, pr.steamid, score, detail);
    }
}

/// 闪到队友：同一颗闪光（同一投掷者、相差 ≤ 2 tick 的致盲事件）让队友致盲 ≥ 1.5 秒，
/// 且致盲敌人的总时长不超过队友的。
pub fn team_flash(c: &Ctx, out: &mut Vec<Highlight>) {
    if !c.inp.blind_data {
        return;
    }
    let mut groups: BTreeMap<(u32, u64), Vec<Vec<&BlindIn>>> = BTreeMap::new();
    let mut blinds: Vec<&BlindIn> = c.inp.blinds.iter().filter(|b| b.attacker.is_some() && b.victim.is_some()).collect();
    blinds.sort_by_key(|b| (b.round, b.attacker, b.tick));
    for b in blinds {
        let list = groups.entry((b.round, b.attacker.unwrap())).or_default();
        match list.last_mut() {
            Some(g) if b.tick - g[0].tick <= 2 => g.push(b),
            _ => list.push(vec![b]),
        }
    }
    for ((round, a), list) in groups {
        let Some(sa) = c.team(round, Some(a), None) else { continue };
        for g in list {
            let mut mates = Vec::new();
            let mut team_secs = 0.0f32;
            let mut enemy_secs = 0.0f32;
            for b in &g {
                let v = b.victim.unwrap();
                if v == a {
                    continue;
                }
                match c.team(round, Some(v), b.victim_team) {
                    Some(t) if t == sa => {
                        if b.duration >= TEAM_FLASH_MIN_SECS {
                            mates.push(json!({ "steamid": sid(v), "duration": round1(b.duration as f64) }));
                        }
                        team_secs += b.duration;
                    }
                    Some(_) => enemy_secs += b.duration,
                    None => {}
                }
            }
            if mates.is_empty() || enemy_secs >= team_secs {
                continue;
            }
            let detail = json!({
                "victims": mates,
                "teamSecs": round1(team_secs as f64),
                "enemySecs": round1(enemy_secs as f64),
            });
            let t = g[0].tick;
            c.push(out, Kind::TeamFlash, round, t, t, a, 15.0 + team_secs as f64 * 12.0, detail);
        }
    }
}

/// 队友伤害：一回合内对队友的伤害 ≥ 25，或误杀队友。
pub fn team_damage(c: &Ctx, out: &mut Vec<Highlight>) {
    #[derive(Default)]
    struct Acc {
        dmg: i32,
        first: i32,
        last: i32,
        victims: BTreeMap<u64, i32>,
        weapons: Vec<String>,
        team_kills: Vec<u64>,
    }
    let mut acc: BTreeMap<(u32, u64), Acc> = BTreeMap::new();
    let mut rows: Vec<(u32, u64, i32, Option<(u64, i32, String)>, Option<u64>)> = Vec::new();
    for ds in c.damages.values() {
        for d in ds {
            let (Some(a), Some(v)) = (d.attacker, d.victim) else { continue };
            if a == v {
                continue;
            }
            let (ta, tv) = (c.team(d.round, Some(a), d.attacker_team), c.team(d.round, Some(v), d.victim_team));
            if ta.is_some() && ta == tv {
                rows.push((d.round, a, d.tick, Some((v, d.dmg_health, weapon_label(&d.weapon))), None));
            }
        }
    }
    for ks in c.kills.values() {
        for k in ks {
            let (Some(a), Some(v)) = (k.attacker, k.victim) else { continue };
            if a == v {
                continue;
            }
            let (ta, tv) = (c.team(k.round, Some(a), k.attacker_team), c.team(k.round, Some(v), k.victim_team));
            if ta.is_some() && ta == tv {
                rows.push((k.round, a, k.tick, None, Some(v)));
            }
        }
    }
    for (round, a, tick, dmg, tk) in rows {
        let e = acc.entry((round, a)).or_insert_with(|| Acc { first: tick, last: tick, ..Default::default() });
        e.first = e.first.min(tick);
        e.last = e.last.max(tick);
        if let Some((v, h, w)) = dmg {
            e.dmg += h;
            *e.victims.entry(v).or_default() += h;
            if !e.weapons.contains(&w) {
                e.weapons.push(w);
            }
        }
        if let Some(v) = tk {
            e.team_kills.push(v);
        }
    }
    for ((round, a), e) in acc {
        if e.dmg < TEAM_DAMAGE_MIN && e.team_kills.is_empty() {
            continue;
        }
        let detail = json!({
            "damage": e.dmg,
            "victims": e.victims.iter().map(|(v, d)| json!({ "steamid": sid(*v), "damage": d })).collect::<Vec<_>>(),
            "teamKills": e.team_kills.iter().map(|v| sid(*v)).collect::<Vec<_>>(),
            "weapons": e.weapons,
        });
        let score = 15.0 + e.dmg as f64 * 0.4 + 40.0 * e.team_kills.len() as f64;
        c.push(out, Kind::TeamDamage, round, e.first, e.last, a, score, detail);
    }
}

/// 有血量优势却输掉对枪：一对一（窗口内没有第三人参与），交火开始时自己比对方多 ≥ 40 血，
/// 自己也开过枪，最后被对方击杀。
pub fn lost_hp_duel(c: &Ctx, out: &mut Vec<Highlight>) {
    let win = c.ticks(DUEL_WINDOW_SECS);
    let pre = c.ticks(1.0);
    for ks in c.kills.values() {
        for k in ks {
            let Some((killer, victim)) = c.enemy_kill(k) else { continue };
            let round = k.round;
            let from = k.tick - win;
            let dmgs: Vec<&&DamageIn> = c.damages_in(round).iter().filter(|d| d.tick >= from - pre && d.tick <= k.tick).collect();
            let duel: Vec<&&&DamageIn> = dmgs
                .iter()
                .filter(|d| d.tick >= from)
                .filter(|d| {
                    (d.attacker == Some(killer) && d.victim == Some(victim)) || (d.attacker == Some(victim) && d.victim == Some(killer))
                })
                .collect();
            let start = duel.first().map(|d| d.tick).unwrap_or(k.tick);
            // Third parties: someone else hurt the victim, or a teammate of the victim hurt the killer.
            let third = dmgs.iter().filter(|d| d.tick >= start - pre).any(|d| {
                c.enemy_damage(d).is_some_and(|(a, v)| (v == victim && a != killer) || (v == killer && a != victim))
            });
            if third {
                continue;
            }
            let fired = c
                .shots
                .get(&(round, victim))
                .is_some_and(|s| s.iter().any(|s| s.tick >= from && s.tick <= k.tick && gun(&s.weapon).is_some()));
            if !fired {
                continue;
            }
            let blinded = c.inp.blinds.iter().any(|b| {
                b.round == round && b.victim == Some(victim) && b.tick <= k.tick && b.tick as f32 + b.duration * c.tr >= start as f32
            });
            if blinded {
                continue;
            }
            let hp = c.hp_before(round, victim, start);
            let enemy_hp = c.hp_before(round, killer, start);
            let diff = hp - enemy_hp;
            if diff < HP_ADV_MIN {
                continue;
            }
            let detail = json!({
                "hp": hp,
                "enemyHp": enemy_hp,
                "killer": sid(killer),
                "weapon": weapon_label(&k.weapon),
                "dealt": duel.iter().filter(|d| d.attacker == Some(victim)).map(|d| d.dmg_health).sum::<i32>(),
            });
            let score = 30.0 + (diff - HP_ADV_MIN) as f64 * 0.6;
            c.push(out, Kind::LostHpDuel, round, start, k.tick, victim, score.min(75.0), detail);
        }
    }
}

/// 近距离连发全空：自动武器连发 ≥ 5 发、没有对敌人造成任何伤害，随后 2 秒内被 12 米以内的敌人击杀。
pub fn whiffed_spray(c: &Ctx, out: &mut Vec<Highlight>) {
    for (&(round, p), shots) in &c.shots {
        let mut i = 0;
        while i < shots.len() {
            let Some(g) = gun(&shots[i].weapon).filter(|g| AUTOMATICS.contains(g)) else {
                i += 1;
                continue;
            };
            let info = aim::weapons::canonical(g).unwrap();
            let max_gap = (info.cycle_ticks(c.tr) * 1.6).ceil() as i32 + 2;
            let mut j = i;
            while j + 1 < shots.len() && gun(&shots[j + 1].weapon) == Some(g) && shots[j + 1].tick - shots[j].tick <= max_gap {
                j += 1;
            }
            let n = j - i + 1;
            let (first, last) = (shots[i].tick, shots[j].tick);
            i = j + 1;
            if n < WHIFF_MIN_SHOTS {
                continue;
            }
            let hit = c.damages_in(round).iter().any(|d| {
                d.tick >= first && d.tick <= last + 2 && !is_grenade(&d.weapon) && c.enemy_damage(d).is_some_and(|(a, _)| a == p)
            });
            if hit {
                continue;
            }
            let death = c.kills_in(round).iter().find(|k| {
                k.victim == Some(p) && k.tick >= last && k.tick <= last + c.ticks(WHIFF_DEATH_SECS) && c.enemy_kill(k).is_some()
            });
            let Some(k) = death else { continue };
            let Some(dist) = k.distance.filter(|d| *d <= WHIFF_MAX_DIST_M) else { continue };
            let detail = json!({
                "shots": n,
                "weapon": g,
                "killer": opt_sid(k.attacker),
                "distance": round1(dist as f64),
            });
            let score = 30.0 + 3.0 * (n - WHIFF_MIN_SHOTS) as f64;
            c.push(out, Kind::WhiffedSpray, round, first, k.tick, p, score.min(70.0), detail);
        }
    }
}
