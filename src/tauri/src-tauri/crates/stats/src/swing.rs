//! Swing: a round win-probability model plus per-event attribution of probability changes
//! (in the spirit of HLTV Rating 3.0's "Round Swing"). See `docs/stats.md` → 「Swing」.
//!
//! - [`WinModel`] predicts the T side's chance to win the round from a [`RoundState`].
//!   Cold start uses a built-in prior table (derivation below); once the local library has
//!   [`LOCAL_MIN_ROUNDS`] rounds, each (T alive, CT alive, planted) bucket is smoothed towards the
//!   locally observed T win rate. Local data only, nothing is downloaded.
//! - [`simulate_round`] replays a round's events (damage, kills, plant, defuse, explosion, round
//!   end), evaluates the model before and after each one and credits the difference to players.
//!   Credits are zero-sum per event: the T side's players share `+Δ`, the CT side's `−Δ`. So over a
//!   round the T side's total is exactly `p_end − p_start` and the winners' total is positive.
//!
//! All swing values exposed to callers are in **percentage points** of win probability.

use crate::{opt_steamid_str, TEAM_CT, TEAM_T};
use demo_parse::{BombEvent, Damage, Kill};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---------------------------------------------------------------------------------------------
// Model constants
// ---------------------------------------------------------------------------------------------

/// Competitive round time after freeze end (1:55).
pub const ROUND_TIME_SECS: f32 = 115.0;
/// C4 timer (`mp_c4timer` 40).
pub const BOMB_TIME_SECS: f32 = 40.0;
pub const DEFUSE_SECS: f32 = 10.0;
pub const DEFUSE_KIT_SECS: f32 = 5.0;
/// Before the plant, T needs time to reach a site and plant (3.2 s). With less than this much
/// round time left the T win probability is scaled down linearly to 0 at time-out.
pub const PLANT_TIME_PRESSURE_SECS: f32 = 20.0;
/// After the plant (with T alive) the CT side needs to retake *and* defuse: its win probability is
/// scaled down linearly once less than `defuse time + this` remains on the bomb.
pub const RETAKE_MARGIN_SECS: f32 = 10.0;

/// Logit offset for T before the plant. Makes 5v5 at freeze end ≈ 47 % T, roughly the T round win
/// rate in CS2 matchmaking and pro play (slightly CT-sided on most maps).
pub const T_BIAS_PRE_PLANT: f64 = -0.12;
/// Logit offset for T after the plant. Makes a 5v5 post-plant ≈ 70 % T: the planting side keeps
/// the site, the CT side must come to it on a timer.
pub const T_BIAS_POST_PLANT: f64 = 0.85;
/// Logit per $1000 difference in average equipment value of the *alive* players (T − CT).
/// Full buy vs eco (≈ $4300 vs $700) gives ≈ −1.8 logit → the eco side wins ≈ 14 %, in line with
/// the usual 10–20 % anti-eco loss rate.
pub const EQUIP_LOGIT_PER_1000: f64 = 0.5;
pub const EQUIP_LOGIT_MAX: f64 = 2.5;
/// Logit per 100 HP difference in average health of the alive players (T − CT). One player of
/// five hit for 100 → avg −20 HP → −0.2 logit ≈ −5 pp at 50 %.
pub const HP_LOGIT_PER_100: f64 = 1.0;

/// Local smoothing kicks in once the library holds this many rounds.
pub const LOCAL_MIN_ROUNDS: u32 = 200;
/// Weight of the prior in a bucket, in rounds: `p = (K·prior + t_wins) / (K + rounds)`.
pub const LOCAL_PSEUDO_ROUNDS: f64 = 100.0;

// ---------------------------------------------------------------------------------------------
// Attribution constants (shares of one event's Δ within one side)
// ---------------------------------------------------------------------------------------------

