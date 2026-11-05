//! Derived per-match statistics, computed from normalized demo events (`demo-parse` model).
//!
//! Pure computation, no I/O: the input is a borrowed view of one match's events
//! ([`MatchEvents`], built from a [`demo_parse::ParsedDemo`] or from database rows) and the output
//! is [`MatchStats`]. See `docs/stats.md` for every metric's definition.
//!
//! Conventions:
//! - Side numbers follow the engine: 2 = T, 3 = CT. A player's side in a round comes from the
//!   freeze-end economy snapshot.
//! - Teams are side-independent: team **A** = the team that started on CT, team **B** = the team
//!   that started on T. Side swaps (halftime, overtime) are detected per round.
//! - Suicides (attacker = victim / no attacker) and team kills never count as kills; team damage
//!   never counts towards ADR. Deaths count every death (same as the in-game scoreboard).
//! - Damage is *effective* damage: capped at the victim's remaining health.

mod compute;
mod econ;

pub use compute::compute;
pub use demo_parse::{TEAM_CT, TEAM_T};
pub use econ::BuyType;

use demo_parse::{BombEvent, Blind, Damage, Economy, Grenade, Kill, ParsedDemo, Player, Purchase, Round};
use serde::{Deserialize, Serialize};

/// Bump when any metric definition changes; stored rows with an older version are recomputed.
pub const STATS_VERSION: i32 = 1;

/// Trade window in seconds (a death is "traded" if the killer dies within this time).
pub const TRADE_WINDOW_SECS: f32 = 5.0;

/// Borrowed view of one match's events.
#[derive(Clone, Copy)]
pub struct MatchEvents<'a> {
    pub tick_rate: f32,
    pub players: &'a [Player],
    pub rounds: &'a [Round],
    pub kills: &'a [Kill],
    pub damages: &'a [Damage],
    pub grenades: &'a [Grenade],
    pub blinds: &'a [Blind],
    pub bombs: &'a [BombEvent],
    pub purchases: &'a [Purchase],
    pub economies: &'a [Economy],
}

impl<'a> From<&'a ParsedDemo> for MatchEvents<'a> {
    fn from(d: &'a ParsedDemo) -> Self {
        Self {
            tick_rate: if d.header.tick_rate > 0.0 { d.header.tick_rate } else { 64.0 },
            players: &d.players,
            rounds: &d.rounds,
            kills: &d.kills,
            damages: &d.damages,
            grenades: &d.grenades,
            blinds: &d.blinds,
            bombs: &d.bombs,
            purchases: &d.purchases,
            economies: &d.economies,
        }
    }
}

/// Side-independent team: A started on CT, B started on T.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Team {
    A,
    B,
}

impl Team {
    pub fn as_str(self) -> &'static str {
        match self {
            Team::A => "A",
            Team::B => "B",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "A" => Some(Team::A),
            "B" => Some(Team::B),
            _ => None,
        }
    }
    pub fn other(self) -> Self {
        match self {
            Team::A => Team::B,
            Team::B => Team::A,
        }
    }
}

/// Serialize Steam64 as a string (exceeds JS `Number.MAX_SAFE_INTEGER`).
pub mod steamid_str {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Serialize `Option<u64>` Steam64 as an optional string.
pub mod opt_steamid_str {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &Option<u64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => s.serialize_str(&v.to_string()),
            None => s.serialize_none(),
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
        let s = Option::<String>::deserialize(d)?;
        s.map(|s| s.parse().map_err(serde::de::Error::custom)).transpose()
    }
}

/// Everything derived from one match.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchStats {
    pub version: i32,
    /// False when the demo has flashbangs but no `player_blind` events (ESL servers): blind
    /// metrics are then `None`, not 0.
    pub blind_data_available: bool,
    pub score_a: u32,
    pub score_b: u32,
    /// `None` on a draw.
    pub winner: Option<Team>,
    pub players: Vec<PlayerMatchStats>,
    pub player_rounds: Vec<PlayerRoundStats>,
    pub team_rounds: Vec<TeamRoundStats>,
    /// Per-round summary and kill feed (for the match timeline).
    pub rounds: Vec<RoundSummary>,
    /// attacker × victim totals over enemy interactions only.
    pub pairs: Vec<PairStats>,
}

