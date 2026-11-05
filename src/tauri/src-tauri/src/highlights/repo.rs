//! 读事件表 → `highlights::MatchInput`，写 `highlights` / `match_highlights`，查询与设置。

use crate::database::{Database, Result};
use highlights::{Kind, MatchInput, RULES_VERSION};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

const DEFAULT_TICK_RATE: f32 = 64.0;

fn osid(v: Option<i64>) -> Option<u64> {
    v.map(|x| x as u64)
}

fn oteam(v: Option<i64>) -> Option<u8> {
    v.and_then(|x| u8::try_from(x).ok())
}

fn query<T>(c: &Connection, sql: &str, m: &str, f: impl FnMut(&Row) -> rusqlite::Result<T>) -> Result<Vec<T>> {
    let mut st = c.prepare(sql)?;
    let rows = st.query_map(params![m], f)?;
    Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
}

/// 当前库里这场的 stats 版本（`None` = 还没有统计）。
pub fn stats_version(db: &Database, m: &str) -> Result<Option<i32>> {
    Ok(db
        .conn
        .query_row("SELECT MIN(stats_version) FROM player_match_stats WHERE match_id = ?", params![m], |r| r.get(0))
        .optional()?
        .flatten())
}

fn aim_done(db: &Database, m: &str) -> Result<bool> {
    Ok(db
        .conn
        .query_row("SELECT status = 'done' FROM match_aim WHERE match_id = ?", params![m], |r| r.get::<_, bool>(0))
        .optional()?
        .unwrap_or(false))
}

