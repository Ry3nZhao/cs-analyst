//! 手动操作：合并、撤销合并、拆分、备注 / 标签 / 显示名 / 关注。
//!
//! 对位数据挂在 steamid 上，这里只移动 identity 的归属，所以所有操作都不会丢数据。

use super::link::create_person;
use crate::database::{DbError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonSnapshot {
    pub display_name: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub followed: bool,
    pub user_id: Option<String>,
}

fn err(msg: impl Into<String>) -> DbError {
    DbError::Message(msg.into())
}

pub fn parse_tags(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

pub fn snapshot(c: &Connection, id: i64) -> Result<PersonSnapshot> {
    c.query_row(
        "SELECT display_name, note, tags, followed, user_id FROM persons WHERE id = ?",
        params![id],
        |r| {
            Ok(PersonSnapshot {
                display_name: r.get(0)?,
                note: r.get(1)?,
                tags: parse_tags(&r.get::<_, String>(2)?),
                followed: r.get(3)?,
                user_id: r.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| err(format!("人员 {id} 不存在")))
}

fn write_snapshot(c: &Connection, id: i64, s: &PersonSnapshot) -> Result<()> {
    c.execute(
        "UPDATE persons SET display_name = ?2, note = ?3, tags = ?4, followed = ?5, user_id = ?6,
           updated_at = datetime('now') WHERE id = ?1",
        params![id, s.display_name, s.note, serde_json::to_string(&s.tags).unwrap(), s.followed, s.user_id],
    )?;
    Ok(())
}

fn merged_into(c: &Connection, id: i64) -> Result<Option<i64>> {
    c.query_row("SELECT merged_into FROM persons WHERE id = ?", params![id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| err(format!("人员 {id} 不存在")))
}

/// 把 source 并进 target，返回 person_merges.id。调用方负责事务。
pub fn merge_inner(c: &Connection, target: i64, source: i64, auto: bool) -> Result<i64> {
    if target == source {
        return Err(err("不能和自己合并"));
    }
    if merged_into(c, target)?.is_some() || merged_into(c, source)?.is_some() {
        return Err(err("该人员已被合并，请使用合并后的人员"));
    }
    let t = snapshot(c, target)?;
    let s = snapshot(c, source)?;
    if t.user_id.is_some() && s.user_id.is_some() && t.user_id != s.user_id {
        return Err(err("两个人都绑定了不同的用户，不能合并"));
    }
    let ids: Vec<i64> = {
        let mut st = c.prepare("SELECT id FROM identities WHERE person_id = ? ORDER BY id")?;
        let rows = st.query_map(params![source], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut tags = t.tags.clone();
    for tag in &s.tags {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    let merged = PersonSnapshot {
        display_name: t.display_name.clone().or_else(|| s.display_name.clone()),
        note: match (&t.note, &s.note) {
            (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() && a != b => Some(format!("{a}\n{b}")),
            (Some(a), _) if !a.is_empty() => Some(a.clone()),
            _ => s.note.clone(),
        },
        tags,
        followed: t.followed || s.followed,
        user_id: t.user_id.clone().or_else(|| s.user_id.clone()),
    };
    c.execute(
        "INSERT INTO person_merges (target_id, source_id, identity_ids, target_snapshot, source_snapshot, auto)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            target,
            source,
            serde_json::to_string(&ids).unwrap(),
            serde_json::to_string(&t).unwrap(),
            serde_json::to_string(&s).unwrap(),
            auto
        ],
    )?;
    let merge_id = c.last_insert_rowid();
    c.execute("UPDATE identities SET person_id = ?1 WHERE person_id = ?2", params![target, source])?;
    write_snapshot(c, target, &merged)?;
    // 被合并方让出 user_id，避免两个 person 同时绑定一个用户
    c.execute(
        "UPDATE persons SET merged_into = ?1, user_id = NULL, updated_at = datetime('now') WHERE id = ?2",
        params![target, source],
    )?;
    Ok(merge_id)
}

pub fn merge_persons(c: &Connection, target: i64, source: i64) -> Result<i64> {
    let tx = c.unchecked_transaction()?;
    let id = merge_inner(&tx, target, source, false)?;
    tx.commit()?;
    Ok(id)
}

/// 撤销一次合并，返回恢复出来的 source person id。被撤回的身份标记为手动，自动关联不再把它并回去。
pub fn undo_merge(c: &Connection, merge_id: i64) -> Result<i64> {
    let tx = c.unchecked_transaction()?;
    let row: Option<(i64, i64, String, String, String, Option<String>)> = tx
        .query_row(
            "SELECT target_id, source_id, identity_ids, target_snapshot, source_snapshot, undone_at
             FROM person_merges WHERE id = ?",
            params![merge_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;
    let Some((target, source, ids, t_snap, s_snap, undone)) = row else {
        return Err(err("合并记录不存在"));
    };
    if undone.is_some() {
        return Err(err("这次合并已经撤销过了"));
    }
    if merged_into(&tx, target)?.is_some() {
        return Err(err("合并后的人员又被合并了，请先撤销后面那次合并"));
    }
    if merged_into(&tx, source)? != Some(target) {
        return Err(err("合并记录和当前状态不一致"));
    }
    let ids: Vec<i64> = serde_json::from_str(&ids).map_err(|e| err(e.to_string()))?;
    let t: PersonSnapshot = serde_json::from_str(&t_snap).map_err(|e| err(e.to_string()))?;
    let s: PersonSnapshot = serde_json::from_str(&s_snap).map_err(|e| err(e.to_string()))?;
    {
        let mut st = tx.prepare("UPDATE identities SET person_id = ?1, manual = 1 WHERE id = ?2 AND person_id = ?3")?;
        for id in ids {
            st.execute(params![source, id, target])?;
        }
    }
    tx.execute("UPDATE persons SET merged_into = NULL WHERE id = ?", params![source])?;
    write_snapshot(&tx, target, &t)?;
    write_snapshot(&tx, source, &s)?;
    tx.execute("UPDATE person_merges SET undone_at = datetime('now') WHERE id = ?", params![merge_id])?;
    tx.commit()?;
    Ok(source)
}

/// 把一个身份从当前的人里拆出来：挂到 `to_person`，或者新建一个人。返回身份现在所属的 person id。
pub fn split_identity(c: &Connection, identity: i64, to_person: Option<i64>) -> Result<i64> {
    let tx = c.unchecked_transaction()?;
    let cur: i64 = tx
        .query_row("SELECT person_id FROM identities WHERE id = ?", params![identity], |r| r.get(0))
        .optional()?
        .ok_or_else(|| err("身份不存在"))?;
    let dest = match to_person {
        Some(p) => {
            if merged_into(&tx, p)?.is_some() {
                return Err(err("目标人员已被合并"));
            }
            p
        }
        None => {
            let others: i64 =
                tx.query_row("SELECT COUNT(*) FROM identities WHERE person_id = ? AND id <> ?", params![cur, identity], |r| {
                    r.get(0)
                })?;
            if others == 0 {
                return Err(err("这个人只有这一个身份，无需拆分"));
            }
            create_person(&tx)?
        }
    };
    tx.execute("UPDATE identities SET person_id = ?1, manual = 1 WHERE id = ?2", params![dest, identity])?;
    tx.execute("UPDATE persons SET updated_at = datetime('now') WHERE id IN (?1, ?2)", params![cur, dest])?;
    tx.commit()?;
    Ok(dest)
}

/// 前端传来的修改。字段缺省 = 不改；`displayName` / `note` 传空字符串 = 清空。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonPatch {
    pub display_name: Option<String>,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
    pub followed: Option<bool>,
}

pub fn update_person(c: &Connection, id: i64, patch: &PersonPatch) -> Result<()> {
    let mut s = snapshot(c, id)?;
    let clean = |v: &str| {
        let v = v.trim();
        (!v.is_empty()).then(|| v.to_string())
    };
    if let Some(v) = &patch.display_name {
        s.display_name = clean(v);
    }
    if let Some(v) = &patch.note {
        s.note = clean(v);
    }
    if let Some(tags) = &patch.tags {
        let mut out: Vec<String> = vec![];
        for t in tags.iter().filter_map(|t| clean(t)) {
            if !out.contains(&t) {
                out.push(t);
            }
        }
        s.tags = out;
    }
    if let Some(f) = patch.followed {
        s.followed = f;
    }
    write_snapshot(c, id, &s)
}