/// One player's totals for one match. Counts are additive across matches (see [`PlayerMatchStats::add`]);
/// rates are recomputed by [`PlayerMatchStats::finalize`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerMatchStats {
    #[serde(with = "steamid_str")]
    pub steamid: u64,
    pub name: String,
    /// `None` only for aggregates.
    pub team: Option<Team>,
    /// Side in the player's first round (2 = T, 3 = CT).
    pub start_side: Option<u8>,
    /// Matches aggregated (1 for a single match).
    pub matches: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub rounds_played: u32,
    pub rounds_t: u32,
    pub rounds_ct: u32,

    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub flash_assists: u32,
    pub headshot_kills: u32,
    pub team_kills: u32,
    pub suicides: u32,
    /// Effective damage dealt to enemies.
    pub damage: u32,
    pub kast_rounds: u32,
    pub rounds_survived: u32,
    pub mvps: u32,
    pub multi_kills: [u32; 6], // index = kills in a round (2..=5 used)

    // Opening duels, by the player's side at the time.
    pub opening_kills_t: u32,
    pub opening_deaths_t: u32,
    pub opening_kills_ct: u32,
    pub opening_deaths_ct: u32,

    // Trades (5 s window).
    pub trade_kill_opportunities: u32,
    pub trade_kill_attempts: u32,
    pub trade_kills: u32,
    pub traded_death_opportunities: u32,
    pub traded_deaths: u32,

    // Clutches: index = number of enemies alive when the player became the last one (1..=5 used).
    pub clutch_attempts: [u32; 6],
    pub clutch_wins: [u32; 6],
    pub clutch_kills: u32,

    // Utility.
    pub flashes_thrown: u32,
    pub he_thrown: u32,
    pub smokes_thrown: u32,
    pub molotovs_thrown: u32,
    pub decoys_thrown: u32,
    /// Flashes thrown in matches that have blind data (denominator for the per-flash averages).
    pub blind_flashes: u32,
    /// `None` when blind data is unavailable for (any of) the aggregated matches.
    pub enemies_flashed: Option<u32>,
    pub enemy_blind_seconds: Option<f32>,
    pub teammates_flashed: Option<u32>,
    pub teammate_blind_seconds: Option<f32>,
    pub he_damage: u32,
    pub he_team_damage: u32,
    pub fire_damage: u32,
    /// Approximate value of grenades still held at death (summed over deaths).
    pub unused_utility_value: u32,

    /// Derived values (filled by `finalize`).
    #[serde(default)]
    pub derived: Derived,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Derived {
    pub kd: f32,
    pub adr: f32,
    /// 0–100.
    pub kast: f32,
    /// 0–100.
    pub hs_pct: f32,
    pub kpr: f32,
    pub dpr: f32,
    pub apr: f32,
    pub impact: f32,
    /// HLTV Rating 2.0 (public reverse-engineered approximation, reference only).
    pub rating: f32,
    /// 0–100.
    pub survival_rate: f32,
    pub opening_attempts: u32,
    pub opening_success_pct: f32,
    /// 0–100, share of trade opportunities converted into a kill.
    pub trade_kill_pct: f32,
    /// 0–100, share of deaths (with teammates alive) that were traded.
    pub traded_death_pct: f32,
    pub utility_damage: u32,
    pub utility_thrown_per_round: f32,
    pub he_damage_per_he: f32,
    pub enemies_flashed_per_flash: Option<f32>,
    pub enemy_blind_seconds_per_flash: Option<f32>,
    pub teammates_flashed_per_flash: Option<f32>,
}