/// 读出一场的规则输入。没有回合（没解析）时返回 `None`。
pub fn load_input(db: &Database, m: &str) -> Result<Option<MatchInput>> {
    let c = &db.conn;
    let rounds = query(
        c,
        "SELECT number, start_tick, freeze_end_tick, end_tick, winner FROM rounds WHERE match_id = ? ORDER BY number",
        m,
        |r| {
            Ok(highlights::RoundIn {
                number: r.get(0)?,
                start_tick: r.get(1)?,
                freeze_end_tick: r.get(2)?,
                end_tick: r.get(3)?,
                winner: oteam(r.get(4)?),
            })
        },
    )?;
    if rounds.is_empty() {
        return Ok(None);
    }
    let players = query(
        c,
        "SELECT round, steamid, side, kills, died, death_tick, opening_death, traded, clutch_vs, clutch_won,
                utility_thrown, unused_utility_value, swing
         FROM player_round_stats WHERE match_id = ?",
        m,
        |r| {
            Ok(highlights::PlayerRoundIn {
                round: r.get(0)?,
                steamid: r.get::<_, i64>(1)? as u64,
                side: r.get::<_, i64>(2)? as u8,
                kills: r.get(3)?,
                died: r.get(4)?,
                death_tick: r.get(5)?,
                opening_death: r.get(6)?,
                traded: r.get(7)?,
                clutch_vs: r.get(8)?,
                clutch_won: r.get(9)?,
                utility_thrown: r.get(10)?,
                unused_utility_value: r.get(11)?,
                swing: r.get(12)?,
            })
        },
    )?;
    let kills = query(
        c,
        "SELECT round, tick, attacker, victim, assister, weapon, headshot, penetrated, noscope, thrusmoke,
                attacker_blind, attacker_in_air, distance, attacker_team, victim_team
         FROM kills WHERE match_id = ? ORDER BY rowid",
        m,
        |r| {
            Ok(highlights::KillIn {
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
                distance: r.get::<_, Option<f64>>(12)?.map(|d| d as f32),
                attacker_team: oteam(r.get(13)?),
                victim_team: oteam(r.get(14)?),
            })
        },
    )?;
    let damages = query(
        c,
        "SELECT round, tick, attacker, victim, weapon, dmg_health, health_after, attacker_team, victim_team
         FROM damages WHERE match_id = ? ORDER BY rowid",
        m,
        |r| {
            Ok(highlights::DamageIn {
                round: r.get(0)?,
                tick: r.get(1)?,
                attacker: osid(r.get(2)?),
                victim: osid(r.get(3)?),
                weapon: r.get(4)?,
                dmg_health: r.get(5)?,
                health_after: r.get(6)?,
                attacker_team: oteam(r.get(7)?),
                victim_team: oteam(r.get(8)?),
            })
        },
    )?;
    let shots = query(c, "SELECT round, tick, shooter, weapon FROM shots WHERE match_id = ? ORDER BY rowid", m, |r| {
        Ok(highlights::ShotIn {
            round: r.get(0)?,
            tick: r.get(1)?,
            shooter: r.get::<_, i64>(2)? as u64,
            weapon: r.get(3)?,
        })
    })?;
    let blinds = query(
        c,
        "SELECT round, tick, attacker, victim, duration, attacker_team, victim_team FROM blinds WHERE match_id = ? ORDER BY rowid",
        m,
        |r| {
            Ok(highlights::BlindIn {
                round: r.get(0)?,
                tick: r.get(1)?,
                attacker: osid(r.get(2)?),
                victim: osid(r.get(3)?),
                duration: r.get::<_, f64>(4)? as f32,
                attacker_team: oteam(r.get(5)?),
                victim_team: oteam(r.get(6)?),
            })
        },
    )?;
    let bombs = query(c, "SELECT round, tick, player, kind FROM bomb_events WHERE match_id = ? ORDER BY rowid", m, |r| {
        Ok(highlights::BombIn { round: r.get(0)?, tick: r.get(1)?, player: osid(r.get(2)?), kind: r.get(3)? })
    })?;
    let purchases = query(
        c,
        "SELECT round, tick, player, item, cost, was_sold FROM purchases WHERE match_id = ? ORDER BY rowid",
        m,
        |r| {
            Ok(highlights::PurchaseIn {
                round: r.get(0)?,
                tick: r.get(1)?,
                player: r.get::<_, i64>(2)? as u64,
                item: r.get(3)?,
                cost: r.get(4)?,
                was_sold: r.get(5)?,
            })
        },
    )?;
    let win_prob = query(
        c,
        "SELECT round, seq, tick, kind, player, victim, t_alive, ct_alive, p_before, p_after
         FROM round_win_probability WHERE match_id = ? ORDER BY round, seq",
        m,
        |r| {
            Ok(highlights::WinProbIn {
                round: r.get(0)?,
                seq: r.get(1)?,
                tick: r.get(2)?,
                kind: r.get(3)?,
                player: osid(r.get(4)?),
                victim: osid(r.get(5)?),
                t_alive: r.get(6)?,
                ct_alive: r.get(7)?,
                p_before: r.get(8)?,
                p_after: r.get(9)?,
            })
        },
    )?;
    // 可见性不可靠（ESL demo）时没有准星位置 / TTD，甩枪规则不可用
    let vis_reliable: bool = c
        .query_row("SELECT status = 'done' AND visibility_reliable = 1 FROM match_aim WHERE match_id = ?", params![m], |r| r.get(0))
        .optional()?
        .unwrap_or(false);
    let engagements = if vis_reliable {
        Some(query(
            c,
            "SELECT round, observer, target, role, weapon, onset_tick, damage_tick, ttd_ms, cp_deg, killed
             FROM aim_engagements WHERE match_id = ?",
            m,
            |r| {
                Ok(highlights::EngagementIn {
                    round: r.get(0)?,
                    observer: r.get::<_, i64>(1)? as u64,
                    target: r.get::<_, i64>(2)? as u64,
                    role: r.get(3)?,
                    weapon: r.get(4)?,
                    onset_tick: r.get(5)?,
                    damage_tick: r.get(6)?,
                    ttd_ms: r.get(7)?,
                    cp_deg: r.get(8)?,
                    killed: r.get(9)?,
                })
            },
        )?)
    } else {
        None
    };
    let flashes: i64 =
        c.query_row("SELECT COUNT(*) FROM grenades WHERE match_id = ? AND kind = 'flashbang'", params![m], |r| r.get(0))?;
    let blind_data = !blinds.is_empty() || flashes == 0;
    Ok(Some(MatchInput {
        tick_rate: DEFAULT_TICK_RATE,
        rounds,
        players,
        kills,
        damages,
        shots,
        blinds,
        bombs,
        purchases,
        win_prob,
        engagements,
        blind_data,
    }))
}

