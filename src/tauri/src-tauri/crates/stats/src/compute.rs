//! The single pass that turns one match's events into [`MatchStats`].

use crate::econ::{BuyType, Nade, NadeInventory};
use crate::swing::{self, DeathKind, Participant, RoundInput, WinModel};
use crate::*;
use std::collections::{BTreeMap, HashMap, HashSet};

const FULL_HEALTH: i32 = 100;
/// Competitive buy time after freeze end (plus a little slack).
const BUY_TIME_SECS: f32 = 21.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum KillKind {
    Kill,
    TeamKill,
    Suicide,
    World,
}

impl KillKind {
    fn as_str(self) -> &'static str {
        match self {
            KillKind::Kill => "kill",
            KillKind::TeamKill => "team_kill",
            KillKind::Suicide => "suicide",
            KillKind::World => "world",
        }
    }
}

fn other_side(s: u8) -> u8 {
    if s == TEAM_T {
        TEAM_CT
    } else {
        TEAM_T
    }
}

fn is_side(t: Option<u8>) -> Option<u8> {
    t.filter(|t| *t == TEAM_T || *t == TEAM_CT)
}

/// Compute every derived statistic of one match, with the built-in (prior-only) Swing model.
pub fn compute(ev: MatchEvents<'_>) -> MatchStats {
    compute_with_model(ev, &WinModel::prior())
}

