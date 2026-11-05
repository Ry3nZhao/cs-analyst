//! Persistence for derived stats (migration 003) and loading match events back from the 002 tables.

use crate::database::{Database, Result};
use demo_parse::{BombEvent, Blind, Damage, Economy, Grenade, Kill, Player, Purchase, Round};
use rusqlite::{params, Connection, OptionalExtension, Row};
use stats::{MatchEvents, MatchStats, PlayerMatchStats, Team, STATS_VERSION};

/// SourceTV tick rate; not stored in the database (demo-parse always reports 64).
pub const DEFAULT_TICK_RATE: f32 = 64.0;

const STATS_TABLES: &[&str] = &["player_match_stats", "player_round_stats", "team_round_stats"];

fn sid(v: u64) -> i64 {
    v as i64
}

fn osid(v: Option<i64>) -> Option<u64> {
    v.map(|v| v as u64)
}

fn side(v: Option<i64>) -> Option<u8> {
    v.and_then(|v| u8::try_from(v).ok())
}

/// One match's events loaded from the database (owned; borrow with [`LoadedEvents::view`]).
#[derive(Default)]
pub struct LoadedEvents {
    pub players: Vec<Player>,
    pub rounds: Vec<Round>,
    pub kills: Vec<Kill>,
    pub damages: Vec<Damage>,
    pub grenades: Vec<Grenade>,
    pub blinds: Vec<Blind>,
    pub bombs: Vec<BombEvent>,
    pub purchases: Vec<Purchase>,
    pub economies: Vec<Economy>,
}

impl LoadedEvents {
    pub fn view(&self) -> MatchEvents<'_> {
        MatchEvents {
            tick_rate: DEFAULT_TICK_RATE,
            players: &self.players,
            rounds: &self.rounds,
            kills: &self.kills,
            damages: &self.damages,
            grenades: &self.grenades,
            blinds: &self.blinds,
            bombs: &self.bombs,
            purchases: &self.purchases,
            economies: &self.economies,
        }
    }
}

fn query<T>(c: &Connection, sql: &str, m: &str, f: impl FnMut(&Row) -> rusqlite::Result<T>) -> Result<Vec<T>> {
    let mut st = c.prepare(sql)?;
    let rows = st.query_map(params![m], f)?;
    Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
}