/// Kill: share of the killer side's gain given to the flash assister (if any).
pub const FLASH_ASSIST_SHARE: f64 = 0.15;
/// Kill: share given to teammates who damaged the victim earlier in the round, at most this much,
/// scaled by `their damage / 100` and split by damage.
pub const DAMAGE_ASSIST_MAX_SHARE: f64 = 0.35;
/// Kill: share of the victim side's loss taken by the victim; the rest is split evenly among the
/// victim's alive teammates (all of it goes to the victim when nobody is left).
pub const DEATH_SHARE: f64 = 0.7;
/// Same, when the victim is traded within [`crate::TRADE_WINDOW_SECS`].
pub const DEATH_SHARE_TRADED: f64 = 0.35;

// ---------------------------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------------------------

/// Everything the model looks at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundState {
    pub t_alive: u8,
    pub ct_alive: u8,
    /// Average health of the alive players (0–100).
    pub t_hp: f32,
    pub ct_hp: f32,
    /// Average equipment value of the alive players.
    pub t_equip: f32,
    pub ct_equip: f32,
    pub planted: bool,
    /// Round time left before the plant, bomb time left after it.
    pub secs_left: f32,
    /// Some alive CT has a defuse kit.
    pub ct_kit: bool,
}

/// One state bucket observed in local rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketSample {
    pub t_alive: u8,
    pub ct_alive: u8,
    pub planted: bool,
    /// Rounds that passed through this bucket.
    pub rounds: u32,
    /// ...of which the T side won.
    pub t_wins: u32,
}

/// Prior T win probability for `t` vs `ct` alive players, before side / equipment / time effects.
///
/// Derivation: the rest of the round is a sequence of duels; each duel removes one player, and the
/// side with `a` players wins it with probability `a / (a + b)` (a crude Lanchester-style "numbers
/// advantage"). `P(a, b) = a/(a+b)·P(a, b−1) + b/(a+b)·P(a−1, b)`, `P(a, 0) = 1`, `P(0, b) = 0`.
/// This gives 5v4 ≈ 72 %, 4v3 ≈ 74 %, 2v1 ≈ 83 %, 5v5 = 50 %, close to the often-quoted
/// "the team with the opening kill wins ~70–75 % of rounds".
pub fn duel_prior(t: u8, ct: u8) -> f64 {
    let (t, ct) = (t.min(5) as usize, ct.min(5) as usize);
    let mut p = [[0.0f64; 6]; 6];
    for a in 0..=5 {
        for b in 0..=5 {
            p[a][b] = match (a, b) {
                (0, _) => 0.0,
                (_, 0) => 1.0,
                _ => {
                    let (af, bf) = (a as f64, b as f64);
                    af / (af + bf) * p[a][b - 1] + bf / (af + bf) * p[a - 1][b]
                }
            };
        }
    }
    p[t][ct]
}

fn logit(p: f64) -> f64 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Built-in prior for one (T alive, CT alive, planted) bucket: [`duel_prior`] plus the side bias.
pub fn prior_base(t: u8, ct: u8, planted: bool) -> f64 {
    if ct == 0 {
        return 1.0;
    }
    if t == 0 {
        return if planted { 0.5 } else { 0.0 };
    }
    let bias = if planted { T_BIAS_POST_PLANT } else { T_BIAS_PRE_PLANT };
    sigmoid(logit(duel_prior(t, ct)) + bias)
}

/// Round win-probability model (prior + optional local smoothing).
#[derive(Debug, Clone, Default)]
pub struct WinModel {
    local: HashMap<(u8, u8, bool), (u32, u32)>,
    /// Rounds in the local library the samples come from.
    pub local_rounds: u32,
}

impl WinModel {
    /// Prior only.
    pub fn prior() -> Self {
        Self::default()
    }

    /// Prior smoothed with local bucket samples (summed over the library). Below
    /// [`LOCAL_MIN_ROUNDS`] the samples are ignored.
    pub fn with_local(samples: impl IntoIterator<Item = BucketSample>, local_rounds: u32) -> Self {
        let mut local: HashMap<(u8, u8, bool), (u32, u32)> = HashMap::new();
        for s in samples {
            let e = local.entry((s.t_alive, s.ct_alive, s.planted)).or_default();
            e.0 += s.rounds;
            e.1 += s.t_wins;
        }
        Self { local, local_rounds }
    }