fn ratio(a: f32, b: f32) -> f32 {
    if b > 0.0 {
        a / b
    } else {
        0.0
    }
}

impl PlayerMatchStats {
    /// Add another row's counts (for multi-match aggregates). Identity fields are kept from `self`.
    pub fn add(&mut self, o: &PlayerMatchStats) {
        macro_rules! sum {
            ($($f:ident),*) => { $( self.$f += o.$f; )* };
        }
        sum!(
            matches, wins, losses, draws, rounds_played, rounds_t, rounds_ct, kills, deaths, assists, flash_assists,
            headshot_kills, team_kills, suicides, damage, kast_rounds, rounds_survived, mvps, opening_kills_t,
            opening_deaths_t, opening_kills_ct, opening_deaths_ct, trade_kill_opportunities, trade_kill_attempts,
            trade_kills, traded_death_opportunities, traded_deaths, clutch_kills, flashes_thrown, he_thrown,
            smokes_thrown, molotovs_thrown, decoys_thrown, he_damage, he_team_damage, fire_damage,
            unused_utility_value
        );
        for i in 0..6 {
            self.multi_kills[i] += o.multi_kills[i];
            self.clutch_attempts[i] += o.clutch_attempts[i];
            self.clutch_wins[i] += o.clutch_wins[i];
        }
        // Blind metrics only aggregate over matches where they are available.
        fn add_opt<T: std::ops::Add<Output = T> + Copy>(a: &mut Option<T>, b: Option<T>) {
            if let Some(b) = b {
                *a = Some(match *a {
                    Some(x) => x + b,
                    None => b,
                });
            }
        }
        add_opt(&mut self.enemies_flashed, o.enemies_flashed);
        add_opt(&mut self.enemy_blind_seconds, o.enemy_blind_seconds);
        add_opt(&mut self.teammates_flashed, o.teammates_flashed);
        add_opt(&mut self.teammate_blind_seconds, o.teammate_blind_seconds);
        self.blind_flashes += o.blind_flashes;
    }

    /// Recompute [`Derived`] from the counts.
    pub fn finalize(&mut self) {
        let r = self.rounds_played as f32;
        let kpr = ratio(self.kills as f32, r);
        let dpr = ratio(self.deaths as f32, r);
        let apr = ratio(self.assists as f32, r);
        let adr = ratio(self.damage as f32, r);
        let kast = 100.0 * ratio(self.kast_rounds as f32, r);
        let impact = 2.13 * kpr + 0.42 * apr - 0.41;
        let rating = if r > 0.0 { 0.0073 * kast + 0.3591 * kpr - 0.5329 * dpr + 0.2372 * impact + 0.0032 * adr + 0.1587 } else { 0.0 };
        let opening_kills = self.opening_kills_t + self.opening_kills_ct;
        let opening_attempts = opening_kills + self.opening_deaths_t + self.opening_deaths_ct;
        let thrown = self.flashes_thrown + self.he_thrown + self.smokes_thrown + self.molotovs_thrown + self.decoys_thrown;
        // Per-flash blind averages only over flashes from matches with blind data.
        let flashes = self.blind_flashes as f32;
        self.derived = Derived {
            kd: if self.deaths > 0 { self.kills as f32 / self.deaths as f32 } else { self.kills as f32 },
            adr,
            kast,
            hs_pct: 100.0 * ratio(self.headshot_kills as f32, self.kills as f32),
            kpr,
            dpr,
            apr,
            impact,
            rating,
            survival_rate: 100.0 * ratio(self.rounds_survived as f32, r),
            opening_attempts,
            opening_success_pct: 100.0 * ratio(opening_kills as f32, opening_attempts as f32),
            trade_kill_pct: 100.0 * ratio(self.trade_kills as f32, self.trade_kill_opportunities as f32),
            traded_death_pct: 100.0 * ratio(self.traded_deaths as f32, self.traded_death_opportunities as f32),
            utility_damage: self.he_damage + self.fire_damage,
            utility_thrown_per_round: ratio(thrown as f32, r),
            he_damage_per_he: ratio(self.he_damage as f32, self.he_thrown as f32),
            enemies_flashed_per_flash: self.enemies_flashed.map(|v| ratio(v as f32, flashes)),
            enemy_blind_seconds_per_flash: self.enemy_blind_seconds.map(|v| ratio(v, flashes)),
            teammates_flashed_per_flash: self.teammates_flashed.map(|v| ratio(v as f32, flashes)),
        };
    }
}