/// 整场替换集锦并记录计算版本。
pub fn replace_highlights(db: &Database, m: &str, input: &MatchInput, rows: &[highlights::Highlight]) -> Result<()> {
    let sv = stats_version(db, m)?;
    let aim = aim_done(db, m)?;
    let available: Vec<&str> = highlights::available_kinds(input).iter().map(|k| k.as_str()).collect();
    let tx = db.conn.unchecked_transaction()?;
    tx.execute("DELETE FROM highlights WHERE match_id = ?", params![m])?;
    {
        let mut st = tx.prepare(
            "INSERT INTO highlights (match_id, round, start_tick, end_tick, steamid, kind, category, score, detail)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        for h in rows {
            st.execute(params![
                m,
                h.round,
                h.start_tick,
                h.end_tick,
                h.steamid as i64,
                h.kind.as_str(),
                h.kind.category().as_str(),
                h.score,
                h.detail.to_string()
            ])?;
        }
    }
    tx.execute(
        "INSERT INTO match_highlights (match_id, rules_version, stats_version, aim_done, status, error, count, available, computed_at)
         VALUES (?1, ?2, ?3, ?4, 'done', NULL, ?5, ?6, datetime('now'))
         ON CONFLICT(match_id) DO UPDATE SET rules_version = ?2, stats_version = ?3, aim_done = ?4, status = 'done',
           error = NULL, count = ?5, available = ?6, computed_at = datetime('now')",
        params![m, RULES_VERSION, sv, aim, rows.len() as i64, serde_json::to_string(&available).unwrap_or_default()],
    )?;
    tx.commit()?;
    Ok(())
}

/// 记录失败（带上当时的 stats / aim 状态，避免回填反复重试同一个失败）。
pub fn mark_failed(db: &Database, m: &str, error: &str) -> Result<()> {
    let sv = stats_version(db, m)?;
    let aim = aim_done(db, m)?;
    db.conn.execute(
        "INSERT INTO match_highlights (match_id, rules_version, stats_version, aim_done, status, error, computed_at)
         VALUES (?1, ?2, ?3, ?4, 'failed', ?5, datetime('now'))
         ON CONFLICT(match_id) DO UPDATE SET rules_version = ?2, stats_version = ?3, aim_done = ?4, status = 'failed',
           error = ?5, computed_at = datetime('now')",
        params![m, RULES_VERSION, sv, aim, error],
    )?;
    Ok(())
}

