//! 读接口：人员摘要 / 详情、交手、组排队友。所有 steamid 在 JSON 里都是字符串（超出 JS 安全整数）。

use super::manage::parse_tags;
use crate::database::{DbError, Result};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonSummary {
    pub id: i64,
    /// 手动名 > 最近一场的游戏名 > 平台账号名
    pub display_name: String,
    pub custom_name: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub followed: bool,
    pub user_id: Option<String>,
    pub merged_into: Option<i64>,
    pub steamids: Vec<String>,
    pub match_count: i64,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDto {
    pub id: i64,
    pub person_id: i64,
    pub platform: String,
    pub external_id: String,
    pub steamid: Option<String>,
    pub sub_account_id: Option<String>,
    pub last_name: Option<String>,
    pub match_count: i64,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub manual: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AliasDto {
    pub name: String,
    pub steamid: String,
    pub matches: i64,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MergeDto {
    pub id: i64,
    pub target_id: i64,
    pub source_id: i64,
    pub identity_ids: Vec<i64>,
    pub auto: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordDto {
    pub matches: i64,
    pub wins: i64,
    pub losses: i64,
    pub ties: i64,
    /// wins / matches，没有场次时为 null
    pub win_rate: Option<f64>,
}

impl RecordDto {
    fn new(matches: i64, wins: i64, losses: i64) -> Self {
        Self {
            matches,
            wins,
            losses,
            ties: matches - wins - losses,
            win_rate: (matches > 0).then(|| wins as f64 / matches as f64),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonDetail {
    pub person: PersonSummary,
    pub identities: Vec<IdentityDto>,
    pub aliases: Vec<AliasDto>,
    /// 并进这个人且还能撤销的合并（新的在前）
    pub merges: Vec<MergeDto>,
    pub record: RecordDto,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DuelDto {
    pub total: i64,
    pub won: i64,
    pub lost: i64,
    /// won / (won + lost)
    pub win_rate: Option<f64>,
}

impl DuelDto {
    fn new(total: i64, won: i64, lost: i64) -> Self {
        Self { total, won, lost, win_rate: (won + lost > 0).then(|| won as f64 / (won + lost) as f64) }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct H2hMatchDto {
    pub match_id: String,
    pub played_at: Option<String>,
    pub map_name: Option<String>,
    pub platform: String,
    pub same_team: bool,
    /// a 的胜负：1 / 0 / -1
    pub result: i64,
    pub kills: i64,
    pub deaths: i64,
}

/// 所有数字都是 a 视角。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeadToHead {
    pub a: PersonSummary,
    pub b: PersonSummary,
    pub together: RecordDto,
    pub against: RecordDto,
    pub kills: i64,
    pub deaths: i64,
    pub damage: i64,
    pub damage_taken: i64,
    /// 每回合两人第一次交火
    pub duels: DuelDto,
    /// 回合首杀发生在两人之间
    pub openings: DuelDto,
    /// 最近 50 场同场对局
    pub matches: Vec<H2hMatchDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeammateDto {
    pub person: PersonSummary,
    pub record: RecordDto,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeammatesDto {
    pub person: PersonSummary,
    /// 这个人的总战绩，用来和组排胜率对比
    pub overall: RecordDto,
    pub teammates: Vec<TeammateDto>,
}

fn err(msg: impl Into<String>) -> DbError {
    DbError::Message(msg.into())
}

/// 合并链的终点（被合并的人跳到合并后的人）
pub fn resolve(c: &Connection, id: i64) -> Result<i64> {
    let mut cur = id;
    for _ in 0..64 {
        let next: Option<i64> = c
            .query_row("SELECT merged_into FROM persons WHERE id = ?", params![cur], |r| r.get(0))
            .optional()?
            .ok_or_else(|| err(format!("人员 {id} 不存在")))?;
        match next {
            Some(n) => cur = n,
            None => return Ok(cur),
        }
    }
    Err(err("合并链过长"))
}

/// 一个人名下 steam 身份的 steamid（对局数据只通过它关联）
pub fn person_steamids(c: &Connection, id: i64) -> Result<Vec<i64>> {
    let mut st = c.prepare(
        "SELECT steamid FROM identities WHERE person_id = ? AND platform = 'steam' AND steamid IS NOT NULL ORDER BY steamid",
    )?;
    let rows = st.query_map(params![id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

pub fn person_summary(c: &Connection, id: i64) -> Result<PersonSummary> {
    let row = c
        .query_row(
            "SELECT p.display_name, p.note, p.tags, p.followed, p.user_id, p.merged_into,
               (SELECT i.last_name FROM identities i WHERE i.person_id = p.id AND i.last_name IS NOT NULL
                ORDER BY i.platform = 'steam' DESC, i.last_seen_at DESC NULLS LAST, i.match_count DESC LIMIT 1),
               (SELECT i.external_id FROM identities i WHERE i.person_id = p.id ORDER BY i.id LIMIT 1),
               (SELECT COALESCE(SUM(i.match_count), 0) FROM identities i WHERE i.person_id = p.id AND i.platform = 'steam'),
               (SELECT MAX(i.last_seen_at) FROM identities i WHERE i.person_id = p.id)
             FROM persons p WHERE p.id = ?",
            params![id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| err(format!("人员 {id} 不存在")))?;
    let (custom, note, tags, followed, user_id, merged_into, last_name, ext, match_count, last_seen_at) = row;
    let display_name = custom.clone().or(last_name).or(ext).unwrap_or_else(|| format!("#{id}"));
    Ok(PersonSummary {
        id,
        display_name,
        custom_name: custom,
        note,
        tags: parse_tags(&tags),
        followed,
        user_id,
        merged_into,
        steamids: person_steamids(c, id)?.into_iter().map(|s| s.to_string()).collect(),
        match_count,
        last_seen_at,
    })
}

fn record_for(c: &Connection, steamids: &[i64]) -> Result<RecordDto> {
    if steamids.is_empty() {
        return Ok(RecordDto::default());
    }
    let sql = format!(
        "SELECT COUNT(*), COALESCE(SUM(result = 1), 0), COALESCE(SUM(result = -1), 0)
         FROM h2h_player_results WHERE steamid IN ({})",
        placeholders(steamids.len())
    );
    let (m, w, l) = c.query_row(&sql, params_from_iter(steamids), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(RecordDto::new(m, w, l))
}

pub fn get_person(c: &Connection, id: i64) -> Result<PersonDetail> {
    let person = person_summary(c, id)?;
    let identities = {
        let mut st = c.prepare(
            "SELECT id, person_id, platform, external_id, steamid, sub_account_id, last_name, match_count,
               first_seen_at, last_seen_at, manual
             FROM identities WHERE person_id = ? ORDER BY platform = 'steam' DESC, match_count DESC, id",
        )?;
        let rows = st.query_map(params![id], |r| {
            Ok(IdentityDto {
                id: r.get(0)?,
                person_id: r.get(1)?,
                platform: r.get(2)?,
                external_id: r.get(3)?,
                steamid: r.get::<_, Option<i64>>(4)?.map(|s| s.to_string()),
                sub_account_id: r.get(5)?,
                last_name: r.get(6)?,
                match_count: r.get(7)?,
                first_seen_at: r.get(8)?,
                last_seen_at: r.get(9)?,
                manual: r.get(10)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let steamids = person_steamids(c, id)?;
    let aliases = if steamids.is_empty() {
        vec![]
    } else {
        let sql = format!(
            "SELECT mp.name, mp.steamid, COUNT(*), MAX(COALESCE(m.played_at, m.analyzed_at)) AS last
             FROM match_players mp JOIN matches m ON m.id = mp.match_id
             WHERE mp.steamid IN ({}) GROUP BY mp.name, mp.steamid ORDER BY last DESC NULLS LAST, mp.name",
            placeholders(steamids.len())
        );
        let mut st = c.prepare(&sql)?;
        let rows = st.query_map(params_from_iter(&steamids), |r| {
            Ok(AliasDto {
                name: r.get(0)?,
                steamid: r.get::<_, i64>(1)?.to_string(),
                matches: r.get(2)?,
                last_seen_at: r.get(3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let merges = {
        let mut st = c.prepare(
            "SELECT id, target_id, source_id, identity_ids, auto, created_at FROM person_merges
             WHERE target_id = ? AND undone_at IS NULL ORDER BY id DESC",
        )?;
        let rows = st.query_map(params![id], |r| {
            Ok(MergeDto {
                id: r.get(0)?,
                target_id: r.get(1)?,
                source_id: r.get(2)?,
                identity_ids: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                auto: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let record = record_for(c, &steamids)?;
    Ok(PersonDetail { person, identities, aliases, merges, record })
}

/// 搜索：显示名、备注、标签、平台 id、steamid、历史游戏名。空查询返回全部（按出现场数排序）。
pub fn search_persons(c: &Connection, query: Option<&str>, followed_only: bool, limit: i64) -> Result<Vec<PersonSummary>> {
    let q = query.map(str::trim).filter(|q| !q.is_empty());
    let pat = q.map(|q| format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let mut st = c.prepare(
        "SELECT p.id FROM persons p
         WHERE p.merged_into IS NULL
           AND (?3 = 0 OR p.followed = 1)
           AND (?1 IS NULL
             OR p.display_name LIKE ?2 ESCAPE '\\' OR p.note LIKE ?2 ESCAPE '\\' OR p.tags LIKE ?2 ESCAPE '\\'
             OR EXISTS (SELECT 1 FROM identities i WHERE i.person_id = p.id
                        AND (i.external_id LIKE ?2 ESCAPE '\\' OR i.last_name LIKE ?2 ESCAPE '\\'))
             OR EXISTS (SELECT 1 FROM identities i JOIN match_players mp ON mp.steamid = i.steamid
                        WHERE i.person_id = p.id AND i.platform = 'steam' AND mp.name LIKE ?2 ESCAPE '\\'))
         ORDER BY p.followed DESC,
           (SELECT COALESCE(SUM(i.match_count), 0) FROM identities i WHERE i.person_id = p.id AND i.platform = 'steam') DESC,
           p.id
         LIMIT ?4",
    )?;
    let ids: Vec<i64> = st
        .query_map(params![q, pat, followed_only, limit], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    ids.into_iter().map(|id| person_summary(c, id)).collect()
}

pub fn get_head_to_head(c: &Connection, a: i64, b: i64) -> Result<HeadToHead> {
    let (a, b) = (resolve(c, a)?, resolve(c, b)?);
    if a == b {
        return Err(err("同一个人没有交手数据"));
    }
    let (sa, sb) = (person_steamids(c, a)?, person_steamids(c, b)?);
    let (pa, pb) = (person_summary(c, a)?, person_summary(c, b)?);
    let empty = HeadToHead {
        a: pa.clone(),
        b: pb.clone(),
        together: RecordDto::default(),
        against: RecordDto::default(),
        kills: 0,
        deaths: 0,
        damage: 0,
        damage_taken: 0,
        duels: DuelDto::default(),
        openings: DuelDto::default(),
        matches: vec![],
    };
    if sa.is_empty() || sb.is_empty() {
        return Ok(empty);
    }
    let cond = format!("a IN ({}) AND b IN ({})", placeholders(sa.len()), placeholders(sb.len()));
    let args: Vec<i64> = sa.iter().chain(sb.iter()).copied().collect();
    let sql = format!(
        "SELECT COALESCE(SUM(matches_together), 0), COALESCE(SUM(wins_together), 0), COALESCE(SUM(losses_together), 0),
           COALESCE(SUM(matches_against), 0), COALESCE(SUM(wins_against), 0), COALESCE(SUM(losses_against), 0),
           COALESCE(SUM(kills), 0), COALESCE(SUM(deaths), 0), COALESCE(SUM(damage), 0), COALESCE(SUM(damage_taken), 0),
           COALESCE(SUM(duels), 0), COALESCE(SUM(duels_won), 0), COALESCE(SUM(duels_lost), 0),
           COALESCE(SUM(opening_wins), 0), COALESCE(SUM(opening_losses), 0)
         FROM h2h_stats WHERE {cond}"
    );
    let v: [i64; 15] = c.query_row(&sql, params_from_iter(&args), |r| {
        let mut v = [0i64; 15];
        for (i, x) in v.iter_mut().enumerate() {
            *x = r.get(i)?;
        }
        Ok(v)
    })?;
    let sql = format!(
        "SELECT h.match_id, m.played_at, m.map_name, m.platform, h.same_team, h.result, h.kills, h.deaths
         FROM h2h_match h JOIN matches m ON m.id = h.match_id
         WHERE {} ORDER BY COALESCE(m.played_at, m.analyzed_at) DESC NULLS LAST LIMIT 50",
        cond.replace("a IN", "h.a IN").replace("b IN", "h.b IN")
    );
    let mut st = c.prepare(&sql)?;
    let matches = st
        .query_map(params_from_iter(&args), |r| {
            Ok(H2hMatchDto {
                match_id: r.get(0)?,
                played_at: r.get(1)?,
                map_name: r.get(2)?,
                platform: r.get(3)?,
                same_team: r.get(4)?,
                result: r.get(5)?,
                kills: r.get(6)?,
                deaths: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(HeadToHead {
        together: RecordDto::new(v[0], v[1], v[2]),
        against: RecordDto::new(v[3], v[4], v[5]),
        kills: v[6],
        deaths: v[7],
        damage: v[8],
        damage_taken: v[9],
        duels: DuelDto::new(v[10], v[11], v[12]),
        openings: DuelDto::new(v[13] + v[14], v[13], v[14]),
        matches,
        ..empty
    })
}

/// 常一起组排的队友：同队场次 ≥ min_matches，按场次排序。
pub fn list_teammates(c: &Connection, id: i64, min_matches: i64, limit: i64) -> Result<TeammatesDto> {
    let id = resolve(c, id)?;
    let person = person_summary(c, id)?;
    let sa = person_steamids(c, id)?;
    let overall = record_for(c, &sa)?;
    if sa.is_empty() {
        return Ok(TeammatesDto { person, overall, teammates: vec![] });
    }
    let sql = format!(
        "SELECT i.person_id, SUM(s.matches_together) AS n, SUM(s.wins_together), SUM(s.losses_together)
         FROM h2h_stats s JOIN identities i ON i.platform = 'steam' AND i.steamid = s.b
         WHERE s.a IN ({}) AND s.matches_together > 0 AND i.person_id <> ?
         GROUP BY i.person_id HAVING n >= ? ORDER BY n DESC, SUM(s.wins_together) DESC LIMIT ?",
        placeholders(sa.len())
    );
    let mut args: Vec<i64> = sa.clone();
    args.extend([id, min_matches.max(1), limit]);
    let mut st = c.prepare(&sql)?;
    let rows: Vec<(i64, i64, i64, i64)> = st
        .query_map(params_from_iter(&args), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let teammates = rows
        .into_iter()
        .map(|(p, n, w, l)| Ok(TeammateDto { person: person_summary(c, p)?, record: RecordDto::new(n, w, l) }))
        .collect::<Result<Vec<_>>>()?;
    Ok(TeammatesDto { person, overall, teammates })
}
