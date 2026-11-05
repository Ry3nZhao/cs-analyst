//! 训练成绩的存取。

use crate::database::{Database, DbError, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const MODES: &[&str] = &["reaction", "flick", "tracking", "micro", "strafe", "spray"];
/// 一轮训练最多保存的尝试数（压枪 10 次连发 × 30 发 = 300，留足余量）。
pub const MAX_TRIALS: usize = 2000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialInput {
    pub value: Option<f64>,
    pub ok: Option<bool>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInput {
    pub mode: String,
    #[serde(default)]
    pub variant: Option<String>,
    pub started_at: String,
    pub duration_ms: i64,
    pub score: Option<f64>,
    #[serde(default)]
    pub metrics: Option<serde_json::Value>,
    #[serde(default)]
    pub settings: Option<serde_json::Value>,
    #[serde(default)]
    pub trials: Vec<TrialInput>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: String,
    pub mode: String,
    pub variant: String,
    pub started_at: String,
    pub duration_ms: i64,
    pub score: Option<f64>,
    pub metrics: serde_json::Value,
    pub settings: serde_json::Value,
    pub trial_count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrialRow {
    pub idx: i64,
    pub value: Option<f64>,
    pub ok: Option<bool>,
    pub data: serde_json::Value,
}

fn json_or_empty(v: &Option<serde_json::Value>) -> String {
    match v {
        Some(v) if v.is_object() => v.to_string(),
        _ => "{}".into(),
    }
}

fn parse_json(s: String) -> serde_json::Value {
    serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({}))
}

/// 保存一轮训练，返回新 id。会话和所有尝试在一个事务里写入。
pub fn save_session(db: &Database, input: &SessionInput) -> Result<String> {
    if !MODES.contains(&input.mode.as_str()) {
        return Err(DbError::Message(format!("未知的训练模式: {}", input.mode)));
    }
    if input.trials.len() > MAX_TRIALS {
        return Err(DbError::Message(format!("尝试次数过多: {}", input.trials.len())));
    }
    if chrono::DateTime::parse_from_rfc3339(&input.started_at).is_err() {
        return Err(DbError::Message(format!("开始时间格式错误: {}", input.started_at)));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let score = input.score.filter(|s| s.is_finite());
    let tx = db.conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO training_sessions (id, mode, variant, started_at, duration_ms, score, metrics, settings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            id,
            input.mode,
            input.variant.clone().unwrap_or_default(),
            input.started_at,
            input.duration_ms.max(0),
            score,
            json_or_empty(&input.metrics),
            json_or_empty(&input.settings),
        ],
    )?;
    {
        let mut stmt = tx.prepare("INSERT INTO training_results (session_id, idx, value, ok, data) VALUES (?, ?, ?, ?, ?)")?;
        for (i, t) in input.trials.iter().enumerate() {
            stmt.execute(params![id, i as i64, t.value.filter(|v| v.is_finite()), t.ok, json_or_empty(&t.data)])?;
        }
    }
    tx.commit()?;
    Ok(id)
}

/// `range`：`7d` / `30d` / `90d` / `365d` / `all`（默认 `all`）。按开始时间升序，便于直接画趋势。
pub fn list_sessions(db: &Database, mode: &str, variant: Option<&str>, range: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> Result<Vec<SessionRow>> {
    let days: Option<i64> = match range.unwrap_or("all") {
        "all" | "" => None,
        r => {
            let n = r.strip_suffix('d').and_then(|n| n.parse::<i64>().ok()).filter(|n| *n > 0);
            Some(n.ok_or_else(|| DbError::Message(format!("无效的时间范围: {r}")))?)
        }
    };
    let cutoff = days
        .map(|d| (now - chrono::Duration::days(d)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default();
    let mut stmt = db.conn.prepare(
        "SELECT s.id, s.mode, s.variant, s.started_at, s.duration_ms, s.score, s.metrics, s.settings,
                (SELECT COUNT(*) FROM training_results r WHERE r.session_id = s.id)
         FROM training_sessions s
         WHERE s.mode = ?1 AND (?2 IS NULL OR s.variant = ?2) AND s.started_at >= ?3
         ORDER BY s.started_at ASC, s.created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![mode, variant, cutoff], |r| {
            Ok(SessionRow {
                id: r.get(0)?,
                mode: r.get(1)?,
                variant: r.get(2)?,
                started_at: r.get(3)?,
                duration_ms: r.get(4)?,
                score: r.get(5)?,
                metrics: parse_json(r.get(6)?),
                settings: parse_json(r.get(7)?),
                trial_count: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_trials(db: &Database, session_id: &str) -> Result<Vec<TrialRow>> {
    let mut stmt = db.conn.prepare("SELECT idx, value, ok, data FROM training_results WHERE session_id = ? ORDER BY idx")?;
    let rows = stmt
        .query_map([session_id], |r| {
            Ok(TrialRow { idx: r.get(0)?, value: r.get(1)?, ok: r.get(2)?, data: parse_json(r.get(3)?) })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn delete_session(db: &Database, session_id: &str) -> Result<bool> {
    Ok(db.conn.execute("DELETE FROM training_sessions WHERE id = ?", [session_id])? > 0)
}

#[allow(dead_code)]
pub fn session_exists(db: &Database, session_id: &str) -> Result<bool> {
    Ok(db
        .conn
        .query_row("SELECT 1 FROM training_sessions WHERE id = ?", [session_id], |_| Ok(()))
        .optional()?
        .is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn temp_db() -> (Database, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("cs-analyst-training-test-{}", uuid::Uuid::new_v4()));
        let db = Database::open(dir.clone()).expect("open db");
        (db, dir)
    }

    fn input(mode: &str, variant: Option<&str>, started_at: &str, score: f64, trials: usize) -> SessionInput {
        SessionInput {
            mode: mode.into(),
            variant: variant.map(Into::into),
            started_at: started_at.into(),
            duration_ms: 30_000,
            score: Some(score),
            metrics: Some(serde_json::json!({ "mean": score + 10.0, "fouls": 1 })),
            settings: Some(serde_json::json!({ "sensitivity": 1.25 })),
            trials: (0..trials)
                .map(|i| TrialInput { value: Some(200.0 + i as f64), ok: Some(i % 2 == 0), data: Some(serde_json::json!({ "i": i })) })
                .collect(),
        }
    }

    #[test]
    fn migration_creates_training_tables() {
        let (db, dir) = temp_db();
        let v: i32 = db.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert!(v >= 7, "user_version = {v}");
        for t in ["training_sessions", "training_results"] {
            let n: i64 = db
                .conn
                .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?", [t], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 1, "{t}");
        }
        // 迁移可重放（开发库可能先到 7 再补 006）。
        db.conn.execute_batch(include_str!("../../../../../migrations/sqlite/007_training.sql")).unwrap();
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_list_roundtrip() {
        let (db, dir) = temp_db();
        let now = chrono::Utc.with_ymd_and_hms(2026, 11, 5, 12, 0, 0).unwrap();
        let old = save_session(&db, &input("reaction", None, "2026-09-01T10:00:00.000Z", 240.0, 5)).unwrap();
        let new = save_session(&db, &input("reaction", None, "2026-11-04T10:00:00.000Z", 221.5, 5)).unwrap();
        save_session(&db, &input("spray", Some("ak47"), "2026-11-04T11:00:00.000Z", 61.0, 30)).unwrap();
        save_session(&db, &input("spray", Some("m4a1"), "2026-11-04T12:00:00.000Z", 70.0, 20)).unwrap();

        let all = list_sessions(&db, "reaction", None, Some("all"), now).unwrap();
        assert_eq!(all.iter().map(|s| s.id.clone()).collect::<Vec<_>>(), vec![old.clone(), new.clone()]);
        assert_eq!(all[1].score, Some(221.5));
        assert_eq!(all[1].trial_count, 5);
        assert_eq!(all[1].metrics["fouls"], 1);
        assert_eq!(all[1].settings["sensitivity"], 1.25);
        assert_eq!(all[1].variant, "");

        let week = list_sessions(&db, "reaction", None, Some("7d"), now).unwrap();
        assert_eq!(week.len(), 1);
        assert_eq!(week[0].id, new);

        let ak = list_sessions(&db, "spray", Some("ak47"), None, now).unwrap();
        assert_eq!(ak.len(), 1);
        assert_eq!(ak[0].variant, "ak47");
        assert_eq!(list_sessions(&db, "spray", None, None, now).unwrap().len(), 2);

        let trials = get_trials(&db, &new).unwrap();
        assert_eq!(trials.len(), 5);
        assert_eq!(trials[2].idx, 2);
        assert_eq!(trials[2].value, Some(202.0));
        assert_eq!(trials[2].ok, Some(true));
        assert_eq!(trials[1].ok, Some(false));
        assert_eq!(trials[3].data["i"], 3);

        assert!(list_sessions(&db, "reaction", None, Some("x"), now).is_err());

        // 删除会级联删掉尝试。
        assert!(delete_session(&db, &new).unwrap());
        assert!(!session_exists(&db, &new).unwrap());
        assert!(get_trials(&db, &new).unwrap().is_empty());
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_bad_input() {
        let (db, dir) = temp_db();
        assert!(save_session(&db, &input("unknown", None, "2026-11-04T10:00:00Z", 1.0, 0)).is_err());
        assert!(save_session(&db, &input("flick", None, "yesterday", 1.0, 0)).is_err());
        // NaN 成绩存成 NULL，而不是报错。
        let mut i = input("flick", None, "2026-11-04T10:00:00Z", 1.0, 1);
        i.score = Some(f64::NAN);
        let id = save_session(&db, &i).unwrap();
        let rows = list_sessions(&db, "flick", None, None, chrono::Utc::now()).unwrap();
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].score, None);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