/// 需要（重新）计算的对局：已解析、已有统计，且没算过 / 规则版本旧 / stats 重算过 / 枪法数据后来才有。
pub fn list_matches_needing(db: &Database) -> Result<Vec<String>> {
    let mut st = db.conn.prepare(
        "SELECT m.id FROM matches m
         JOIN (SELECT match_id, MIN(stats_version) AS sv FROM player_match_stats GROUP BY match_id) s ON s.match_id = m.id
         LEFT JOIN match_highlights h ON h.match_id = m.id
         LEFT JOIN match_aim a ON a.match_id = m.id
         WHERE m.analysis_status = 'done'
           AND (h.match_id IS NULL
                OR h.rules_version < ?1
                OR h.stats_version IS NOT s.sv
                OR (h.aim_done = 0 AND a.status = 'done'))
         ORDER BY m.played_at DESC NULLS LAST, m.discovered_at DESC",
    )?;
    let rows = st.query_map(params![RULES_VERSION], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
}

// ---------------------------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSetting {
    pub kind: String,
    #[serde(default)]
    pub category: String,
    pub enabled: bool,
    pub weight: f64,
    #[serde(default)]
    pub default_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightSettings {
    pub rules: Vec<RuleSetting>,
    /// 提前多少秒开始播放（`demo_gototick` 的 tick = 起点 − lead × 64）。
    pub lead_secs: i64,
    /// 实验：通过 `-netconport` 自动跳转。
    pub netcon_enabled: bool,
    pub netcon_port: i64,
    pub extra_launch_args: String,
}

pub fn get_settings(db: &Database) -> Result<HighlightSettings> {
    let mut user: HashMap<String, (bool, f64)> = HashMap::new();
    {
        let mut st = db.conn.prepare("SELECT kind, enabled, weight FROM highlight_rules")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, f64>(2)?)))?;
        for r in rows {
            let (k, e, w) = r?;
            user.insert(k, (e, w));
        }
    }
    let rules = Kind::ALL
        .iter()
        .map(|k| {
            let (enabled, weight) = user.get(k.as_str()).copied().unwrap_or((true, k.default_weight()));
            RuleSetting {
                kind: k.as_str().into(),
                category: k.category().as_str().into(),
                enabled,
                weight,
                default_weight: k.default_weight(),
            }
        })
        .collect();
    let (lead_secs, netcon_enabled, netcon_port, extra_launch_args) = db.conn.query_row(
        "SELECT lead_secs, netcon_enabled, netcon_port, extra_launch_args FROM highlight_settings WHERE id = 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    Ok(HighlightSettings { rules, lead_secs, netcon_enabled, netcon_port, extra_launch_args })
}

