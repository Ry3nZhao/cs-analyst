//! 对位数据：每场算出 h2h_player_results（每人胜负）和 h2h_match（每对选手），
//! h2h_stats 由 h2h_match 上的触发器维护。
//!
//! 口径：
//! - 只看 2 / 3 阵营的选手；同队 = 开局阵营相同（COALESCE(team_start, team_end)）。
//! - 胜负按回合推：每回合用 economies / kills / damages 里的阵营判断两队各在哪边，赢家阵营那队得 1 分，已处理换边。
//! - 击杀、伤害、对枪都排除自杀（attacker = victim 或没有 attacker）和同队伤害。
//! - 伤害按实际扣血计：每回合每人从 100 血开始，min(dmg_health, 剩余血量)。
//! - 对枪：每回合两人第一次交火（互相造成伤害 / 击杀），事件间隔不超过 DUEL_GAP_TICKS 的连成一串；
//!   串内谁先杀了对方谁赢，没有击杀算平。
//! - 开局对枪：每回合第一个有效击杀。

use crate::database::Result;
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, HashMap, HashSet};

/// 3 秒 @ 64 tick
pub const DUEL_GAP_TICKS: i64 = 192;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PairFacts {
    pub same_team: bool,
    pub result: i64,
    pub kills: i64,
    pub deaths: i64,
    pub damage: i64,
    pub damage_taken: i64,
    pub duels: i64,
    pub duels_won: i64,
    pub duels_lost: i64,
    pub opening_wins: i64,
    pub opening_losses: i64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PlayerResult {
    pub team: i64,
    pub rounds_won: i64,
    pub rounds_lost: i64,
    pub result: i64,
}

#[derive(Debug, Default)]
pub struct MatchFacts {
    pub players: BTreeMap<i64, PlayerResult>,
    /// (a, b) 两个方向都有
    pub pairs: BTreeMap<(i64, i64), PairFacts>,
}

struct Event {
    round: i64,
    tick: i64,
    attacker: i64,
    victim: i64,
    /// 实际扣血；击杀为 None
    damage: Option<i64>,
}

fn load_vec<T>(c: &Connection, sql: &str, m: &str, f: impl FnMut(&rusqlite::Row) -> rusqlite::Result<T>) -> Result<Vec<T>> {
    let mut st = c.prepare(sql)?;
    let rows = st.query_map(params![m], f)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn compute_match(c: &Connection, m: &str) -> Result<MatchFacts> {
    let mut out = MatchFacts::default();
    // 选手和开局阵营
    let team: HashMap<i64, i64> = load_vec(
        c,
        "SELECT steamid, COALESCE(team_start, team_end) FROM match_players WHERE match_id = ? AND steamid > 0",
        m,
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)),
    )?
    .into_iter()
    .filter_map(|(s, t)| t.filter(|t| *t == 2 || *t == 3).map(|t| (s, t)))
    .collect();
    if team.is_empty() {
        return Ok(out);
    }

    // 每回合两队各在哪边：投票（economies 最可靠，击杀 / 伤害里的阵营补充）
    let mut votes: HashMap<i64, [i64; 2]> = HashMap::new(); // round -> [开局 T 那队此回合是 T 的票, 是 CT 的票]
    let mut vote = |round: i64, player: Option<i64>, side: Option<i64>| {
        let (Some(p), Some(side)) = (player, side) else { return };
        let Some(&start) = team.get(&p) else { return };
        if side != 2 && side != 3 {
            return;
        }
        // 开局 T 那队这回合所在阵营
        let t_team_side = if start == 2 { side } else { 5 - side };
        let v = votes.entry(round).or_default();
        v[(t_team_side - 2) as usize] += 1;
    };
    for (round, player, side) in load_vec(c, "SELECT round, player, team FROM economies WHERE match_id = ?", m, |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<i64>>(2)?))
    })? {
        vote(round, player, side);
    }
    for sql in [
        "SELECT round, attacker, attacker_team FROM kills WHERE match_id = ?",
        "SELECT round, victim, victim_team FROM kills WHERE match_id = ?",
    ] {
        for (round, player, side) in load_vec(c, sql, m, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))? {
            vote(round, player, side);
        }
    }

    let rounds: Vec<(i64, Option<i64>)> =
        load_vec(c, "SELECT number, winner FROM rounds WHERE match_id = ? ORDER BY number", m, |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut score = [0i64; 2]; // [开局 T 队, 开局 CT 队]
    for (number, winner) in &rounds {
        let Some(w) = winner.filter(|w| *w == 2 || *w == 3) else { continue };
        let Some(v) = votes.get(number) else { continue };
        if v[0] == v[1] {
            continue;
        }
        let t_team_side = if v[0] > v[1] { 2 } else { 3 };
        if w == t_team_side {
            score[0] += 1;
        } else {
            score[1] += 1;
        }
    }
    let result_of = |t: i64| -> i64 {
        let (mine, theirs) = if t == 2 { (score[0], score[1]) } else { (score[1], score[0]) };
        (mine - theirs).signum()
    };
    for (&s, &t) in &team {
        let (won, lost) = if t == 2 { (score[0], score[1]) } else { (score[1], score[0]) };
        out.players.insert(s, PlayerResult { team: t, rounds_won: won, rounds_lost: lost, result: result_of(t) });
    }

    // 所有组合
    let ids: Vec<i64> = team.keys().copied().collect();
    for &a in &ids {
        for &b in &ids {
            if a != b {
                out.pairs.insert(
                    (a, b),
                    PairFacts { same_team: team[&a] == team[&b], result: result_of(team[&a]), ..Default::default() },
                );
            }
        }
    }

    // 事件：有效（非自杀、非同队、双方都在场上名单里）的伤害和击杀
    let valid = |a: Option<i64>, v: Option<i64>, at: Option<i64>, vt: Option<i64>| -> Option<(i64, i64)> {
        let (a, v) = (a?, v?);
        if a == v || !team.contains_key(&a) || !team.contains_key(&v) {
            return None;
        }
        // 以事件发生时的阵营为准，缺失时退回开局阵营
        if let (Some(at), Some(vt)) = (at, vt) {
            if at == vt {
                return None;
            }
        } else if team[&a] == team[&v] {
            return None;
        }
        Some((a, v))
    };

    let mut events: Vec<Event> = vec![];
    // 实际扣血：每回合每人 100 起
    let mut hp: HashMap<(i64, i64), i64> = HashMap::new();
    let dmg_rows: Vec<(i64, i64, Option<i64>, Option<i64>, i64, i64, Option<i64>, Option<i64>)> = load_vec(
        c,
        "SELECT round, tick, attacker, victim, dmg_health, health_after, attacker_team, victim_team
         FROM damages WHERE match_id = ? ORDER BY round, tick, rowid",
        m,
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
    )?;
    for (round, tick, a, v, dmg, after, at, vt) in dmg_rows {
        let real = match v {
            Some(v) => {
                let cur = hp.entry((round, v)).or_insert(100);
                let real = dmg.max(0).min(*cur);
                *cur = after.max(0);
                real
            }
            None => 0,
        };
        if let Some((a, v)) = valid(a, v, at, vt) {
            events.push(Event { round, tick, attacker: a, victim: v, damage: Some(real) });
        }
    }
    let kill_rows: Vec<(i64, i64, Option<i64>, Option<i64>, Option<i64>, Option<i64>)> = load_vec(
        c,
        "SELECT round, tick, attacker, victim, attacker_team, victim_team FROM kills WHERE match_id = ? ORDER BY round, tick, rowid",
        m,
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
    )?;
    // 每回合死亡时间（判断对枪串中途被第三人打死）
    let mut death_tick: HashMap<(i64, i64), i64> = HashMap::new();
    let mut opening_done: HashSet<i64> = HashSet::new();
    for &(round, tick, a, v, at, vt) in &kill_rows {
        if let Some(v) = v {
            death_tick.entry((round, v)).or_insert(tick);
        }
        if let Some((a, v)) = valid(a, v, at, vt) {
            if opening_done.insert(round) {
                out.pairs.get_mut(&(a, v)).unwrap().opening_wins += 1;
                out.pairs.get_mut(&(v, a)).unwrap().opening_losses += 1;
            }
            events.push(Event { round, tick, attacker: a, victim: v, damage: None });
        }
    }

    // 累计击杀 / 伤害，并按 (回合, 无序对) 分组找第一次交火
    // 同 tick 时伤害排在击杀前
    events.sort_by_key(|e| (e.round, e.tick, e.damage.is_none()));
    let mut by_pair: BTreeMap<(i64, i64, i64), Vec<&Event>> = BTreeMap::new();
    for e in &events {
        match e.damage {
            Some(d) => {
                out.pairs.get_mut(&(e.attacker, e.victim)).unwrap().damage += d;
                out.pairs.get_mut(&(e.victim, e.attacker)).unwrap().damage_taken += d;
            }
            None => {
                out.pairs.get_mut(&(e.attacker, e.victim)).unwrap().kills += 1;
                out.pairs.get_mut(&(e.victim, e.attacker)).unwrap().deaths += 1;
            }
        }
        let (lo, hi) = if e.attacker < e.victim { (e.attacker, e.victim) } else { (e.victim, e.attacker) };
        by_pair.entry((e.round, lo, hi)).or_default().push(e);
    }
    for ((round, lo, hi), evs) in by_pair {
        let mut last = evs[0].tick;
        let mut winner: Option<i64> = None;
        // 任意一人已经死了（被第三人击杀；之后只可能是遗留的燃烧弹等伤害）则不再延续
        let died = |p: i64| death_tick.get(&(round, p)).copied().unwrap_or(i64::MAX);
        let cutoff = died(lo).min(died(hi));
        if evs[0].tick > cutoff {
            continue;
        }
        for e in &evs {
            if e.tick - last > DUEL_GAP_TICKS || e.tick > cutoff {
                break;
            }
            last = e.tick;
            if e.damage.is_none() {
                winner = Some(e.attacker);
                break;
            }
        }
        for (x, y) in [(lo, hi), (hi, lo)] {
            let p = out.pairs.get_mut(&(x, y)).unwrap();
            p.duels += 1;
            match winner {
                Some(w) if w == x => p.duels_won += 1,
                Some(_) => p.duels_lost += 1,
                None => {}
            }
        }
    }
    Ok(out)
}

