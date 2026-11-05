//! 自动关联：demo 里的 steamid → steam 身份 → person；sub_accounts → 「我关注的人」；
//! 平台账号通过 steamid（metadata 或 demo 推断）挂到同一个人。
//!
//! 规则：
//! - 每个 identity 都必须属于一个 person；新建 identity 时如果没有归属就新建一个 person。
//! - 自动关联只会「合并 person」（记录在 person_merges，auto = 1，可撤销），不会拆分。
//! - `identities.manual = 1` 的身份（用户拆分 / 撤销过）自动关联不再移动。

use super::manage::merge_inner;
use crate::database::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub const STEAM: &str = "steam";

/// Steam64 的合法范围（个人账号）。
pub fn parse_steamid(s: &str) -> Option<i64> {
    let v: i64 = s.trim().parse().ok()?;
    (76561197960265728..76561202255233023).contains(&v).then_some(v)
}

pub fn create_person(c: &Connection) -> Result<i64> {
    c.execute("INSERT INTO persons DEFAULT VALUES", [])?;
    Ok(c.last_insert_rowid())
}

fn find_identity(c: &Connection, platform: &str, external_id: &str) -> Result<Option<(i64, i64)>> {
    Ok(c
        .query_row(
            "SELECT id, person_id FROM identities WHERE platform = ? AND external_id = ?",
            params![platform, external_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// 找到或新建 steam 身份。返回 (identity id, person id)。
pub fn ensure_steam_identity(c: &Connection, steamid: i64) -> Result<(i64, i64)> {
    let ext = steamid.to_string();
    if let Some(found) = find_identity(c, STEAM, &ext)? {
        return Ok(found);
    }
    let person = create_person(c)?;
    c.execute(
        "INSERT INTO identities (person_id, platform, external_id, steamid) VALUES (?, ?, ?, ?)",
        params![person, STEAM, ext, steamid],
    )?;
    Ok((c.last_insert_rowid(), person))
}

/// 刷新 steam 身份的名字 / 场数 / 出现时间（只看已入库的 match_players）。
pub fn refresh_steam_identity(c: &Connection, steamid: i64) -> Result<()> {
    c.execute(
        "UPDATE identities SET
           match_count = (SELECT COUNT(*) FROM match_players WHERE steamid = ?1),
           last_name = COALESCE((
             SELECT mp.name FROM match_players mp JOIN matches m ON m.id = mp.match_id
             WHERE mp.steamid = ?1
             ORDER BY COALESCE(m.played_at, m.analyzed_at, m.discovered_at) DESC LIMIT 1), last_name),
           first_seen_at = (
             SELECT MIN(COALESCE(m.played_at, m.analyzed_at)) FROM match_players mp JOIN matches m ON m.id = mp.match_id
             WHERE mp.steamid = ?1),
           last_seen_at = (
             SELECT MAX(COALESCE(m.played_at, m.analyzed_at)) FROM match_players mp JOIN matches m ON m.id = mp.match_id
             WHERE mp.steamid = ?1)
         WHERE platform = 'steam' AND steamid = ?1",
        params![steamid],
    )?;
    Ok(())
}

/// 一场对局里出现的每个 steamid 都生成身份（没有归属的自动新建 person）。
pub fn ensure_match_identities(c: &Connection, match_id: &str) -> Result<()> {
    let steamids: Vec<i64> = {
        let mut st = c.prepare("SELECT steamid FROM match_players WHERE match_id = ? AND steamid > 0")?;
        let rows = st.query_map(params![match_id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for s in steamids {
        if parse_steamid(&s.to_string()).is_none() {
            continue; // bot / 非法 id
        }
        ensure_steam_identity(c, s)?;
        refresh_steam_identity(c, s)?;
    }
    Ok(())
}

fn person_user(c: &Connection, person: i64) -> Result<Option<String>> {
    Ok(c.query_row("SELECT user_id FROM persons WHERE id = ?", params![person], |r| r.get(0))?)
}

/// 把 `identity` 所在的人自动并进 `person`。手动身份、两个不同用户的人不动。
fn attach(c: &Connection, person: i64, identity: i64) -> Result<()> {
    let (cur, manual): (i64, bool) =
        c.query_row("SELECT person_id, manual FROM identities WHERE id = ?", params![identity], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    if cur == person || manual {
        return Ok(());
    }
    let (u_target, u_source) = (person_user(c, person)?, person_user(c, cur)?);
    if u_target.is_some() && u_source.is_some() && u_target != u_source {
        return Ok(());
    }
    // 绑定了用户的一方作为合并目标，保证「我关注的人」的 id 稳定
    let (target, source) = if u_target.is_none() && u_source.is_some() { (cur, person) } else { (person, cur) };
    merge_inner(c, target, source, true)?;
    Ok(())
}

fn metadata_steamid(metadata: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(metadata).ok()?;
    for key in ["steamid", "steam_id", "steamId", "steam64", "steamid64"] {
        match v.get(key) {
            Some(serde_json::Value::String(s)) => {
                if let Some(id) = parse_steamid(s) {
                    return Some(id);
                }
            }
            Some(serde_json::Value::Number(n)) => {
                if let Some(id) = n.as_i64().and_then(|n| parse_steamid(&n.to_string())) {
                    return Some(id);
                }
            }
            _ => {}
        }
    }
    None
}

/// 从这个子账号下载的已解析 demo 推断 steamid：所有场次都出现的 steamid 里唯一的那个；
/// 不唯一时再用子账号显示名和游戏名比对。
fn infer_steamid_from_demos(c: &Connection, sub_account_id: &str, display_name: &str) -> Result<Option<i64>> {
    let n: i64 = c.query_row(
        "SELECT COUNT(*) FROM matches WHERE sub_account_id = ? AND analysis_status = 'done'",
        params![sub_account_id],
        |r| r.get(0),
    )?;
    if n == 0 {
        return Ok(None);
    }
    let mut st = c.prepare(
        "SELECT mp.steamid, MAX(LOWER(mp.name) = LOWER(?2)) FROM match_players mp JOIN matches m ON m.id = mp.match_id
         WHERE m.sub_account_id = ?1 AND m.analysis_status = 'done'
         GROUP BY mp.steamid HAVING COUNT(DISTINCT mp.match_id) = ?3",
    )?;
    let cands: Vec<(i64, bool)> = st
        .query_map(params![sub_account_id, display_name.trim(), n], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if cands.len() == 1 {
        return Ok(Some(cands[0].0));
    }
    let named: Vec<i64> = cands.iter().filter(|c| c.1).map(|c| c.0).collect();
    Ok((named.len() == 1).then(|| named[0]))
}

fn platform_identity_name(platform: &str) -> &str {
    match platform {
        "valve" => STEAM,
        other => other,
    }
}

/// users / sub_accounts → 「我关注的人」。幂等，可在每次解析后调用。
pub fn sync_sub_accounts(c: &Connection) -> Result<()> {
    let users: Vec<(String, String)> = {
        let mut st = c.prepare("SELECT id, display_name FROM users ORDER BY created_at")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (user_id, user_name) in users {
        let person: i64 = match c
            .query_row(
                "SELECT id FROM persons WHERE user_id = ? AND merged_into IS NULL ORDER BY id LIMIT 1",
                params![user_id],
                |r| r.get(0),
            )
            .optional()?
        {
            Some(p) => p,
            None => {
                c.execute(
                    "INSERT INTO persons (display_name, followed, user_id) VALUES (?, 1, ?)",
                    params![user_name, user_id],
                )?;
                c.last_insert_rowid()
            }
        };

        let accounts: Vec<(String, String, String, String, String)> = {
            let mut st = c.prepare(
                "SELECT id, platform, external_id, display_name, metadata FROM sub_accounts WHERE user_id = ? ORDER BY created_at",
            )?;
            let rows = st.query_map(params![user_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (sa_id, platform, external_id, display_name, metadata) in accounts {
            let ident_platform = platform_identity_name(&platform);
            let mut steamid = match platform.as_str() {
                "valve" | "renown" => parse_steamid(&external_id),
                _ => None,
            }
            .or_else(|| metadata_steamid(&metadata));

            let identity = match find_identity(c, ident_platform, &external_id)? {
                Some((id, _)) => id,
                None => {
                    if ident_platform == STEAM && steamid.is_none() {
                        continue; // valve 子账号但 external_id 不是 Steam64
                    }
                    c.execute(
                        "INSERT INTO identities (person_id, platform, external_id, steamid, sub_account_id, last_name)
                         VALUES (?, ?, ?, ?, ?, ?)",
                        params![person, ident_platform, external_id, steamid, sa_id, display_name],
                    )?;
                    c.last_insert_rowid()
                }
            };
            if steamid.is_none() {
                steamid = c.query_row("SELECT steamid FROM identities WHERE id = ?", params![identity], |r| r.get(0))?;
            }
            if steamid.is_none() {
                steamid = infer_steamid_from_demos(c, &sa_id, &display_name)?;
            }
            c.execute(
                "UPDATE identities SET sub_account_id = ?2, steamid = COALESCE(steamid, ?3),
                   last_name = CASE WHEN platform = 'steam' THEN last_name ELSE ?4 END
                 WHERE id = ?1",
                params![identity, sa_id, steamid, display_name],
            )?;
            // 当前 person 可能已因合并变化
            let person = c.query_row(
                "SELECT id FROM persons WHERE user_id = ? AND merged_into IS NULL ORDER BY id LIMIT 1",
                params![user_id],
                |r| r.get::<_, i64>(0),
            )?;
            attach(c, person, identity)?;
        }
    }
    Ok(())
}

/// 平台身份（非 steam）带了 steamid 的，和对应的 steam 身份归到同一个人。
pub fn link_by_steamid(c: &Connection) -> Result<()> {
    let rows: Vec<(i64, i64)> = {
        let mut st = c.prepare(
            "SELECT id, steamid FROM identities WHERE platform <> 'steam' AND steamid IS NOT NULL AND manual = 0",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (identity, steamid) in rows {
        let (steam_identity, _) = ensure_steam_identity(c, steamid)?;
        let person: i64 = c.query_row("SELECT person_id FROM identities WHERE id = ?", params![identity], |r| r.get(0))?;
        attach(c, person, steam_identity)?;
    }
    Ok(())
}