/// Load the parsed events of a match (empty `rounds` if it was never analyzed).
pub fn load_events(db: &Database, m: &str) -> Result<LoadedEvents> {
    let c = &db.conn;
    Ok(LoadedEvents {
        players: query(
            c,
            "SELECT steamid, name, team_start, team_end, rank, rank_type FROM match_players WHERE match_id = ? ORDER BY slot",
            m,
            |r| {
                Ok(Player {
                    steamid: r.get::<_, i64>(0)? as u64,
                    name: r.get(1)?,
                    team_start: side(r.get(2)?),
                    team_end: side(r.get(3)?),
                    rank: r.get(4)?,
                    rank_type: r.get(5)?,
                })
            },
        )?,
        rounds: query(
            c,
            "SELECT number, start_tick, freeze_end_tick, end_tick, winner, reason FROM rounds WHERE match_id = ? ORDER BY number",
            m,
            |r| {
                Ok(Round {
                    number: r.get(0)?,
                    start_tick: r.get(1)?,
                    freeze_end_tick: r.get(2)?,
                    end_tick: r.get(3)?,
                    winner: side(r.get(4)?),
                    reason: r.get(5)?,
                })
            },
        )?,
        kills: query(
            c,
            "SELECT round, tick, attacker, victim, assister, weapon, headshot, penetrated, noscope, thrusmoke,
               attacker_blind, attacker_in_air, assisted_flash, distance, attacker_team, victim_team
             FROM kills WHERE match_id = ? ORDER BY rowid",
            m,
            |r| {
                Ok(Kill {
                    round: r.get(0)?,
                    tick: r.get(1)?,
                    attacker: osid(r.get(2)?),
                    victim: osid(r.get(3)?),
                    assister: osid(r.get(4)?),
                    weapon: r.get(5)?,
                    headshot: r.get(6)?,
                    penetrated: r.get(7)?,
                    noscope: r.get(8)?,
                    thrusmoke: r.get(9)?,
                    attacker_blind: r.get(10)?,
                    attacker_in_air: r.get(11)?,
                    assisted_flash: r.get(12)?,
                    distance: r.get(13)?,
                    attacker_pos: None,
                    victim_pos: None,
                    attacker_team: side(r.get(14)?),
                    victim_team: side(r.get(15)?),
                })
            },
        )?,
        damages: query(
            c,
            "SELECT round, tick, attacker, victim, weapon, dmg_health, dmg_armor, health_after, armor_after, hitgroup,
               attacker_team, victim_team
             FROM damages WHERE match_id = ? ORDER BY rowid",
            m,
            |r| {
                Ok(Damage {
                    round: r.get(0)?,
                    tick: r.get(1)?,
                    attacker: osid(r.get(2)?),
                    victim: osid(r.get(3)?),
                    weapon: r.get(4)?,
                    dmg_health: r.get(5)?,
                    dmg_armor: r.get(6)?,
                    health_after: r.get(7)?,
                    armor_after: r.get(8)?,
                    hitgroup: r.get(9)?,
                    attacker_team: side(r.get(10)?),
                    victim_team: side(r.get(11)?),
                })
            },
        )?,
        grenades: query(
            c,
            "SELECT round, tick, thrower, kind, entity_id FROM grenades WHERE match_id = ? ORDER BY rowid",
            m,
            |r| {
                Ok(Grenade {
                    round: r.get(0)?,
                    tick: r.get(1)?,
                    thrower: osid(r.get(2)?),
                    kind: r.get(3)?,
                    entity_id: r.get(4)?,
                    pos: None,
                })
            },
        )?,
        blinds: query(
            c,
            "SELECT round, tick, attacker, victim, duration, attacker_team, victim_team FROM blinds WHERE match_id = ? ORDER BY rowid",
            m,
            |r| {
                Ok(Blind {
                    round: r.get(0)?,
                    tick: r.get(1)?,
                    attacker: osid(r.get(2)?),
                    victim: osid(r.get(3)?),
                    duration: r.get(4)?,
                    attacker_team: side(r.get(5)?),
                    victim_team: side(r.get(6)?),
                })
            },
        )?,
        bombs: query(
            c,
            "SELECT round, tick, player, kind, site FROM bomb_events WHERE match_id = ? ORDER BY rowid",
            m,
            |r| Ok(BombEvent { round: r.get(0)?, tick: r.get(1)?, player: osid(r.get(2)?), kind: r.get(3)?, site: r.get(4)?, pos: None }),
        )?,
        purchases: query(
            c,
            "SELECT round, tick, player, item, cost, was_sold FROM purchases WHERE match_id = ? ORDER BY rowid",
            m,
            |r| {
                Ok(Purchase {
                    round: r.get(0)?,
                    tick: r.get(1)?,
                    player: r.get::<_, i64>(2)? as u64,
                    item: r.get(3)?,
                    cost: r.get(4)?,
                    was_sold: r.get(5)?,
                })
            },
        )?,
        economies: query(
            c,
            "SELECT round, player, team, balance, equip_value, round_start_equip_value, cash_spent_this_round, armor,
               has_helmet, has_defuser
             FROM economies WHERE match_id = ? ORDER BY round, rowid",
            m,
            |r| {
                Ok(Economy {
                    round: r.get(0)?,
                    player: r.get::<_, i64>(1)? as u64,
                    team: side(r.get(2)?),
                    balance: r.get(3)?,
                    equip_value: r.get(4)?,
                    round_start_equip_value: r.get(5)?,
                    cash_spent_this_round: r.get(6)?,
                    armor: r.get(7)?,
                    has_helmet: r.get(8)?,
                    has_defuser: r.get(9)?,
                })
            },
        )?,
    })
}

/// Replace the stored stats of a match, in one transaction.
pub fn replace_match_stats(db: &Database, m: &str, s: &MatchStats) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    write_stats(&tx, m, s)?;
    tx.commit()?;
    Ok(())
}