    /// Whether local samples are in use.
    pub fn uses_local(&self) -> bool {
        self.local_rounds >= LOCAL_MIN_ROUNDS && !self.local.is_empty()
    }

    /// T win probability of a bucket (prior, smoothed with local data when available).
    pub fn base(&self, t: u8, ct: u8, planted: bool) -> f64 {
        let prior = prior_base(t, ct, planted);
        if t == 0 || ct == 0 || !self.uses_local() {
            return prior;
        }
        match self.local.get(&(t, ct, planted)) {
            Some(&(n, w)) if n > 0 => (LOCAL_PSEUDO_ROUNDS * prior + w as f64) / (LOCAL_PSEUDO_ROUNDS + n as f64),
            _ => prior,
        }
    }

    /// T win probability in a state.
    pub fn predict(&self, s: &RoundState) -> f64 {
        let left = s.secs_left.max(0.0) as f64;
        if s.ct_alive == 0 {
            return 1.0;
        }
        if s.planted {
            let defuse = if s.ct_kit { DEFUSE_KIT_SECS } else { DEFUSE_SECS } as f64;
            if s.t_alive == 0 {
                // Uncontested defuse: depends only on whether there is time for it.
                return (0.5 - (left - defuse) / 6.0).clamp(0.02, 0.98);
            }
            let p = self.adjusted(s, true);
            let ct_chance = ((left - defuse) / RETAKE_MARGIN_SECS as f64).clamp(0.0, 1.0);
            return 1.0 - (1.0 - p) * ct_chance;
        }
        if s.t_alive == 0 {
            return 0.0;
        }
        let p = self.adjusted(s, false);
        p * (left / PLANT_TIME_PRESSURE_SECS as f64).clamp(0.0, 1.0)
    }

    fn adjusted(&self, s: &RoundState, planted: bool) -> f64 {
        let base = self.base(s.t_alive, s.ct_alive, planted);
        let equip = (EQUIP_LOGIT_PER_1000 * (s.t_equip - s.ct_equip) as f64 / 1000.0).clamp(-EQUIP_LOGIT_MAX, EQUIP_LOGIT_MAX);
        let hp = HP_LOGIT_PER_100 * (s.t_hp - s.ct_hp) as f64 / 100.0;
        sigmoid(logit(base) + equip + hp)
    }
}

// ---------------------------------------------------------------------------------------------
// Round simulation and attribution
// ---------------------------------------------------------------------------------------------

/// One point of a round's win-probability curve: an event and the T win probability right before
/// and right after it. Between two points the probability drifts with time (from the previous
/// `p_after` to the next `p_before`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WinProbPoint {
    pub tick: i32,
    /// Seconds since freeze end (or round start).
    pub time_secs: f32,
    /// `start` | `damage` | `kill` | `team_kill` | `suicide` | `world` | `planted` | `defused` |
    /// `exploded` | `end`
    pub kind: String,
    /// Main actor: attacker / planter / defuser (`None` for start, end, world deaths).
    #[serde(with = "opt_steamid_str", default)]
    pub player: Option<u64>,
    /// Victim of a kill or damage event.
    #[serde(with = "opt_steamid_str", default)]
    pub victim: Option<u64>,
    /// Alive counts after the event.
    pub t_alive: u8,
    pub ct_alive: u8,
    pub bomb_planted: bool,
    /// T win probability (0–1) before / after the event.
    pub p_before: f32,
    pub p_after: f32,
}

/// The win-probability curve of one round.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundWinProbability {
    pub round: u32,
    pub winner_side: Option<u8>,
    /// T win probability at freeze end.
    pub p_start: f32,
    pub points: Vec<WinProbPoint>,
}

/// Kind of a kill event (mirrors the stats kill classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeathKind {
    Kill,
    TeamKill,
    Suicide,
    World,
}

impl DeathKind {
    fn as_str(self) -> &'static str {
        match self {
            DeathKind::Kill => "kill",
            DeathKind::TeamKill => "team_kill",
            DeathKind::Suicide => "suicide",
            DeathKind::World => "world",
        }
    }
}

/// One participant at freeze end.
pub(crate) struct Participant {
    pub steamid: u64,
    pub side: u8,
    pub equip: u32,
    pub kit: bool,
}