/// Compute every derived statistic of one match; Swing uses `model`.
pub fn compute_with_model(ev: MatchEvents<'_>, model: &WinModel) -> MatchStats {
    let tick_rate = if ev.tick_rate > 0.0 { ev.tick_rate } else { 64.0 };
    let window = (TRADE_WINDOW_SECS * tick_rate).round() as i32;

    let mut rounds: Vec<&Round> = ev.rounds.iter().collect();
    rounds.sort_by_key(|r| r.number);

    // ---- side of each player in each round: economy snapshot, then event teams as fallback ----
    let mut side: HashMap<(u32, u64), u8> = HashMap::new();
    for e in ev.economies {
        if let Some(t) = is_side(e.team) {
            side.insert((e.round, e.player), t);
        }
    }
    for k in ev.kills {
        if let (Some(a), Some(t)) = (k.attacker, is_side(k.attacker_team)) {
            side.entry((k.round, a)).or_insert(t);
        }
        if let (Some(v), Some(t)) = (k.victim, is_side(k.victim_team)) {
            side.entry((k.round, v)).or_insert(t);
        }
    }

    // ---- roster: demo players first, then anyone else who played a round ----
    let mut roster: Vec<u64> = ev.players.iter().map(|p| p.steamid).collect();
    let mut seen: HashSet<u64> = roster.iter().copied().collect();
    let mut extra: Vec<u64> = side.keys().map(|(_, p)| *p).filter(|p| !seen.contains(p)).collect();
    extra.sort();
    extra.dedup();
    for p in extra {
        seen.insert(p);
        roster.push(p);
    }
    let names: HashMap<u64, &str> = ev.players.iter().map(|p| (p.steamid, p.name.as_str())).collect();
    let team_start: HashMap<u64, u8> =
        ev.players.iter().filter_map(|p| Some((p.steamid, is_side(p.team_start)?))).collect();

    // ---- teams: A = players on CT in the first round; per-round side of team A ----
    let first_round = rounds.first().map(|r| r.number).unwrap_or(1);
    let mut seed_a: Vec<u64> = roster.iter().copied().filter(|p| side.get(&(first_round, *p)) == Some(&TEAM_CT)).collect();
    if seed_a.is_empty() {
        seed_a = roster.iter().copied().filter(|p| team_start.get(p) == Some(&TEAM_CT)).collect();
    }
    let mut side_a: BTreeMap<u32, u8> = BTreeMap::new();
    let mut prev = TEAM_CT;
    for r in &rounds {
        let (mut ct, mut t) = (0, 0);
        for p in &seed_a {
            match side.get(&(r.number, *p)) {
                Some(&TEAM_CT) => ct += 1,
                Some(&TEAM_T) => t += 1,
                _ => {}
            }
        }
        let s = if ct > t {
            TEAM_CT
        } else if t > ct {
            TEAM_T
        } else {
            prev
        };
        side_a.insert(r.number, s);
        prev = s;
    }
    let team_of_side = |round: u32, s: u8| -> Team {
        if side_a.get(&round).copied().unwrap_or(TEAM_CT) == s {
            Team::A
        } else {
            Team::B
        }
    };
    let mut team: HashMap<u64, Team> = HashMap::new();
    for p in &roster {
        let (mut a, mut b) = (0, 0);
        for r in &rounds {
            if let Some(s) = side.get(&(r.number, *p)) {
                match team_of_side(r.number, *s) {
                    Team::A => a += 1,
                    Team::B => b += 1,
                }
            }
        }
        let t = if a > b {
            Team::A
        } else if b > a {
            Team::B
        } else if team_start.get(p) == Some(&TEAM_T) {
            Team::B
        } else {
            Team::A
        };
        team.insert(*p, t);
    }

    // Pistol rounds: the first round and the first round after the first side swap.
    let mut pistol: HashSet<u32> = HashSet::new();
    pistol.insert(first_round);
    let first_side = side_a.get(&first_round).copied().unwrap_or(TEAM_CT);
    if let Some((r, _)) = side_a.iter().find(|(_, s)| **s != first_side) {
        pistol.insert(*r);
    }

    let blind_data_available = !(ev.blinds.is_empty() && ev.grenades.iter().any(|g| g.kind == "flashbang"));

    // ---- index events by round ----
    fn by_round<T, F: Fn(&T) -> u32>(items: &[T], f: F) -> HashMap<u32, Vec<&T>> {
        let mut m: HashMap<u32, Vec<&T>> = HashMap::new();
        for it in items {
            m.entry(f(it)).or_default().push(it);
        }
        m
    }
    let kills_by = by_round(ev.kills, |k| k.round);
    let dmg_by = by_round(ev.damages, |d| d.round);
    let nades_by = by_round(ev.grenades, |g| g.round);
    let blinds_by = by_round(ev.blinds, |b| b.round);
    let bombs_by = by_round(ev.bombs, |b| b.round);
    let buys_by = by_round(ev.purchases, |p| p.round);
    let econ: HashMap<(u32, u64), &Economy> = ev.economies.iter().map(|e| ((e.round, e.player), e)).collect();

    let mut player_rounds: Vec<PlayerRoundStats> = vec![];
    let mut team_rounds: Vec<TeamRoundStats> = vec![];
    let mut summaries: Vec<RoundSummary> = vec![];
    let mut pairs: BTreeMap<(u64, u64), PairStats> = BTreeMap::new();
    let mut win_probability: Vec<RoundWinProbability> = vec![];
    let mut swing_samples: Vec<BucketSample> = vec![];
    // Per-player extra counters not kept per round.
    #[derive(Default)]
    struct Extra {
        team_kills: u32,
        suicides: u32,
        deaths: u32,
        trade_opp: u32,
        trade_att: u32,
        traded_opp: u32,
        clutch_kills: u32,
        flashes: u32,
        he: u32,
        smokes: u32,
        fire: u32,
        decoys: u32,
        enemy_blind: f32,
        team_flashed: u32,
        team_blind: f32,
        he_damage: u32,
        he_team_damage: u32,
        fire_damage: u32,
    }
    let mut extra: HashMap<u64, Extra> = HashMap::new();
    let mut inventory: HashMap<u64, (u8, NadeInventory)> = HashMap::new();
    let (mut score_a, mut score_b) = (0u32, 0u32);

    for r in &rounds {
        let rn = r.number;
        let sa = side_a.get(&rn).copied().unwrap_or(TEAM_CT);
        let participants: Vec<u64> = roster.iter().copied().filter(|p| side.contains_key(&(rn, *p))).collect();
        let side_of = |p: u64| side.get(&(rn, p)).copied();
        let mut pr: HashMap<u64, PlayerRoundStats> = participants
            .iter()
            .map(|p| {
                let s = side_of(*p).unwrap();
                (
                    *p,
                    PlayerRoundStats {
                        round: rn,
                        steamid: *p,
                        team: team_of_side(rn, s),
                        side: s,
                        kills: 0,
                        died: false,
                        death_tick: None,
                        assists: 0,
                        flash_assists: 0,
                        headshot_kills: 0,
                        damage: 0,
                        utility_damage: 0,
                        kast: false,
                        traded: false,
                        trade_kills: 0,
                        opening_kill: false,
                        opening_death: false,
                        clutch_vs: None,
                        clutch_won: None,
                        mvp: false,
                        equip_value: 0,
                        balance: 0,
                        utility_thrown: 0,
                        enemies_flashed: if blind_data_available { Some(0) } else { None },
                        unused_utility_value: 0,
                        swing: 0.0,
                    },
                )
            })
            .collect();

        // ---- kills ----
        // Events after the final round's end happen after the match is over: ignored (the in-game
        // scoreboard ignores them too). Earlier post-round events stay in their round.
        let last_round = rounds.last().map(|x| x.number) == Some(rn);
        let in_match = |tick: i32| !last_round || tick <= r.end_tick;
        let mut kills: Vec<&Kill> =
            kills_by.get(&rn).map(|v| v.iter().copied().filter(|k| in_match(k.tick)).collect()).unwrap_or_default();
        kills.sort_by_key(|k| k.tick);
        let kside = |p: Option<u64>, fallback: Option<u8>| p.and_then(side_of).or(is_side(fallback));
        let kinds: Vec<KillKind> = kills
            .iter()
            .map(|k| match (k.attacker, k.victim) {
                (None, _) | (_, None) => KillKind::World,
                (Some(a), Some(v)) if a == v => KillKind::Suicide,
                _ => {
                    let (sa_, sv) = (kside(k.attacker, k.attacker_team), kside(k.victim, k.victim_team));
                    if sa_.is_some() && sa_ == sv {
                        KillKind::TeamKill
                    } else {
                        KillKind::Kill
                    }
                }
            })
            .collect();
        let mut trade = vec![false; kills.len()];
        let mut traded = vec![false; kills.len()];
        for i in 0..kills.len() {
            if kinds[i] != KillKind::Kill {
                continue;
            }
            let (e, v, t) = (kills[i].attacker, kills[i].victim, kills[i].tick);
            let v_side = kside(v, kills[i].victim_team);
            for j in i + 1..kills.len() {
                if kills[j].tick > t + window {
                    break;
                }
                if kinds[j] == KillKind::Kill && kills[j].victim == e && kside(kills[j].attacker, kills[j].attacker_team) == v_side {
                    traded[i] = true;
                    trade[j] = true;
                    break;
                }
            }
        }
        let opening_idx = (0..kills.len()).find(|&i| kinds[i] == KillKind::Kill && kills[i].tick <= r.end_tick);

        let mut alive: HashMap<u8, HashSet<u64>> = HashMap::new();
        for p in &participants {
            alive.entry(side_of(*p).unwrap()).or_default().insert(*p);
        }
        let damages: Vec<&Damage> = {
            let mut d: Vec<&Damage> =
                dmg_by.get(&rn).map(|v| v.iter().copied().filter(|d| in_match(d.tick)).collect()).unwrap_or_default();
            d.sort_by_key(|d| d.tick);
            d
        };
        let mut clutch: Option<(u8, u64, u8, i32)> = None; // side, player, vs, tick
        let mut feed: Vec<KillEvent> = vec![];
        let origin = r.freeze_end_tick.unwrap_or(r.start_tick);

        for (i, k) in kills.iter().enumerate() {
            let kind = kinds[i];
            let after_end = k.tick > r.end_tick;
            let a_side = kside(k.attacker, k.attacker_team);
            let v_side = kside(k.victim, k.victim_team);
            feed.push(KillEvent {
                tick: k.tick,
                time_secs: (k.tick - origin) as f32 / tick_rate,
                attacker: k.attacker,
                victim: k.victim,
                assister: k.assister,
                weapon: k.weapon.clone(),
                headshot: k.headshot,
                assisted_flash: k.assisted_flash,
                attacker_side: a_side,
                victim_side: v_side,
                kind: kind.as_str(),
                opening: opening_idx == Some(i),
                trade: trade[i],
                traded: traded[i],
                after_round_end: after_end,
            });

            // Victim bookkeeping (every kind of death).
            if let Some(v) = k.victim {
                extra.entry(v).or_default().deaths += 1;
                if let Some(s) = pr.get_mut(&v) {
                    if !s.died {
                        s.died = true;
                        s.death_tick = Some(k.tick);
                    }
                }
                if let Some(vs) = v_side {
                    if let Some(set) = alive.get_mut(&vs) {
                        set.remove(&v);
                    }
                }
            }
            match kind {
                KillKind::Suicide => extra.entry(k.victim.unwrap()).or_default().suicides += 1,
                KillKind::TeamKill => extra.entry(k.attacker.unwrap()).or_default().team_kills += 1,
                _ => {}
            }
            if kind != KillKind::Kill {
                continue;
            }
            let (a, v) = (k.attacker.unwrap(), k.victim.unwrap());
            if let Some(s) = pr.get_mut(&a) {
                s.kills += 1;
                if k.headshot {
                    s.headshot_kills += 1;
                }
                if trade[i] {
                    s.trade_kills += 1;
                }
                if opening_idx == Some(i) {
                    s.opening_kill = true;
                }
            }
            if let Some(s) = pr.get_mut(&v) {
                if traded[i] {
                    s.traded = true;
                }
                if opening_idx == Some(i) {
                    s.opening_death = true;
                }
            }
            if let Some(asst) = k.assister {
                let as_side = side_of(asst);
                if asst != a && asst != v && as_side.is_some() && as_side != v_side {
                    if let Some(s) = pr.get_mut(&asst) {
                        s.assists += 1;
                        if k.assisted_flash {
                            s.flash_assists += 1;
                        }
                    }
                }
            }
            let pair = pairs.entry((a, v)).or_insert_with(|| PairStats { attacker: a, victim: v, ..Default::default() });
            pair.kills += 1;
            if k.headshot {
                pair.headshot_kills += 1;
            }
            if opening_idx == Some(i) {
                pair.opening_kills += 1;
            }

            if after_end {
                continue;
            }
            // Trade opportunities for the victim's teammates still alive.
            if let Some(vs) = v_side {
                let mates: Vec<u64> = alive.get(&vs).map(|s| s.iter().copied().collect()).unwrap_or_default();
                if !mates.is_empty() {
                    extra.entry(v).or_default().traded_opp += 1;
                }
                for m in mates {
                    let x = extra.entry(m).or_default();
                    x.trade_opp += 1;
                    let attempted = damages.iter().any(|d| {
                        d.attacker == Some(m) && d.victim == Some(a) && d.tick >= k.tick && d.tick <= k.tick + window
                    });
                    if attempted {
                        x.trade_att += 1;
                    }
                }
                // Clutch: the victim's side just dropped to one player.
                if clutch.is_none() {
                    let left = alive.get(&vs).map(|s| s.len()).unwrap_or(0);
                    let enemies = alive.get(&other_side(vs)).map(|s| s.len()).unwrap_or(0);
                    if left == 1 && enemies >= 1 {
                        let p = *alive[&vs].iter().next().unwrap();
                        clutch = Some((vs, p, enemies.min(5) as u8, k.tick));
                    }
                }
            }
        }
        // Clutch kills / result.
        if let Some((cs, cp, vs, ct)) = clutch {
            if let Some(s) = pr.get_mut(&cp) {
                s.clutch_vs = Some(vs);
                s.clutch_won = Some(r.winner == Some(cs));
            }
            let n = kills.iter().enumerate().filter(|(i, k)| kinds[*i] == KillKind::Kill && k.attacker == Some(cp) && k.tick > ct).count();
            extra.entry(cp).or_default().clutch_kills += n as u32;
        }

        // ---- damage (effective, capped at remaining health) ----
        let mut health: HashMap<u64, i32> = HashMap::new();
        for d in &damages {
            let Some(v) = d.victim else { continue };
            let prev = *health.get(&v).unwrap_or(&FULL_HEALTH);
            let eff = (prev - d.health_after.max(0)).clamp(0, d.dmg_health.max(0)) as u32;
            health.insert(v, d.health_after.max(0));
            let Some(a) = d.attacker else { continue };
            if a == v {
                continue;
            }
            let (a_side, v_side) = (kside(Some(a), d.attacker_team), kside(Some(v), d.victim_team));
            let is_he = d.weapon == "hegrenade";
            let is_fire = d.weapon == "inferno" || d.weapon == "molotov" || d.weapon == "incgrenade";
            if a_side.is_some() && a_side == v_side {
                if is_he {
                    extra.entry(a).or_default().he_team_damage += eff;
                }
                continue;
            }
            if let Some(s) = pr.get_mut(&a) {
                s.damage += eff;
                if is_he || is_fire {
                    s.utility_damage += eff;
                }
            }
            if is_he {
                extra.entry(a).or_default().he_damage += eff;
            } else if is_fire {
                extra.entry(a).or_default().fire_damage += eff;
            }
            let pair = pairs.entry((a, v)).or_insert_with(|| PairStats { attacker: a, victim: v, ..Default::default() });
            pair.damage += eff;
            pair.hits += 1;
        }

        // ---- grenades thrown ----
        let nades = nades_by.get(&rn).map(|v| v.as_slice()).unwrap_or(&[]);
        for g in nades {
            let Some(p) = g.thrower else { continue };
            let Some(n) = Nade::from_kind(&g.kind) else { continue };
            let x = extra.entry(p).or_default();
            match n {
                Nade::Flash => x.flashes += 1,
                Nade::He => x.he += 1,
                Nade::Smoke => x.smokes += 1,
                Nade::Fire => x.fire += 1,
                Nade::Decoy => x.decoys += 1,
            }
            if let Some(s) = pr.get_mut(&p) {
                s.utility_thrown += 1;
            }
        }

        // ---- blinds (alive victims, excluding self-flashes) ----
        if blind_data_available {
            for b in blinds_by.get(&rn).map(|v| v.as_slice()).unwrap_or(&[]) {
                let (Some(a), Some(v)) = (b.attacker, b.victim) else { continue };
                if a == v || b.duration <= 0.0 {
                    continue;
                }
                if pr.get(&v).and_then(|s| s.death_tick).is_some_and(|t| t <= b.tick) {
                    continue;
                }
                let (a_side, v_side) = (kside(Some(a), b.attacker_team), kside(Some(v), b.victim_team));
                if a_side.is_none() || v_side.is_none() {
                    continue;
                }
                if a_side == v_side {
                    let x = extra.entry(a).or_default();
                    x.team_flashed += 1;
                    x.team_blind += b.duration;
                } else {
                    if let Some(s) = pr.get_mut(&a) {
                        *s.enemies_flashed.get_or_insert(0) += 1;
                    }
                    extra.entry(a).or_default().enemy_blind += b.duration;
                }
            }
        }

        // ---- economy + grenade inventory ----
        // `item_purchase` is re-emitted in batches later in the round (at death / round end);
        // only events inside the buy window are real purchases.
        let buy_end = r.freeze_end_tick.unwrap_or(r.start_tick + (15.0 * tick_rate) as i32) + (BUY_TIME_SECS * tick_rate) as i32;
        let buys: Vec<&Purchase> =
            buys_by.get(&rn).map(|v| v.iter().copied().filter(|b| b.tick <= buy_end).collect()).unwrap_or_default();
        for p in &participants {
            let s = pr.get_mut(p).unwrap();
            let e = econ.get(&(rn, *p));
            let late: i32 = buys
                .iter()
                .filter(|b| b.player == *p && !b.was_sold && r.freeze_end_tick.is_some_and(|f| b.tick > f))
                .map(|b| b.cost.unwrap_or(0))
                .sum();
            s.equip_value = (e.and_then(|e| e.equip_value).unwrap_or(0) + late).max(0) as u32;
            s.balance = e.and_then(|e| e.balance).unwrap_or(0).max(0) as u32;

            let (inv_side, inv) = inventory.entry(*p).or_insert_with(|| (s.side, NadeInventory::default()));
            if *inv_side != s.side {
                inv.clear(); // side swap: inventories reset
                *inv_side = s.side;
            }
            let mut mine: Vec<&Purchase> = buys.iter().copied().filter(|b| b.player == *p).collect();
            mine.sort_by_key(|b| b.tick);
            for b in mine {
                if let Some((n, default_cost)) = Nade::from_item(&b.item) {
                    if b.was_sold {
                        inv.sell(n);
                    } else {
                        inv.buy(n, b.cost.map(|c| c.max(0) as u32).unwrap_or(default_cost));
                    }
                }
            }
            for g in nades.iter().filter(|g| g.thrower == Some(*p)) {
                if let Some(n) = Nade::from_kind(&g.kind) {
                    inv.throw(n);
                }
            }
            if s.died {
                s.unused_utility_value = inv.value();
                inv.clear();
            }
        }

        // ---- Swing (needs equipment values) ----
        let round_bombs: Vec<&BombEvent> = bombs_by.get(&rn).map(|v| v.to_vec()).unwrap_or_default();
        let out = swing::simulate_round(
            model,
            RoundInput {
                round: rn,
                tick_rate,
                origin,
                end_tick: r.end_tick,
                winner: r.winner,
                participants: participants
                    .iter()
                    .map(|p| Participant {
                        steamid: *p,
                        side: pr[p].side,
                        equip: pr[p].equip_value,
                        kit: econ.get(&(rn, *p)).and_then(|e| e.has_defuser).unwrap_or(false),
                    })
                    .collect(),
                kills: kills
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let dk = match kinds[i] {
                            KillKind::Kill => DeathKind::Kill,
                            KillKind::TeamKill => DeathKind::TeamKill,
                            KillKind::Suicide => DeathKind::Suicide,
                            KillKind::World => DeathKind::World,
                        };
                        (*k, dk, traded[i])
                    })
                    .collect(),
                damages: damages.clone(),
                bombs: round_bombs,
            },
        );
        for (p, v) in &out.swing {
            if let Some(s) = pr.get_mut(p) {
                s.swing += *v;
            }
        }
        win_probability.push(out.curve);
        swing_samples.extend(out.samples);

        // ---- KAST ----
        for s in pr.values_mut() {
            s.kast = s.kills > 0 || s.assists > 0 || !s.died || s.traded;
        }

        // ---- MVP (approximation of the engine rule; `round_mvp` is not parsed) ----
        let bombs = bombs_by.get(&rn).map(|v| v.as_slice()).unwrap_or(&[]);
        let reason = r.reason.as_deref().unwrap_or("");
        let mvp = match reason {
            "bomb_exploded" | "target_bombed" => bombs.iter().rev().find(|b| b.kind == "planted").and_then(|b| b.player),
            "bomb_defused" => bombs.iter().rev().find(|b| b.kind == "defused").and_then(|b| b.player),
            _ => None,
        }
        .or_else(|| {
            let w = r.winner?;
            pr.values()
                .filter(|s| s.side == w && s.kills > 0)
                .max_by(|x, y| (x.kills, x.damage, std::cmp::Reverse(x.steamid)).cmp(&(y.kills, y.damage, std::cmp::Reverse(y.steamid))))
                .map(|s| s.steamid)
        });
        if let Some(m) = mvp {
            if let Some(s) = pr.get_mut(&m) {
                s.mvp = true;
            }
        }

        // ---- team rounds / score ----
        let winner_team = r.winner.and_then(|w| is_side(Some(w))).map(|w| team_of_side(rn, w));
        match winner_team {
            Some(Team::A) => score_a += 1,
            Some(Team::B) => score_b += 1,
            None => {}
        }
        for t in [Team::A, Team::B] {
            let t_side = if t == Team::A { sa } else { other_side(sa) };
            let members: Vec<&PlayerRoundStats> = pr.values().filter(|s| s.side == t_side).collect();
            let equip: u32 = members.iter().map(|s| s.equip_value).sum();
            team_rounds.push(TeamRoundStats {
                round: rn,
                team: t,
                side: t_side,
                won: winner_team == Some(t),
                score: if t == Team::A { score_a } else { score_b },
                equip_value: equip,
                balance: members.iter().map(|s| s.balance).sum(),
                buy_type: BuyType::classify(pistol.contains(&rn), t_side, equip, members.len() as u32),
                kills: members.iter().map(|s| s.kills).sum(),
                survivors: members.iter().filter(|s| s.death_tick.map_or(true, |d| d > r.end_tick)).count() as u32,
            });
        }

        let planted = bombs.iter().rev().find(|b| b.kind == "planted");
        summaries.push(RoundSummary {
            number: rn,
            winner_side: r.winner,
            winner_team,
            reason: r.reason.clone(),
            side_a: sa,
            score_a,
            score_b,
            start_tick: r.start_tick,
            freeze_end_tick: r.freeze_end_tick,
            end_tick: r.end_tick,
            duration_secs: (r.end_tick - origin).max(0) as f32 / tick_rate,
            bomb_planted: planted.is_some(),
            bomb_site: planted.and_then(|b| b.site),
            mvp,
            kills: feed,
        });

        let mut rows: Vec<PlayerRoundStats> = pr.into_values().collect();
        rows.sort_by_key(|s| roster.iter().position(|p| *p == s.steamid));
        player_rounds.extend(rows);
    }

    let winner = match score_a.cmp(&score_b) {
        std::cmp::Ordering::Greater => Some(Team::A),
        std::cmp::Ordering::Less => Some(Team::B),
        std::cmp::Ordering::Equal => None,
    };

    // ---- per-player totals ----
    let mut players: Vec<PlayerMatchStats> = vec![];
    for p in &roster {
        let rows: Vec<&PlayerRoundStats> = player_rounds.iter().filter(|s| s.steamid == *p).collect();
        if rows.is_empty() {
            continue;
        }
        let x = extra.remove(p).unwrap_or_default();
        let t = team[p];
        let mut m = PlayerMatchStats {
            steamid: *p,
            name: names.get(p).map(|s| s.to_string()).unwrap_or_default(),
            team: Some(t),
            start_side: rows.first().map(|s| s.side),
            matches: 1,
            wins: (winner == Some(t)) as u32,
            losses: (winner == Some(t.other())) as u32,
            draws: winner.is_none() as u32,
            rounds_played: rows.len() as u32,
            team_kills: x.team_kills,
            deaths: x.deaths,
            suicides: x.suicides,
            trade_kill_opportunities: x.trade_opp,
            trade_kill_attempts: x.trade_att,
            traded_death_opportunities: x.traded_opp,
            clutch_kills: x.clutch_kills,
            flashes_thrown: x.flashes,
            he_thrown: x.he,
            smokes_thrown: x.smokes,
            molotovs_thrown: x.fire,
            decoys_thrown: x.decoys,
            blind_flashes: if blind_data_available { x.flashes } else { 0 },
            enemies_flashed: blind_data_available.then_some(0),
            enemy_blind_seconds: blind_data_available.then_some(x.enemy_blind),
            teammates_flashed: blind_data_available.then_some(x.team_flashed),
            teammate_blind_seconds: blind_data_available.then_some(x.team_blind),
            he_damage: x.he_damage,
            he_team_damage: x.he_team_damage,
            fire_damage: x.fire_damage,
            ..Default::default()
        };
        for s in rows {
            if s.side == TEAM_T {
                m.rounds_t += 1;
            } else {
                m.rounds_ct += 1;
            }
            m.kills += s.kills;
            m.assists += s.assists;
            m.flash_assists += s.flash_assists;
            m.headshot_kills += s.headshot_kills;
            m.damage += s.damage;
            m.kast_rounds += s.kast as u32;
            m.rounds_survived += (!s.died) as u32;
            m.mvps += s.mvp as u32;
            m.multi_kills[(s.kills as usize).min(5)] += 1;
            m.trade_kills += s.trade_kills;
            m.traded_deaths += s.traded as u32;
            let (ok, od) = if s.side == TEAM_T {
                (&mut m.opening_kills_t, &mut m.opening_deaths_t)
            } else {
                (&mut m.opening_kills_ct, &mut m.opening_deaths_ct)
            };
            *ok += s.opening_kill as u32;
            *od += s.opening_death as u32;
            if let Some(vs) = s.clutch_vs {
                m.clutch_attempts[vs as usize] += 1;
                if s.clutch_won == Some(true) {
                    m.clutch_wins[vs as usize] += 1;
                }
            }
            if let (Some(total), Some(n)) = (m.enemies_flashed.as_mut(), s.enemies_flashed) {
                *total += n;
            }
            m.unused_utility_value += s.unused_utility_value;
            m.swing_total += s.swing;
        }
        m.finalize();
        players.push(m);
    }

    MatchStats {
        version: STATS_VERSION,
        blind_data_available,
        score_a,
        score_b,
        winner,
        players,
        player_rounds,
        team_rounds,
        rounds: summaries,
        pairs: pairs.into_values().collect(),
        win_probability,
        swing_samples: swing::merge_samples(swing_samples),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use demo_parse::{Damage, Economy, Grenade, Kill, Player, Round};

    const A1: u64 = 1;
    const A2: u64 = 2;
    const B1: u64 = 11;
    const B2: u64 = 12;
    const B3: u64 = 13;

    fn player(sid: u64, side: u8) -> Player {
        Player { steamid: sid, name: format!("p{sid}"), team_start: Some(side), team_end: None, rank: None, rank_type: None }
    }
    fn round(n: u32, winner: u8) -> Round {
        let s = (n as i32) * 10_000;
        Round { number: n, start_tick: s, freeze_end_tick: Some(s + 100), end_tick: s + 5000, winner: Some(winner), reason: None }
    }
    fn econ(round: u32, sid: u64, side: u8) -> Economy {
        Economy {
            round,
            player: sid,
            team: Some(side),
            balance: Some(800),
            equip_value: Some(4500),
            round_start_equip_value: None,
            cash_spent_this_round: None,
            armor: None,
            has_helmet: None,
            has_defuser: None,
        }
    }
    fn kill(round: u32, tick: i32, a: Option<u64>, v: u64, sides: (u8, u8)) -> Kill {
        Kill {
            round,
            tick,
            attacker: a,
            victim: Some(v),
            assister: None,
            weapon: if a == Some(v) { "world".into() } else { "ak47".into() },
            headshot: false,
            penetrated: false,
            noscope: false,
            thrusmoke: false,
            attacker_blind: false,
            attacker_in_air: false,
            assisted_flash: false,
            distance: None,
            attacker_pos: None,
            victim_pos: None,
            attacker_team: a.map(|_| sides.0),
            victim_team: Some(sides.1),
        }
    }

    struct Fixture {
        players: Vec<Player>,
        rounds: Vec<Round>,
        kills: Vec<Kill>,
        econ: Vec<Economy>,
        grenades: Vec<Grenade>,
        damages: Vec<Damage>,
    }

    impl Fixture {
        /// Team A = {A1, A2} on CT in round 1, swapping to T in round 2. Team B = {B1, B2, B3}.
        fn new(rounds: Vec<Round>) -> Self {
            let mut econ = vec![];
            for r in &rounds {
                let (sa, sb) = if r.number == 1 { (TEAM_CT, TEAM_T) } else { (TEAM_T, TEAM_CT) };
                for p in [A1, A2] {
                    econ.push(super::tests::econ(r.number, p, sa));
                }
                for p in [B1, B2, B3] {
                    econ.push(super::tests::econ(r.number, p, sb));
                }
            }
            Self {
                players: vec![player(A1, TEAM_CT), player(A2, TEAM_CT), player(B1, TEAM_T), player(B2, TEAM_T), player(B3, TEAM_T)],
                rounds,
                kills: vec![],
                econ,
                grenades: vec![],
                damages: vec![],
            }
        }
        fn run(&self) -> MatchStats {
            compute(MatchEvents {
                tick_rate: 64.0,
                players: &self.players,
                rounds: &self.rounds,
                kills: &self.kills,
                damages: &self.damages,
                grenades: &self.grenades,
                blinds: &[],
                bombs: &[],
                purchases: &[],
                economies: &self.econ,
            })
        }
    }

    fn get(s: &MatchStats, sid: u64) -> &PlayerMatchStats {
        s.players.iter().find(|p| p.steamid == sid).unwrap()
    }

    #[test]
    fn trades_openings_and_clutch() {
        let mut f = Fixture::new(vec![round(1, TEAM_CT)]);
        let t0 = 10_200;
        let ct_t = (TEAM_CT, TEAM_T);
        let t_ct = (TEAM_T, TEAM_CT);
        f.kills = vec![
            kill(1, t0, Some(B1), A1, t_ct),            // opening: B1 (T) kills A1 (CT)
            kill(1, t0 + 64 * 3, Some(A2), B1, ct_t),   // A2 trades A1 within 5 s
            kill(1, t0 + 64 * 20, Some(A2), B2, ct_t),  // not a trade
            kill(1, t0 + 64 * 21, Some(A2), B3, ct_t),  // A2 wins
        ];
        let s = f.run();
        let (a1, a2, b1) = (get(&s, A1), get(&s, A2), get(&s, B1));
        assert_eq!((b1.opening_kills_t, a1.opening_deaths_ct), (1, 1));
        assert_eq!(a2.trade_kills, 1);
        assert_eq!(a1.traded_deaths, 1);
        assert_eq!(a1.kast_rounds, 1, "traded death counts for KAST");
        assert_eq!(a2.trade_kill_opportunities, 1);
        // A2 became the last CT alive against 3 Ts and won.
        assert_eq!((a2.clutch_attempts[3], a2.clutch_wins[3], a2.clutch_kills), (1, 1, 3));
        assert_eq!(a2.multi_kills[3], 1);
        assert_eq!(s.rounds[0].kills.iter().filter(|k| k.trade).count(), 1);
        assert_eq!(s.rounds[0].mvp, Some(A2));
    }

    #[test]
    fn swing_attribution_shares() {
        use crate::swing::{DAMAGE_ASSIST_MAX_SHARE, FLASH_ASSIST_SHARE};
        let mut f = Fixture::new(vec![round(1, TEAM_T)]);
        let t0 = 10_100 + 64 * 10;
        let dmg = |tick: i32, a: u64, v: u64, after: i32| Damage {
            round: 1,
            tick,
            attacker: Some(a),
            victim: Some(v),
            weapon: "ak47".into(),
            dmg_health: 100 - after,
            dmg_armor: 0,
            health_after: after,
            armor_after: 0,
            hitgroup: None,
            attacker_team: Some(TEAM_T),
            victim_team: Some(TEAM_CT),
        };
        // B1 hits A1 for 50, then B2 kills A1 with a flash assist from B3; A2 survives untraded.
        f.damages = vec![dmg(t0, B1, A1, 50), dmg(t0 + 64, B2, A1, 0)];
        let mut k = kill(1, t0 + 64, Some(B2), A1, (TEAM_T, TEAM_CT));
        k.assister = Some(B3);
        k.assisted_flash = true;
        f.kills = vec![k];
        let s = f.run();
        let sw = |p: u64| s.player_rounds.iter().find(|r| r.steamid == p).unwrap().swing as f64;
        let kill_pt = s.win_probability[0].points.iter().find(|p| p.kind == "kill").unwrap();
        let g = 100.0 * (kill_pt.p_after - kill_pt.p_before) as f64;
        assert!(g > 0.0, "a T kill raises the T win probability");
        // B3: exactly the flash share of the kill (plus its share of the time drift / round end).
        let even = sw(B3) - g * FLASH_ASSIST_SHARE;
        let pool = DAMAGE_ASSIST_MAX_SHARE * 0.5;
        // Every T shares the same "even" part (drift + round end), so subtract it.
        assert!((sw(B2) - even - g * (1.0 - FLASH_ASSIST_SHARE - pool)).abs() < 0.01, "killer {}", sw(B2));
        let dmg_pt = s.win_probability[0].points.iter().find(|p| p.kind == "damage").unwrap();
        let hit = 100.0 * (dmg_pt.p_after - dmg_pt.p_before) as f64;
        assert!((sw(B1) - even - hit - g * pool).abs() < 0.01, "damage assister {}", sw(B1));
        assert!(sw(A1) < sw(A2), "the victim loses more than the survivor");
        // Zero-sum.
        let total: f64 = [A1, A2, B1, B2, B3].iter().map(|p| sw(*p)).sum();
        assert!(total.abs() < 1e-3);
    }

    #[test]
    fn suicides_and_team_kills_are_not_kills() {
        let mut f = Fixture::new(vec![round(1, TEAM_T)]);
        f.kills = vec![
            kill(1, 10_200, Some(A1), A1, (TEAM_CT, TEAM_CT)), // world suicide
            kill(1, 10_300, Some(B1), B2, (TEAM_T, TEAM_T)),   // team kill
            kill(1, 10_400, None, A2, (0, TEAM_CT)),           // bomb / fall damage
        ];
        let s = f.run();
        let (a1, b1, a2) = (get(&s, A1), get(&s, B1), get(&s, A2));
        assert_eq!((a1.kills, a1.deaths, a1.suicides), (0, 1, 1));
        assert_eq!((b1.kills, b1.team_kills), (0, 1));
        assert_eq!(a2.deaths, 1);
        // No enemy kill → no opening duel.
        assert_eq!(s.players.iter().map(|p| p.opening_kills_t + p.opening_kills_ct).sum::<u32>(), 0);
        assert_eq!(s.rounds[0].kills.iter().map(|k| k.kind).collect::<Vec<_>>(), ["suicide", "team_kill", "world"]);
    }

    #[test]
    fn score_follows_teams_across_side_swap() {
        // Round 1: CT (team A) wins. Round 2: sides swap, CT (now team B) wins.
        let f = Fixture::new(vec![round(1, TEAM_CT), round(2, TEAM_CT)]);
        let s = f.run();
        assert_eq!((s.score_a, s.score_b), (1, 1));
        assert_eq!(s.rounds[1].side_a, TEAM_T);
        assert_eq!(s.rounds[1].winner_team, Some(Team::B));
        assert_eq!(get(&s, A1).team, Some(Team::A));
        assert_eq!((get(&s, A1).rounds_ct, get(&s, A1).rounds_t), (1, 1));
        // Round 2 follows the first side swap → pistol round.
        assert!(s.team_rounds.iter().all(|t| t.buy_type == BuyType::Pistol));
        assert_eq!(s.winner, None);
    }

    #[test]
    fn missing_blind_events_mean_unavailable() {
        let mut f = Fixture::new(vec![round(1, TEAM_T)]);
        f.grenades = vec![Grenade { round: 1, tick: 10_300, thrower: Some(A1), kind: "flashbang".into(), entity_id: None, pos: None }];
        let s = f.run();
        assert!(!s.blind_data_available);
        let a1 = get(&s, A1);
        assert_eq!(a1.flashes_thrown, 1);
        assert_eq!(a1.enemies_flashed, None);
        assert_eq!(a1.derived.enemies_flashed_per_flash, None);

        // No flashes at all: blind metrics are a real zero.
        let f = Fixture::new(vec![round(1, TEAM_T)]);
        let s = f.run();
        assert!(s.blind_data_available);
        assert_eq!(get(&s, A1).enemies_flashed, Some(0));
    }

    #[test]
    fn aggregate_adds_counts_and_recomputes_rates() {
        let f = Fixture::new(vec![round(1, TEAM_CT)]);
        let s = f.run();
        let mut agg = get(&s, A1).clone();
        agg.add(get(&s, A1));
        agg.finalize();
        assert_eq!(agg.matches, 2);
        assert_eq!(agg.rounds_played, 2);
        assert_eq!(agg.derived.kast, get(&s, A1).derived.kast);
    }
}