fn write_stats(c: &Connection, m: &str, s: &MatchStats) -> rusqlite::Result<()> {
    for t in STATS_TABLES {
        c.execute(&format!("DELETE FROM {t} WHERE match_id = ?"), params![m])?;
    }

    let mut st = c.prepare(&format!(
        "INSERT INTO player_match_stats ({PMS_COLUMNS}) VALUES ({})",
        vec!["?"; PMS_COLUMNS.split(',').count()].join(", ")
    ))?;
    for p in &s.players {
        let won: Option<i64> = if p.wins > 0 {
            Some(1)
        } else if p.losses > 0 {
            Some(0)
        } else {
            None
        };
        let d = &p.derived;
        st.execute(rusqlite::params_from_iter([
            &m as &dyn rusqlite::ToSql,
            &sid(p.steamid),
            &STATS_VERSION,
            &p.name,
            &p.team.map(Team::as_str).unwrap_or("A"),
            &p.start_side,
            &won,
            &p.rounds_played,
            &p.rounds_t,
            &p.rounds_ct,
            &p.kills,
            &p.deaths,
            &p.assists,
            &p.flash_assists,
            &p.headshot_kills,
            &p.team_kills,
            &p.suicides,
            &p.damage,
            &p.kast_rounds,
            &p.rounds_survived,
            &p.mvps,
            &p.multi_kills[2],
            &p.multi_kills[3],
            &p.multi_kills[4],
            &p.multi_kills[5],
            &p.opening_kills_t,
            &p.opening_deaths_t,
            &p.opening_kills_ct,
            &p.opening_deaths_ct,
            &p.trade_kill_opportunities,
            &p.trade_kill_attempts,
            &p.trade_kills,
            &p.traded_death_opportunities,
            &p.traded_deaths,
            &p.clutch_attempts[1],
            &p.clutch_wins[1],
            &p.clutch_attempts[2],
            &p.clutch_wins[2],
            &p.clutch_attempts[3],
            &p.clutch_wins[3],
            &p.clutch_attempts[4],
            &p.clutch_wins[4],
            &p.clutch_attempts[5],
            &p.clutch_wins[5],
            &p.clutch_kills,
            &p.flashes_thrown,
            &p.he_thrown,
            &p.smokes_thrown,
            &p.molotovs_thrown,
            &p.decoys_thrown,
            &p.enemies_flashed,
            &p.enemy_blind_seconds,
            &p.teammates_flashed,
            &p.teammate_blind_seconds,
            &p.he_damage,
            &p.he_team_damage,
            &p.fire_damage,
            &p.unused_utility_value,
            &d.adr,
            &d.kast,
            &d.hs_pct,
            &d.rating,
        ]))?;
    }

    let mut st = c.prepare(
        "INSERT INTO player_round_stats (match_id, round, steamid, team, side, kills, died, death_tick, assists,
           flash_assists, headshot_kills, damage, utility_damage, kast, traded, trade_kills, opening_kill,
           opening_death, clutch_vs, clutch_won, mvp, equip_value, balance, utility_thrown, enemies_flashed,
           unused_utility_value)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for r in &s.player_rounds {
        st.execute(params![
            m,
            r.round,
            sid(r.steamid),
            r.team.as_str(),
            r.side,
            r.kills,
            r.died,
            r.death_tick,
            r.assists,
            r.flash_assists,
            r.headshot_kills,
            r.damage,
            r.utility_damage,
            r.kast,
            r.traded,
            r.trade_kills,
            r.opening_kill,
            r.opening_death,
            r.clutch_vs,
            r.clutch_won,
            r.mvp,
            r.equip_value,
            r.balance,
            r.utility_thrown,
            r.enemies_flashed,
            r.unused_utility_value
        ])?;
    }

    let mut st = c.prepare(
        "INSERT INTO team_round_stats (match_id, round, team, side, won, score, equip_value, balance, buy_type, kills, survivors)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    for t in &s.team_rounds {
        st.execute(params![
            m,
            t.round,
            t.team.as_str(),
            t.side,
            t.won,
            t.score,
            t.equip_value,
            t.balance,
            t.buy_type.as_str(),
            t.kills,
            t.survivors
        ])?;
    }
    Ok(())
}

/// Column list of `player_match_stats`, in insert / select order.
const PMS_COLUMNS: &str = "match_id, steamid, stats_version, name, team, start_side, won, rounds_played, rounds_t, rounds_ct,
  kills, deaths, assists, flash_assists, headshot_kills, team_kills, suicides, damage, kast_rounds, rounds_survived, mvps,
  mk2, mk3, mk4, mk5, opening_kills_t, opening_deaths_t, opening_kills_ct, opening_deaths_ct,
  trade_kill_opportunities, trade_kill_attempts, trade_kills, traded_death_opportunities, traded_deaths,
  clutch_1v1_attempts, clutch_1v1_wins, clutch_1v2_attempts, clutch_1v2_wins, clutch_1v3_attempts, clutch_1v3_wins,
  clutch_1v4_attempts, clutch_1v4_wins, clutch_1v5_attempts, clutch_1v5_wins, clutch_kills,
  flashes_thrown, he_thrown, smokes_thrown, molotovs_thrown, decoys_thrown, enemies_flashed, enemy_blind_seconds,
  teammates_flashed, teammate_blind_seconds, he_damage, he_team_damage, fire_damage, unused_utility_value,
  adr, kast, hs_pct, rating";