pub(crate) struct RoundInput<'a> {
    pub round: u32,
    pub tick_rate: f32,
    /// Freeze end (or round start).
    pub origin: i32,
    pub end_tick: i32,
    pub winner: Option<u8>,
    pub participants: Vec<Participant>,
    /// Kills sorted by tick, with their kind and whether the victim was traded.
    pub kills: Vec<(&'a Kill, DeathKind, bool)>,
    /// Damage events sorted by tick.
    pub damages: Vec<&'a Damage>,
    pub bombs: Vec<&'a BombEvent>,
}

pub(crate) struct RoundOutput {
    pub curve: RoundWinProbability,
    /// Swing per player, in percentage points.
    pub swing: HashMap<u64, f32>,
    /// Buckets this round passed through (deduplicated), for local model updates.
    pub samples: Vec<BucketSample>,
}

enum Ev<'a> {
    Damage(&'a Damage),
    Kill(&'a Kill, DeathKind, bool),
    Bomb(&'a BombEvent),
}

struct Sim<'m> {
    model: &'m WinModel,
    tick_rate: f32,
    origin: i32,
    side: BTreeMap<u64, u8>,
    equip: HashMap<u64, u32>,
    kit: HashMap<u64, bool>,
    alive: BTreeSet<u64>,
    hp: HashMap<u64, i32>,
    plant_tick: Option<i32>,
    /// Round decided by the bomb: `Some(side)`.
    decided: Option<u8>,
    credit: HashMap<u64, f64>,
}

impl Sim<'_> {
    fn state(&self, tick: i32) -> RoundState {
        let mut t = (0u8, 0f32, 0f32);
        let mut ct = (0u8, 0f32, 0f32);
        let mut ct_kit = false;
        for p in &self.alive {
            let hp = *self.hp.get(p).unwrap_or(&100) as f32;
            let eq = *self.equip.get(p).unwrap_or(&0) as f32;
            let acc = if self.side[p] == TEAM_T { &mut t } else { &mut ct };
            acc.0 += 1;
            acc.1 += hp;
            acc.2 += eq;
            if self.side[p] == TEAM_CT && *self.kit.get(p).unwrap_or(&false) {
                ct_kit = true;
            }
        }
        let avg = |sum: f32, n: u8| if n > 0 { sum / n as f32 } else { 0.0 };
        let secs = |from: i32| (tick - from) as f32 / self.tick_rate;
        RoundState {
            t_alive: t.0,
            ct_alive: ct.0,
            t_hp: avg(t.1, t.0),
            ct_hp: avg(ct.1, ct.0),
            t_equip: avg(t.2, t.0),
            ct_equip: avg(ct.2, ct.0),
            planted: self.plant_tick.is_some(),
            secs_left: match self.plant_tick {
                Some(pt) => BOMB_TIME_SECS - secs(pt),
                None => ROUND_TIME_SECS - secs(self.origin),
            },
            ct_kit,
        }
    }

    fn p(&self, tick: i32) -> f64 {
        match self.decided {
            Some(s) => (s == TEAM_T) as u8 as f64,
            None => self.model.predict(&self.state(tick)),
        }
    }

    fn is_alive_side(&self, p: u64, side: u8) -> bool {
        self.alive.contains(&p) && self.side.get(&p) == Some(&side)
    }

    /// Players to share a side's even split: alive members, else every participant of the side.
    fn pool(&self, side: u8, exclude: Option<u64>) -> Vec<u64> {
        let alive: Vec<u64> = self.alive.iter().copied().filter(|p| self.side[p] == side && Some(*p) != exclude).collect();
        if !alive.is_empty() {
            return alive;
        }
        self.side.iter().filter(|(p, s)| **s == side && Some(**p) != exclude).map(|(p, _)| *p).collect()
    }

    fn split_even(&mut self, side: u8, amount: f64, exclude: Option<u64>) {
        if amount == 0.0 {
            return;
        }
        let mut pool = self.pool(side, exclude);
        if pool.is_empty() {
            pool = self.pool(side, None);
        }
        let n = pool.len() as f64;
        for p in pool {
            *self.credit.entry(p).or_default() += amount / n;
        }
    }

    fn give(&mut self, p: u64, amount: f64) {
        *self.credit.entry(p).or_default() += amount;
    }

    /// Credit an event's change: `gain_t` to the T side (and `−gain_t` to CT) via the closures.
    fn gain_for(side: u8, delta_t: f64) -> f64 {
        if side == TEAM_T {
            delta_t
        } else {
            -delta_t
        }
    }
}

