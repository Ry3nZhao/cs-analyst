//! 主玩家（「我」）：读写 `app_settings.main_player_steamid`，没设置时推测一个候选人。
//!
//! 推测只给建议，不写入。来源按可信度排序：
//! 1. `valve`：已绑定的 Valve 子账号（external_id 就是 Steam64）
//! 2. `account`：其他平台子账号关联到的 steamid（W1-C 自动关联）
//! 3. `followed`：「我关注的人」里出场最多的 steam 身份
//! 4. `frequent`：已解析对局里出场最多的人（至少 2 场，且明显多于第二名）

use crate::database::Result;
use crate::identity::link::parse_steamid;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MainPlayerSuggestion {
    pub steamid: String,
    pub display_name: Option<String>,
    pub person_id: Option<i64>,
    /// valve | account | followed | frequent
    pub source: String,
    pub match_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MainPlayer {
    /// 十进制字符串；None = 未设置
    pub steamid: Option<String>,
    pub display_name: Option<String>,
    pub person_id: Option<i64>,
    /// 已解析对局里的出场次数
    pub match_count: i64,
    /// 只在未设置时给出
    pub suggestion: Option<MainPlayerSuggestion>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCandidate {
    pub steamid: String,
    pub name: String,
    pub person_id: Option<i64>,
    pub match_count: i64,
    pub last_seen_at: Option<String>,
    /// 是否「我关注的人」
    pub followed: bool,
}

/// 接受 Steam64、`https://steamcommunity.com/profiles/<id>`、`[U:1:n]`、`STEAM_X:Y:Z`。
pub fn parse_steam_input(input: &str) -> Option<i64> {
    let s = input.trim().trim_end_matches('/');
    if s.is_empty() {
        return None;
    }
    if let Some(v) = parse_steamid(s) {
        return Some(v);
    }
    if let Some(pos) = s.find("/profiles/") {
        let rest = &s[pos + "/profiles/".len()..];
        let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return parse_steamid(&id);
    }
    const BASE: i64 = 76561197960265728;
    let upper = s.to_ascii_uppercase();
    if let Some(inner) = upper.strip_prefix("[U:1:").and_then(|r| r.strip_suffix(']')) {
        let account: i64 = inner.parse().ok()?;
        return parse_steamid(&(BASE + account).to_string());
    }
    if let Some(rest) = upper.strip_prefix("STEAM_") {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() == 3 {
            let y: i64 = parts[1].parse().ok()?;
            let z: i64 = parts[2].parse().ok()?;
            return parse_steamid(&(BASE + z * 2 + y).to_string());
        }
    }
    None
}

pub fn get_steamid(c: &Connection) -> Result<Option<i64>> {
    Ok(c.query_row("SELECT main_player_steamid FROM app_settings WHERE id = 1", [], |r| r.get(0))?)
}

pub fn set_steamid(c: &Connection, steamid: Option<i64>) -> Result<()> {
    c.execute(
        "UPDATE app_settings SET main_player_steamid = ?1, updated_at = datetime('now') WHERE id = 1",
        params![steamid],
    )?;
    Ok(())
}

/// steamid → (person id, 显示名, 已解析场次)。显示名：手动名 > 身份最近的名字 > 最近一场里的名字。
pub fn describe(c: &Connection, steamid: i64) -> Result<(Option<i64>, Option<String>, i64)> {
    let person: Option<(i64, Option<String>, Option<String>)> = c
        .query_row(
            "SELECT p.id, p.display_name, i.last_name FROM identities i JOIN persons p ON p.id = i.person_id
             WHERE i.platform = 'steam' AND i.steamid = ? LIMIT 1",
            params![steamid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let last_name: Option<String> = c
        .query_row(
            "SELECT mp.name FROM match_players mp JOIN matches m ON m.id = mp.match_id
             WHERE mp.steamid = ? ORDER BY COALESCE(m.played_at, m.discovered_at) DESC LIMIT 1",
            params![steamid],
            |r| r.get(0),
        )
        .optional()?;
    let count: i64 = c.query_row(
        "SELECT COUNT(DISTINCT match_id) FROM match_players WHERE steamid = ?",
        params![steamid],
        |r| r.get(0),
    )?;
    let (person_id, custom, ident_name) = match person {
        Some((id, custom, name)) => (Some(id), custom, name),
        None => (None, None, None),
    };
    let name = custom
        .filter(|s| !s.trim().is_empty())
        .or(ident_name.filter(|s| !s.trim().is_empty()))
        .or(last_name);
    Ok((person_id, name, count))
}

pub fn get(c: &Connection) -> Result<MainPlayer> {
    match get_steamid(c)? {
        Some(id) => {
            let (person_id, display_name, match_count) = describe(c, id)?;
            Ok(MainPlayer { steamid: Some(id.to_string()), display_name, person_id, match_count, suggestion: None })
        }
        None => Ok(MainPlayer {
            steamid: None,
            display_name: None,
            person_id: None,
            match_count: 0,
            suggestion: suggest(c)?,
        }),
    }
}

fn suggestion(c: &Connection, steamid: i64, source: &str, fallback_name: Option<String>) -> Result<MainPlayerSuggestion> {
    let (person_id, name, match_count) = describe(c, steamid)?;
    Ok(MainPlayerSuggestion {
        steamid: steamid.to_string(),
        display_name: name.or(fallback_name),
        person_id,
        source: source.to_string(),
        match_count,
    })
}

/// 推测候选人（不写入）。
pub fn suggest(c: &Connection) -> Result<Option<MainPlayerSuggestion>> {
    // 1. Valve 子账号：external_id 就是 Steam64。多个时取出场最多的。
    let mut best: Option<(i64, String, i64)> = None;
    {
        let mut st = c.prepare(
            "SELECT external_id, display_name FROM sub_accounts WHERE platform = 'valve' AND status != 'disabled'
             ORDER BY created_at",
        )?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (ext, name) = row?;
            if let Some(id) = parse_steamid(&ext) {
                let n: i64 = c.query_row(
                    "SELECT COUNT(DISTINCT match_id) FROM match_players WHERE steamid = ?",
                    params![id],
                    |r| r.get(0),
                )?;
                if best.as_ref().map_or(true, |b| n > b.2) {
                    best = Some((id, name, n));
                }
            }
        }
    }
    if let Some((id, name, _)) = best {
        return Ok(Some(suggestion(c, id, "valve", Some(name))?));
    }

    // 2. 其他平台子账号关联到的 steamid（identities.sub_account_id）
    let linked: Option<(i64, Option<String>)> = c
        .query_row(
            "SELECT i.steamid, sa.display_name FROM identities i JOIN sub_accounts sa ON sa.id = i.sub_account_id
             WHERE i.steamid IS NOT NULL
             ORDER BY (SELECT COUNT(*) FROM match_players mp WHERE mp.steamid = i.steamid) DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((id, name)) = linked {
        return Ok(Some(suggestion(c, id, "account", name)?));
    }

    // 3. 「我关注的人」里出场最多的 steam 身份
    let followed: Option<i64> = c
        .query_row(
            "SELECT i.steamid FROM persons p JOIN identities i ON i.person_id = p.id
             WHERE p.followed = 1 AND p.merged_into IS NULL AND i.platform = 'steam' AND i.steamid IS NOT NULL
             ORDER BY i.match_count DESC, p.id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = followed {
        return Ok(Some(suggestion(c, id, "followed", None)?));
    }

    // 4. 出场最多的人：至少 2 场，且比第二名多（只导入别人的 demo 时不要乱猜）
    let top: Vec<(i64, i64)> = {
        let mut st = c.prepare(
            "SELECT steamid, COUNT(DISTINCT match_id) AS n FROM match_players WHERE steamid > 0
             GROUP BY steamid ORDER BY n DESC LIMIT 2",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    if let Some(&(id, n)) = top.first() {
        let second = top.get(1).map_or(0, |t| t.1);
        if n >= 2 && n > second {
            return Ok(Some(suggestion(c, id, "frequent", None)?));
        }
    }
    Ok(None)
}

/// 已解析对局里的选手，按出场次数排序，供「我是谁」选择。`query` 按名字 / steamid 过滤。
pub fn list_candidates(c: &Connection, query: Option<&str>, limit: i64) -> Result<Vec<PlayerCandidate>> {
    let q = query.map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{}%", s.to_lowercase()));
    let mut st = c.prepare(
        "WITH agg AS (
           SELECT mp.steamid, COUNT(DISTINCT mp.match_id) AS n, MAX(COALESCE(m.played_at, m.discovered_at)) AS last_seen
           FROM match_players mp JOIN matches m ON m.id = mp.match_id
           WHERE mp.steamid > 0
           GROUP BY mp.steamid
         )
         SELECT a.steamid, a.n, a.last_seen,
           (SELECT mp2.name FROM match_players mp2 JOIN matches m2 ON m2.id = mp2.match_id
             WHERE mp2.steamid = a.steamid ORDER BY COALESCE(m2.played_at, m2.discovered_at) DESC LIMIT 1) AS name,
           p.id, p.display_name, COALESCE(p.followed, 0)
         FROM agg a
         LEFT JOIN identities i ON i.platform = 'steam' AND i.steamid = a.steamid
         LEFT JOIN persons p ON p.id = i.person_id
         WHERE ?1 IS NULL
            OR CAST(a.steamid AS TEXT) LIKE ?1
            OR LOWER(COALESCE(p.display_name, '')) LIKE ?1
            OR EXISTS (SELECT 1 FROM match_players x WHERE x.steamid = a.steamid AND LOWER(x.name) LIKE ?1)
         ORDER BY COALESCE(p.followed, 0) DESC, a.n DESC, a.last_seen DESC
         LIMIT ?2",
    )?;
    let rows = st.query_map(params![q, limit], |r| {
        let steamid: i64 = r.get(0)?;
        let custom: Option<String> = r.get(5)?;
        let name: Option<String> = r.get(3)?;
        Ok(PlayerCandidate {
            steamid: steamid.to_string(),
            name: custom.filter(|s| !s.trim().is_empty()).or(name).unwrap_or_else(|| steamid.to_string()),
            person_id: r.get(4)?,
            match_count: r.get(1)?,
            last_seen_at: r.get(2)?,
            followed: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