/// One player in one round.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRoundStats {
    pub round: u32,
    #[serde(with = "steamid_str")]
    pub steamid: u64,
    pub team: Team,
    pub side: u8,
    pub kills: u32,
    pub died: bool,
    /// Tick of death (if died).
    pub death_tick: Option<i32>,
    pub assists: u32,
    pub flash_assists: u32,
    pub headshot_kills: u32,
    pub damage: u32,
    pub utility_damage: u32,
    pub kast: bool,
    pub traded: bool,
    pub trade_kills: u32,
    pub opening_kill: bool,
    pub opening_death: bool,
    /// Enemies alive when this player became the last alive of their side.
    pub clutch_vs: Option<u8>,
    pub clutch_won: Option<bool>,
    pub mvp: bool,
    /// Equipment value at freeze end plus later purchases.
    pub equip_value: u32,
    /// Money at freeze end.
    pub balance: u32,
    pub utility_thrown: u32,
    pub enemies_flashed: Option<u32>,
    pub unused_utility_value: u32,
}

/// One team in one round.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamRoundStats {
    pub round: u32,
    pub team: Team,
    pub side: u8,
    pub won: bool,
    /// Score of this team after the round.
    pub score: u32,
    pub equip_value: u32,
    pub balance: u32,
    pub buy_type: BuyType,
    pub kills: u32,
    pub survivors: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundSummary {
    pub number: u32,
    pub winner_side: Option<u8>,
    pub winner_team: Option<Team>,
    /// Engine reason string, e.g. `t_killed`, `bomb_exploded`.
    pub reason: Option<String>,
    /// Side team A played this round.
    pub side_a: u8,
    pub score_a: u32,
    pub score_b: u32,
    pub start_tick: i32,
    pub freeze_end_tick: Option<i32>,
    pub end_tick: i32,
    pub duration_secs: f32,
    pub bomb_planted: bool,
    pub bomb_site: Option<i32>,
    #[serde(with = "opt_steamid_str")]
    pub mvp: Option<u64>,
    pub kills: Vec<KillEvent>,
}

/// Kill feed entry with derived flags.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillEvent {
    pub tick: i32,
    /// Seconds since freeze end (or round start).
    pub time_secs: f32,
    #[serde(with = "opt_steamid_str")]
    pub attacker: Option<u64>,
    #[serde(with = "opt_steamid_str")]
    pub victim: Option<u64>,
    #[serde(with = "opt_steamid_str")]
    pub assister: Option<u64>,
    pub weapon: String,
    pub headshot: bool,
    pub assisted_flash: bool,
    pub attacker_side: Option<u8>,
    pub victim_side: Option<u8>,
    /// `kill` | `team_kill` | `suicide` | `world`
    pub kind: &'static str,
    pub opening: bool,
    /// This kill avenged a teammate's death within the trade window.
    pub trade: bool,
    /// The victim's death was avenged within the trade window.
    pub traded: bool,
    /// Happened after the round was decided.
    pub after_round_end: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairStats {
    #[serde(with = "steamid_str")]
    pub attacker: u64,
    #[serde(with = "steamid_str")]
    pub victim: u64,
    pub damage: u32,
    pub hits: u32,
    pub kills: u32,
    pub headshot_kills: u32,
    pub opening_kills: u32,
}