fn other(side: u8) -> u8 {
    if side == TEAM_T {
        TEAM_CT
    } else {
        TEAM_T
    }
}

/// Replay one round and attribute every change of the T win probability.
pub(crate) fn simulate_round(model: &WinModel, input: RoundInput<'_>) -> RoundOutput {
    let mut sim = Sim {
        model,
        tick_rate: if input.tick_rate > 0.0 { input.tick_rate } else { 64.0 },
        origin: input.origin,
        side: input.participants.iter().map(|p| (p.steamid, p.side)).collect(),
        equip: input.participants.iter().map(|p| (p.steamid, p.equip)).collect(),
        kit: input.participants.iter().map(|p| (p.steamid, p.kit)).collect(),
        alive: input.participants.iter().map(|p| p.steamid).collect(),
        hp: HashMap::new(),
        plant_tick: None,
        decided: None,
        credit: HashMap::new(),
    };
    let time = |tick: i32, s: &Sim| (tick - s.origin) as f32 / s.tick_rate;

    // Merge events in tick order; on the same tick: damage, then kills, then bomb events.
    let mut events: Vec<(i32, u8, usize, Ev)> = vec![];
    for (i, d) in input.damages.iter().enumerate() {
        events.push((d.tick, 0, i, Ev::Damage(d)));
    }
    for (i, (k, kind, traded)) in input.kills.iter().enumerate() {
        events.push((k.tick, 1, i, Ev::Kill(k, *kind, *traded)));
    }
    for (i, b) in input.bombs.iter().enumerate() {
        if matches!(b.kind.as_str(), "planted" | "defused" | "exploded") {
            events.push((b.tick, 2, i, Ev::Bomb(b)));
        }
    }
    events.retain(|e| e.0 >= input.origin && e.0 <= input.end_tick);
    events.sort_by_key(|e| (e.0, e.1, e.2));

    // Damage dealt to each victim this round by each attacker (effective), for kill assists.
    let mut dealt: HashMap<(u64, u64), i32> = HashMap::new();
    let mut samples: BTreeSet<(u8, u8, bool)> = BTreeSet::new();
    let sample = |s: &Sim, set: &mut BTreeSet<(u8, u8, bool)>| {
        if s.decided.is_some() {
            return;
        }
        let st = s.state(s.origin);
        if st.t_alive > 0 && st.ct_alive > 0 {
            set.insert((st.t_alive, st.ct_alive, st.planted));
        }
    };

    let p_start = sim.p(input.origin);
    sample(&sim, &mut samples);
    let st0 = sim.state(input.origin);
    let mut points = vec![WinProbPoint {
        tick: input.origin,
        time_secs: 0.0,
        kind: "start".into(),
        player: None,
        victim: None,
        t_alive: st0.t_alive,
        ct_alive: st0.ct_alive,
        bomb_planted: false,
        p_before: p_start as f32,
        p_after: p_start as f32,
    }];
    let mut p_prev = p_start;

    for (tick, _, _, ev) in events {
        if sim.decided.is_some() {
            break;
        }
        // Bot takeover: a dead player who still deals damage controls a bot of their team (whose
        // owner disconnected and is not a participant). Count them alive again.
        let actor = match &ev {
            Ev::Damage(d) => d.attacker,
            Ev::Kill(k, _, _) => k.attacker,
            Ev::Bomb(b) => b.player,
        };
        if let Some(a) = actor.filter(|a| sim.side.contains_key(a) && !sim.alive.contains(a)) {
            sim.alive.insert(a);
            sim.hp.insert(a, 100);
        }
        // Time drift since the previous event: shared evenly by the alive players of each side.
        let before = sim.p(tick);
        let drift = before - p_prev;
        sim.split_even(TEAM_T, drift, None);
        sim.split_even(TEAM_CT, -drift, None);

        match ev {
            Ev::Damage(d) => {
                let Some(v) = d.victim else {
                    p_prev = before;
                    continue;
                };
                if !sim.alive.contains(&v) {
                    p_prev = before;
                    continue;
                }
                let prev = *sim.hp.get(&v).unwrap_or(&100);
                let after_hp = d.health_after.max(0);
                if after_hp == 0 {
                    // Lethal: the kill event takes care of it.
                    p_prev = before;
                    continue;
                }
                let eff = (prev - after_hp).max(0);
                if eff == 0 {
                    p_prev = before;
                    continue;
                }
                sim.hp.insert(v, after_hp);
                let after = sim.p(tick);
                let delta = after - before;
                let v_side = sim.side[&v];
                match d.attacker.filter(|a| *a != v && sim.side.contains_key(a)) {
                    Some(a) if sim.side[&a] != v_side => {
                        *dealt.entry((a, v)).or_default() += eff;
                        let a_side = sim.side[&a];
                        let g = Sim::gain_for(a_side, delta);
                        sim.give(a, g);
                        sim.give(v, -g);
                    }
                    Some(a) => {
                        // Team damage: the attacker carries the loss, the enemies share the gain.
                        let g = Sim::gain_for(v_side, delta);
                        sim.give(a, g);
                        sim.split_even(other(v_side), -g, None);
                    }
                    None => {
                        let g = Sim::gain_for(v_side, delta);
                        sim.give(v, g);
                        sim.split_even(other(v_side), -g, None);
                    }
                }
                p_prev = after;
                push(&mut points, &sim, tick, time(tick, &sim), "damage", d.attacker, Some(v), before, after);
            }
            Ev::Kill(k, dk, traded) => {
                let Some(v) = k.victim else { p_prev = before; continue };
                if !sim.alive.contains(&v) {
                    p_prev = before;
                    continue;
                }
                let v_side = sim.side[&v];
                sim.alive.remove(&v);
                let after = sim.p(tick);
                let delta = after - before;
                let attacker = k.attacker.filter(|a| sim.side.contains_key(a));
                match (dk, attacker) {
                    (DeathKind::Kill, Some(a)) if sim.side[&a] != v_side => {
                        let a_side = sim.side[&a];
                        let g = Sim::gain_for(a_side, delta);
                        // Killer side.
                        let mut left = 1.0;
                        if k.assisted_flash {
                            if let Some(fa) = k.assister.filter(|x| *x != a && sim.side.get(x) == Some(&a_side)) {
                                sim.give(fa, g * FLASH_ASSIST_SHARE);
                                left -= FLASH_ASSIST_SHARE;
                            }
                        }
                        let mates: Vec<(u64, i32)> = sim
                            .side
                            .iter()
                            .filter(|(p, s)| **s == a_side && **p != a)
                            .filter_map(|(p, _)| dealt.get(&(*p, v)).filter(|d| **d > 0).map(|d| (*p, *d)))
                            .collect();
                        let mate_dmg: i32 = mates.iter().map(|m| m.1).sum();
                        if mate_dmg > 0 {
                            let pool = DAMAGE_ASSIST_MAX_SHARE * (mate_dmg as f64 / 100.0).min(1.0);
                            for (m, d) in &mates {
                                sim.give(*m, g * pool * *d as f64 / mate_dmg as f64);
                            }
                            left -= pool;
                        }
                        sim.give(a, g * left);
                        // Victim side.
                        let share = if traded { DEATH_SHARE_TRADED } else { DEATH_SHARE };
                        let mates_alive = sim.alive.iter().any(|p| sim.side[p] == v_side);
                        if mates_alive {
                            sim.give(v, -g * share);
                            sim.split_even(v_side, -g * (1.0 - share), None);
                        } else {
                            sim.give(v, -g);
                        }
                    }
                    (DeathKind::TeamKill, Some(a)) | (DeathKind::Kill, Some(a)) => {
                        let g = Sim::gain_for(v_side, delta);
                        sim.give(a, g);
                        sim.split_even(other(v_side), -g, None);
                    }
                    _ => {
                        let g = Sim::gain_for(v_side, delta);
                        sim.give(v, g);
                        sim.split_even(other(v_side), -g, None);
                    }
                }
                p_prev = after;
                sample(&sim, &mut samples);
                push(&mut points, &sim, tick, time(tick, &sim), dk.as_str(), attacker, Some(v), before, after);
            }
            Ev::Bomb(b) => {
                let actor = b.player.filter(|p| sim.side.contains_key(p));
                match b.kind.as_str() {
                    "planted" => {
                        if sim.plant_tick.is_some() {
                            p_prev = before;
                            continue;
                        }
                        sim.plant_tick = Some(tick);
                    }
                    "defused" => sim.decided = Some(TEAM_CT),
                    _ => sim.decided = Some(TEAM_T),
                }
                let after = sim.p(tick);
                let delta = after - before;
                // The acting side: T for plant / explosion, CT for defuse.
                let side = if b.kind == "defused" { TEAM_CT } else { TEAM_T };
                let g = Sim::gain_for(side, delta);
                let credited = match b.kind.as_str() {
                    "exploded" => planter(&input.bombs).filter(|p| sim.side.get(p) == Some(&TEAM_T)),
                    _ => actor.filter(|p| sim.is_alive_side(*p, side)),
                };
                match credited {
                    Some(p) => sim.give(p, g),
                    None => sim.split_even(side, g, None),
                }
                sim.split_even(other(side), -g, None);
                let kind = match b.kind.as_str() {
                    "planted" => "planted",
                    "defused" => "defused",
                    _ => "exploded",
                };
                p_prev = after;
                sample(&sim, &mut samples);
                push(&mut points, &sim, tick, time(tick, &sim), kind, credited.or(actor), None, before, after);
            }
        }
    }

    // Round end: converge to the result. The remaining change is usually small (time pressure is
    // priced in by the drift, the bomb by its events); large residuals come from things the events
    // don't show (disconnects, bot takeovers), so it is shared by every participant of each side.
    let end = input.end_tick.max(input.origin);
    if let Some(w) = input.winner.filter(|w| *w == TEAM_T || *w == TEAM_CT) {
        let before = if sim.decided.is_some() { p_prev } else { sim.p(end) };
        let drift = before - p_prev;
        sim.split_even(TEAM_T, drift, None);
        sim.split_even(TEAM_CT, -drift, None);
        let after = if w == TEAM_T { 1.0 } else { 0.0 };
        let g = Sim::gain_for(w, after - before);
        push(&mut points, &sim, end, time(end, &sim), "end", None, None, before, after);
        sim.alive.clear(); // → split over all participants
        sim.split_even(w, g, None);
        sim.split_even(other(w), -g, None);
    }

    let won_t = input.winner == Some(TEAM_T);
    let samples = if input.winner.is_some_and(|w| w == TEAM_T || w == TEAM_CT) {
        samples
            .into_iter()
            .map(|(t, ct, planted)| BucketSample { t_alive: t, ct_alive: ct, planted, rounds: 1, t_wins: won_t as u32 })
            .collect()
    } else {
        vec![]
    };
    RoundOutput {
        curve: RoundWinProbability { round: input.round, winner_side: input.winner, p_start: p_start as f32, points },
        swing: sim.credit.into_iter().map(|(p, v)| (p, (v * 100.0) as f32)).collect(),
        samples,
    }
}