/// 重算一场的对位数据（幂等：先删后插，触发器同步 h2h_stats）。调用方负责事务。
pub fn recompute_match(c: &Connection, m: &str) -> Result<()> {
    c.execute("DELETE FROM h2h_match WHERE match_id = ?", params![m])?;
    c.execute("DELETE FROM h2h_player_results WHERE match_id = ?", params![m])?;
    let facts = compute_match(c, m)?;
    let mut st = c.prepare(
        "INSERT INTO h2h_player_results (match_id, steamid, team, rounds_won, rounds_lost, result) VALUES (?, ?, ?, ?, ?, ?)",
    )?;
    for (s, p) in &facts.players {
        st.execute(params![m, s, p.team, p.rounds_won, p.rounds_lost, p.result])?;
    }
    let mut st = c.prepare(
        "INSERT INTO h2h_match (match_id, a, b, same_team, result, kills, deaths, damage, damage_taken,
           duels, duels_won, duels_lost, opening_wins, opening_losses)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for ((a, b), p) in &facts.pairs {
        st.execute(params![
            m, a, b, p.same_team, p.result, p.kills, p.deaths, p.damage, p.damage_taken, p.duels, p.duels_won,
            p.duels_lost, p.opening_wins, p.opening_losses
        ])?;
    }
    Ok(())
}

/// 全量重建：清空三张表，对所有已解析的对局重算。返回处理的场数。
pub fn rebuild_all(c: &Connection) -> Result<usize> {
    let tx = c.unchecked_transaction()?;
    tx.execute_batch("DELETE FROM h2h_match; DELETE FROM h2h_player_results; DELETE FROM h2h_stats;")?;
    let ids: Vec<String> = {
        let mut st = tx.prepare("SELECT id FROM matches WHERE analysis_status = 'done' ORDER BY id")?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for id in &ids {
        recompute_match(&tx, id)?;
    }
    tx.commit()?;
    Ok(ids.len())
}