/// Columns selected by [`row_to_player_stats`] (prefix with a table alias via `pms_select`).
pub fn pms_select(alias: &str) -> String {
    PMS_COLUMNS
        .split(',')
        .map(|c| format!("{alias}.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Number of columns produced by [`pms_select`].
pub fn pms_column_count() -> usize {
    PMS_COLUMNS.split(',').count()
}

/// Map a row selected with [`pms_select`] (starting at column `o`) back to stats; `finalize` is applied.
pub fn row_to_player_stats(r: &Row, o: usize) -> rusqlite::Result<PlayerMatchStats> {
    let g = |i: usize| -> rusqlite::Result<u32> { r.get::<_, i64>(o + i).map(|v| v.max(0) as u32) };
    let won: Option<i64> = r.get(o + 6)?;
    let flashes = g(45)?;
    let enemies_flashed: Option<i64> = r.get(o + 50)?;
    let mut p = PlayerMatchStats {
        steamid: r.get::<_, i64>(o + 1)? as u64,
        name: r.get(o + 3)?,
        team: Team::parse(&r.get::<_, String>(o + 4)?),
        start_side: side(r.get(o + 5)?),
        matches: 1,
        wins: (won == Some(1)) as u32,
        losses: (won == Some(0)) as u32,
        draws: won.is_none() as u32,
        rounds_played: g(7)?,
        rounds_t: g(8)?,
        rounds_ct: g(9)?,
        kills: g(10)?,
        deaths: g(11)?,
        assists: g(12)?,
        flash_assists: g(13)?,
        headshot_kills: g(14)?,
        team_kills: g(15)?,
        suicides: g(16)?,
        damage: g(17)?,
        kast_rounds: g(18)?,
        rounds_survived: g(19)?,
        mvps: g(20)?,
        multi_kills: [0, 0, g(21)?, g(22)?, g(23)?, g(24)?],
        opening_kills_t: g(25)?,
        opening_deaths_t: g(26)?,
        opening_kills_ct: g(27)?,
        opening_deaths_ct: g(28)?,
        trade_kill_opportunities: g(29)?,
        trade_kill_attempts: g(30)?,
        trade_kills: g(31)?,
        traded_death_opportunities: g(32)?,
        traded_deaths: g(33)?,
        clutch_attempts: [0, g(34)?, g(36)?, g(38)?, g(40)?, g(42)?],
        clutch_wins: [0, g(35)?, g(37)?, g(39)?, g(41)?, g(43)?],
        clutch_kills: g(44)?,
        flashes_thrown: flashes,
        he_thrown: g(46)?,
        smokes_thrown: g(47)?,
        molotovs_thrown: g(48)?,
        decoys_thrown: g(49)?,
        blind_flashes: if enemies_flashed.is_some() { flashes } else { 0 },
        enemies_flashed: enemies_flashed.map(|v| v.max(0) as u32),
        enemy_blind_seconds: r.get::<_, Option<f64>>(o + 51)?.map(|v| v as f32),
        teammates_flashed: r.get::<_, Option<i64>>(o + 52)?.map(|v| v.max(0) as u32),
        teammate_blind_seconds: r.get::<_, Option<f64>>(o + 53)?.map(|v| v as f32),
        he_damage: g(54)?,
        he_team_damage: g(55)?,
        fire_damage: g(56)?,
        unused_utility_value: g(57)?,
        derived: Default::default(),
    };
    p.finalize();
    Ok(p)
}

/// Lowest stored `stats_version` of a match (`None` = no stats rows).
pub fn stored_version(db: &Database, m: &str) -> Result<Option<i32>> {
    Ok(db
        .conn
        .query_row("SELECT MIN(stats_version) FROM player_match_stats WHERE match_id = ?", params![m], |r| r.get(0))
        .optional()?
        .flatten())
}

/// Analyzed matches whose stats are missing or outdated.
pub fn list_matches_needing_stats(db: &Database) -> Result<Vec<String>> {
    let mut st = db.conn.prepare(
        "SELECT m.id FROM matches m
         WHERE m.analysis_status = 'done'
           AND COALESCE((SELECT MIN(s.stats_version) FROM player_match_stats s WHERE s.match_id = m.id), 0) < ?1
         ORDER BY m.played_at DESC NULLS LAST, m.discovered_at DESC",
    )?;
    let rows = st.query_map(params![STATS_VERSION], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
}