fn planter(bombs: &[&BombEvent]) -> Option<u64> {
    bombs.iter().rev().find(|b| b.kind == "planted").and_then(|b| b.player)
}

#[allow(clippy::too_many_arguments)]
fn push(
    points: &mut Vec<WinProbPoint>,
    sim: &Sim,
    tick: i32,
    time_secs: f32,
    kind: &str,
    player: Option<u64>,
    victim: Option<u64>,
    before: f64,
    after: f64,
) {
    let st = sim.state(tick);
    points.push(WinProbPoint {
        tick,
        time_secs,
        kind: kind.to_string(),
        player,
        victim,
        t_alive: st.t_alive,
        ct_alive: st.ct_alive,
        bomb_planted: st.planted,
        p_before: before as f32,
        p_after: after as f32,
    });
}

/// Merge per-match bucket samples (one row per bucket).
pub fn merge_samples(samples: impl IntoIterator<Item = BucketSample>) -> Vec<BucketSample> {
    let mut m: BTreeMap<(u8, u8, bool), BucketSample> = BTreeMap::new();
    for s in samples {
        let e = m.entry((s.t_alive, s.ct_alive, s.planted)).or_insert(BucketSample { rounds: 0, t_wins: 0, ..s });
        e.rounds += s.rounds;
        e.t_wins += s.t_wins;
    }
    m.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(t: u8, ct: u8, planted: bool, left: f32) -> RoundState {
        RoundState {
            t_alive: t,
            ct_alive: ct,
            t_hp: 100.0,
            ct_hp: 100.0,
            t_equip: 4000.0,
            ct_equip: 4000.0,
            planted,
            secs_left: left,
            ct_kit: false,
        }
    }

    #[test]
    fn prior_table_is_sane() {
        assert!((duel_prior(5, 5) - 0.5).abs() < 1e-9);
        assert!((duel_prior(5, 4) - 0.716).abs() < 0.01, "5v4 {}", duel_prior(5, 4));
        let p55 = prior_base(5, 5, false);
        assert!((0.45..0.49).contains(&p55), "5v5 {p55}");
        let p55p = prior_base(5, 5, true);
        assert!((0.68..0.72).contains(&p55p), "5v5 planted {p55p}");
        // Monotone in both alive counts.
        for planted in [false, true] {
            for t in 1..=5u8 {
                for ct in 1..=5u8 {
                    if t < 5 {
                        assert!(prior_base(t + 1, ct, planted) > prior_base(t, ct, planted));
                    }
                    if ct < 5 {
                        assert!(prior_base(t, ct + 1, planted) < prior_base(t, ct, planted));
                    }
                }
            }
        }
        // Print the table for the docs.
        for planted in [false, true] {
            eprintln!("planted={planted}");
            for t in 1..=5u8 {
                let row: Vec<String> = (1..=5u8).map(|ct| format!("{:.2}", prior_base(t, ct, planted))).collect();
                eprintln!("T{t}: {}", row.join(" "));
            }
        }
    }

    #[test]
    fn time_and_bomb_effects() {
        let m = WinModel::prior();
        // Running out of time without a plant: T chance falls to 0.
        assert!(m.predict(&st(5, 5, false, 100.0)) > 0.4);
        assert!(m.predict(&st(5, 5, false, 5.0)) < 0.15);
        assert_eq!(m.predict(&st(5, 5, false, 0.0)), 0.0);
        // Bomb about to explode: T wins.
        assert!(m.predict(&st(1, 5, true, 3.0)) > 0.99);
        // Planted, T dead, plenty of time: CT defuses.
        assert!(m.predict(&st(0, 2, true, 30.0)) < 0.05);
        // Equipment matters.
        let mut eco = st(5, 5, false, 100.0);
        eco.t_equip = 700.0;
        eco.ct_equip = 4300.0;
        let p = m.predict(&eco);
        assert!((0.08..0.2).contains(&p), "eco {p}");
    }

    #[test]
    fn local_smoothing() {
        let samples = vec![BucketSample { t_alive: 5, ct_alive: 4, planted: false, rounds: 300, t_wins: 270 }];
        // Not enough local rounds yet: prior only.
        let few = WinModel::with_local(samples.clone(), 100);
        assert_eq!(few.base(5, 4, false), prior_base(5, 4, false));
        let many = WinModel::with_local(samples, 1000);
        let expect = (100.0 * prior_base(5, 4, false) + 270.0) / 400.0;
        assert!((many.base(5, 4, false) - expect).abs() < 1e-12);
        // Untouched buckets keep the prior.
        assert_eq!(many.base(3, 3, true), prior_base(3, 3, true));
    }
}