pub fn update_settings(db: &Database, s: &HighlightSettings) -> Result<()> {
    let tx = db.conn.unchecked_transaction()?;
    for r in &s.rules {
        let Some(k) = Kind::parse(&r.kind) else { continue };
        let weight = if r.weight.is_finite() { r.weight.clamp(0.0, 3.0) } else { k.default_weight() };
        if r.enabled && (weight - k.default_weight()).abs() < 1e-9 {
            tx.execute("DELETE FROM highlight_rules WHERE kind = ?", params![k.as_str()])?;
        } else {
            tx.execute(
                "INSERT INTO highlight_rules (kind, enabled, weight) VALUES (?1, ?2, ?3)
                 ON CONFLICT(kind) DO UPDATE SET enabled = ?2, weight = ?3",
                params![k.as_str(), r.enabled, weight],
            )?;
        }
    }
    tx.execute(
        "UPDATE highlight_settings SET lead_secs = ?1, netcon_enabled = ?2, netcon_port = ?3, extra_launch_args = ?4,
           updated_at = datetime('now') WHERE id = 1",
        params![
            s.lead_secs.clamp(0, 30),
            s.netcon_enabled,
            s.netcon_port.clamp(1024, 65535),
            s.extra_launch_args.trim()
        ],
    )?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// 查询
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HighlightFilters {
    /// 十进制 steamid；不传 = 所有选手
    pub steamid: Option<String>,
    pub match_id: Option<String>,
    /// `highlight` | `mistake`
    pub category: Option<String>,
    pub kinds: Option<Vec<String>>,
    pub maps: Option<Vec<String>>,
    /// 最近 N 天
    pub days: Option<i64>,
    /// ISO 日期（含）
    pub from: Option<String>,
    pub to: Option<String>,
    /// 默认 500，最多 5000
    pub limit: Option<usize>,
    /// 包含设置里关掉的规则
    pub include_disabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightItem {
    pub id: i64,
    pub match_id: String,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
    pub platform: String,
    pub round: i64,
    pub start_tick: i64,
    pub end_tick: i64,
    pub steamid: String,
    pub player_name: Option<String>,
    pub side: Option<i64>,
    pub kind: String,
    pub category: String,
    pub score: f64,
    pub weighted_score: f64,
    pub detail: serde_json::Value,
    /// raw | compressed | deleted
    pub demo_storage: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindCount {
    pub kind: String,
    pub category: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightList {
    pub items: Vec<HighlightItem>,
    /// 符合筛选条件的总数（items 受 limit 限制）
    pub total: i64,
    /// 按类型计数（忽略类型 / 高光失误筛选，用于筛选按钮上的数字）
    pub counts: Vec<KindCount>,
    /// 涉及的对局数
    pub matches: i64,
    /// 可选的地图（该选手所有有集锦的对局）
    pub maps: Vec<String>,
    /// 已解析但集锦还没算出来的对局数（后台回填中）
    pub pending: i64,
    /// steamid → 名字（细节里出现的人，取最近一场的名字）
    pub names: BTreeMap<String, String>,
}

struct Where {
    sql: Vec<String>,
    args: Vec<rusqlite::types::Value>,
}

impl Where {
    fn push(&mut self, sql: impl Into<String>, args: impl IntoIterator<Item = rusqlite::types::Value>) {
        self.sql.push(sql.into());
        self.args.extend(args);
    }
    fn clause(&self) -> String {
        if self.sql.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", self.sql.join(" AND "))
        }
    }
}

fn base_where(f: &HighlightFilters, disabled: &[String], with_kind: bool) -> std::result::Result<Where, String> {
    use rusqlite::types::Value as V;
    let mut w = Where { sql: vec![], args: vec![] };
    if let Some(s) = f.steamid.as_deref().filter(|s| !s.trim().is_empty()) {
        let id: i64 = s.trim().parse::<u64>().map_err(|_| format!("无效的 steamid: {s}"))? as i64;
        w.push("h.steamid = ?", [V::Integer(id)]);
    }
    if let Some(m) = f.match_id.as_deref().filter(|s| !s.is_empty()) {
        w.push("h.match_id = ?", [V::Text(m.into())]);
    }
    if let Some(maps) = f.maps.as_ref().filter(|v| !v.is_empty()) {
        let qs = vec!["?"; maps.len()].join(",");
        w.push(format!("m.map_name IN ({qs})"), maps.iter().map(|s| V::Text(s.clone())));
    }
    let date = "datetime(COALESCE(m.played_at, m.discovered_at))";
    if let Some(d) = f.days.filter(|d| *d > 0) {
        w.push(format!("{date} >= datetime('now', ?)"), [V::Text(format!("-{d} days"))]);
    }
    if let Some(from) = f.from.as_deref().filter(|s| !s.is_empty()) {
        w.push(format!("{date} >= datetime(?)"), [V::Text(from.into())]);
    }
    if let Some(to) = f.to.as_deref().filter(|s| !s.is_empty()) {
        // 只给日期时包含当天
        let to = if to.len() == 10 { format!("{to} 23:59:59") } else { to.to_string() };
        w.push(format!("{date} <= datetime(?)"), [V::Text(to)]);
    }
    if !f.include_disabled && !disabled.is_empty() {
        let qs = vec!["?"; disabled.len()].join(",");
        w.push(format!("h.kind NOT IN ({qs})"), disabled.iter().map(|s| V::Text(s.clone())));
    }
    if with_kind {
        if let Some(c) = f.category.as_deref().filter(|c| *c == "highlight" || *c == "mistake") {
            w.push("h.category = ?", [V::Text(c.into())]);
        }
        if let Some(kinds) = f.kinds.as_ref().filter(|v| !v.is_empty()) {
            let qs = vec!["?"; kinds.len()].join(",");
            w.push(format!("h.kind IN ({qs})"), kinds.iter().map(|s| V::Text(s.clone())));
        }
    }
    Ok(w)
}

pub fn list_highlights(db: &Database, f: &HighlightFilters) -> std::result::Result<HighlightList, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let settings = get_settings(db).map_err(|e| e.to_string())?;
    let weights: HashMap<String, f64> = settings.rules.iter().map(|r| (r.kind.clone(), r.weight)).collect();
    let disabled: Vec<String> = settings.rules.iter().filter(|r| !r.enabled).map(|r| r.kind.clone()).collect();
    let c = &db.conn;
    let from = "FROM highlights h JOIN matches m ON m.id = h.match_id";

    // 权重表达式：CASE kind WHEN … THEN weight … ELSE 1 END
    let mut weight_sql = String::from("(CASE h.kind");
    for (k, wt) in &weights {
        // kind 来自 Kind::ALL，安全
        weight_sql.push_str(&format!(" WHEN '{k}' THEN {wt}"));
    }
    weight_sql.push_str(" ELSE 1.0 END)");

    let w = base_where(f, &disabled, true)?;
    let (total, matches): (i64, i64) = c
        .query_row(
            &format!("SELECT COUNT(*), COUNT(DISTINCT h.match_id) {from} {}", w.clause()),
            params_from_iter(w.args.iter()),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(e)?;

    let limit = f.limit.unwrap_or(500).clamp(1, 5000);
    let sql = format!(
        "SELECT h.id, h.match_id, m.map_name, COALESCE(m.played_at, m.discovered_at), m.platform, h.round, h.start_tick,
                h.end_tick, h.steamid, mp.name, prs.side, h.kind, h.category, h.score, h.score * {weight_sql} AS ws,
                h.detail, m.demo_storage
         {from}
         LEFT JOIN match_players mp ON mp.match_id = h.match_id AND mp.steamid = h.steamid
         LEFT JOIN player_round_stats prs ON prs.match_id = h.match_id AND prs.round = h.round AND prs.steamid = h.steamid
         {}
         ORDER BY {} LIMIT {limit}",
        w.clause(),
        if f.match_id.is_some() { "h.round, h.start_tick, ws DESC" } else { "ws DESC, COALESCE(m.played_at, m.discovered_at) DESC, h.round" }
    );
    let mut st = c.prepare(&sql).map_err(e)?;
    let items = st
        .query_map(params_from_iter(w.args.iter()), |r| {
            let detail: String = r.get(15)?;
            Ok(HighlightItem {
                id: r.get(0)?,
                match_id: r.get(1)?,
                map_name: r.get(2)?,
                played_at: r.get(3)?,
                platform: r.get(4)?,
                round: r.get(5)?,
                start_tick: r.get(6)?,
                end_tick: r.get(7)?,
                steamid: (r.get::<_, i64>(8)? as u64).to_string(),
                player_name: r.get(9)?,
                side: r.get(10)?,
                kind: r.get(11)?,
                category: r.get(12)?,
                score: r.get(13)?,
                weighted_score: (r.get::<_, f64>(14)? * 10.0).round() / 10.0,
                detail: serde_json::from_str(&detail).unwrap_or(serde_json::Value::Null),
                demo_storage: r.get(16)?,
            })
        })
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;

    // 按类型计数：只受选手 / 对局 / 地图 / 时间约束
    let wc = base_where(f, &disabled, false)?;
    let mut st = c
        .prepare(&format!("SELECT h.kind, h.category, COUNT(*) {from} {} GROUP BY h.kind, h.category", wc.clause()))
        .map_err(e)?;
    let mut counts: Vec<KindCount> = st
        .query_map(params_from_iter(wc.args.iter()), |r| Ok(KindCount { kind: r.get(0)?, category: r.get(1)?, count: r.get(2)? }))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(e)?;
    let order = |k: &str| Kind::ALL.iter().position(|x| x.as_str() == k).unwrap_or(usize::MAX);
    counts.sort_by_key(|c| order(&c.kind));

    // 可选地图：该选手（或全部）有集锦的对局
    let mut maps_where = Where { sql: vec![], args: vec![] };
    if let Some(s) = f.steamid.as_deref().and_then(|s| s.trim().parse::<u64>().ok()) {
        maps_where.push("h.steamid = ?", [rusqlite::types::Value::Integer(s as i64)]);
    }
    let mut st = c
        .prepare(&format!(
            "SELECT DISTINCT m.map_name {from} {} {} m.map_name IS NOT NULL ORDER BY m.map_name",
            maps_where.clause(),
            if maps_where.sql.is_empty() { "WHERE" } else { "AND" }
        ))
        .map_err(e)?;
    let maps = st
        .query_map(params_from_iter(maps_where.args.iter()), |r| r.get(0))
        .map_err(e)?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(e)?;

    // 还在回填中的对局（只算这名选手出场的）
    let pending: i64 = match f.steamid.as_deref().and_then(|s| s.trim().parse::<u64>().ok()) {
        Some(s) => c
            .query_row(
                "SELECT COUNT(*) FROM matches m JOIN match_players mp ON mp.match_id = m.id AND mp.steamid = ?
                 LEFT JOIN match_highlights h ON h.match_id = m.id
                 WHERE m.analysis_status = 'done' AND (h.match_id IS NULL OR h.rules_version < ?)",
                params![s as i64, RULES_VERSION],
                |r| r.get(0),
            )
            .map_err(e)?,
        None => c
            .query_row(
                "SELECT COUNT(*) FROM matches m LEFT JOIN match_highlights h ON h.match_id = m.id
                 WHERE m.analysis_status = 'done' AND (h.match_id IS NULL OR h.rules_version < ?)",
                params![RULES_VERSION],
                |r| r.get(0),
            )
            .map_err(e)?,
    };

    // 名字：细节里出现的 steamid + 主角
    let mut want: Vec<String> = Vec::new();
    for it in &items {
        want.push(it.steamid.clone());
        collect_sids(&it.detail, &mut want);
    }
    want.sort();
    want.dedup();
    let match_ids: Vec<String> = {
        let mut v: Vec<String> = items.iter().map(|i| i.match_id.clone()).collect();
        v.sort();
        v.dedup();
        v
    };
    let mut names = BTreeMap::new();
    if !want.is_empty() && !match_ids.is_empty() {
        let qs = vec!["?"; match_ids.len()].join(",");
        let mut st = c
            .prepare(&format!(
                "SELECT mp.steamid, mp.name FROM match_players mp JOIN matches m ON m.id = mp.match_id
                 WHERE mp.match_id IN ({qs}) ORDER BY COALESCE(m.played_at, m.discovered_at)"
            ))
            .map_err(e)?;
        let rows = st
            .query_map(params_from_iter(match_ids.iter()), |r| Ok(((r.get::<_, i64>(0)? as u64).to_string(), r.get::<_, String>(1)?)))
            .map_err(e)?;
        for r in rows {
            let (s, n) = r.map_err(e)?;
            if want.binary_search(&s).is_ok() {
                names.insert(s, n); // 后面的（更近的）覆盖前面的
            }
        }
    }

    Ok(HighlightList { items, total, counts, matches, maps, pending, names })
}

/// 细节 JSON 里看起来像 SteamID64 的字符串。
fn collect_sids(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) if s.len() == 17 && s.starts_with("765") && s.bytes().all(|b| b.is_ascii_digit()) => {
            out.push(s.clone())
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_sids(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| collect_sids(x, out)),
        _ => {}
    }
}

/// 播放需要的信息。
pub struct PlaybackRow {
    pub match_id: String,
    pub start_tick: i64,
    pub round_start_tick: Option<i64>,
    pub file_path: Option<String>,
    pub demo_storage: String,
    pub demo_original_size: Option<i64>,
}

pub fn get_playback_row(db: &Database, id: i64) -> Result<Option<PlaybackRow>> {
    Ok(db
        .conn
        .query_row(
            "SELECT h.match_id, h.start_tick, r.start_tick, m.file_path, m.demo_storage, m.demo_original_size
             FROM highlights h JOIN matches m ON m.id = h.match_id
             LEFT JOIN rounds r ON r.match_id = h.match_id AND r.number = h.round
             WHERE h.id = ?",
            params![id],
            |r| {
                Ok(PlaybackRow {
                    match_id: r.get(0)?,
                    start_tick: r.get(1)?,
                    round_start_tick: r.get(2)?,
                    file_path: r.get(3)?,
                    demo_storage: r.get(4)?,
                    demo_original_size: r.get(5)?,
                })
            },
        )
        .optional()?)
}
