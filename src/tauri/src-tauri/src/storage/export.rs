//! Export matches (selected or all) to CSV (one file per table) or JSON (one file).
//! Output goes to `<data_root>/exports/<timestamp>-<format>/`.
//!
//! JSON note: integers beyond 2^53 (Steam64 ids) are written as strings so JavaScript readers
//! do not lose precision; CSV keeps them as plain digits.

use super::{log, now_stamp, unique_dir};
use crate::database::{Database, DbError, Result};
use rusqlite::types::ValueRef;
use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// (table, description). `matches` gets a fixed column list; the others are exported with all
/// columns, filtered by `match_id`. Tables that do not exist (older schema) are skipped.
pub const TABLES: &[(&str, &str)] = &[
    ("matches", "对局"),
    ("match_players", "对局玩家"),
    ("player_match_stats", "计分板 / 每人每场指标"),
    ("player_round_stats", "每人每回合"),
    ("team_round_stats", "每队每回合"),
    ("rounds", "回合"),
    ("kills", "击杀"),
    ("damages", "伤害"),
    ("shots", "开枪"),
    ("grenades", "道具"),
    ("blinds", "致盲"),
    ("bomb_events", "炸弹"),
    ("purchases", "购买"),
    ("economies", "经济"),
    ("player_match_aim", "枪法"),
];

/// Columns of `matches` worth exporting (no raw platform payloads or download URLs).
const MATCH_COLUMNS: &[&str] = &[
    "id", "platform", "match_id", "share_code", "map_name", "played_at", "duration_seconds",
    "game_mode", "server_name", "rounds_played", "file_name", "demo_storage", "favorite",
    "analysis_status", "analyzer_version", "analyzed_at",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "csv" => Some(Format::Csv),
            "json" => Some(Format::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFile {
    pub table: String,
    pub path: String,
    pub rows: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub dir: String,
    pub format: String,
    pub matches: usize,
    pub files: Vec<ExportedFile>,
}

fn columns(db: &Database, table: &str) -> Result<Vec<String>> {
    let mut stmt = db.conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols = stmt.query_map([], |r| r.get::<_, String>(1))?;
    Ok(cols.filter_map(|c| c.ok()).collect())
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Run `f` for every exported row of `table`. Ids are bound through a temp table so any number of
/// matches works.
fn for_each_row(
    db: &Database,
    table: &str,
    cols: &[String],
    mut f: impl FnMut(&rusqlite::Row) -> std::io::Result<()>,
) -> Result<u64> {
    let list = cols.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ");
    let key = if table == "matches" { "id" } else { "match_id" };
    let order = if table == "matches" { "ORDER BY played_at" } else { "" };
    let sql = format!(
        "SELECT {list} FROM {t} WHERE {key} IN (SELECT id FROM temp.storage_export_ids) {order}",
        t = quote_ident(table)
    );
    let mut stmt = db.conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut n = 0;
    while let Some(r) = rows.next()? {
        f(r)?;
        n += 1;
    }
    Ok(n)
}

fn csv_field(v: ValueRef) -> String {
    let s = match v {
        ValueRef::Null => return String::new(),
        ValueRef::Integer(i) => return i.to_string(),
        ValueRef::Real(f) => return f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(b) => b.iter().map(|x| format!("{x:02x}")).collect(),
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn json_value(v: ValueRef) -> serde_json::Value {
    use serde_json::Value;
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) if i.unsigned_abs() > (1u64 << 53) => Value::String(i.to_string()),
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::String(b.iter().map(|x| format!("{x:02x}")).collect()),
    }
}

/// Export `ids` (or every match when `None`) into a new directory under `out_root`.
pub fn export(db: &Database, ids: Option<&[String]>, format: Format, out_root: &Path) -> Result<ExportResult> {
    db.conn.execute_batch(
        "DROP TABLE IF EXISTS temp.storage_export_ids; CREATE TEMP TABLE storage_export_ids (id TEXT PRIMARY KEY);",
    )?;
    let result = (|| {
        match ids {
            Some(ids) => {
                let mut ins = db.conn.prepare("INSERT OR IGNORE INTO temp.storage_export_ids (id) VALUES (?)")?;
                for id in ids {
                    ins.execute([id])?;
                }
            }
            None => {
                db.conn.execute("INSERT INTO temp.storage_export_ids SELECT id FROM matches", [])?;
            }
        }
        let n_matches: usize = db.conn.query_row(
            "SELECT COUNT(*) FROM matches WHERE id IN (SELECT id FROM temp.storage_export_ids)",
            [],
            |r| r.get(0),
        )?;
        if n_matches == 0 {
            return Err(DbError::Message("没有可导出的对局".into()));
        }
        std::fs::create_dir_all(out_root)?;
        let tag = if format == Format::Csv { "csv" } else { "json" };
        let dir = unique_dir(out_root, &format!("{}-{tag}", now_stamp()));
        std::fs::create_dir_all(&dir)?;
        let files = match format {
            Format::Csv => export_csv(db, &dir)?,
            Format::Json => export_json(db, &dir)?,
        };
        log(format!("exported {n_matches} match(es) as {tag} to {}", dir.display()));
        Ok(ExportResult { dir: dir.to_string_lossy().into_owned(), format: tag.into(), matches: n_matches, files })
    })();
    let _ = db.conn.execute_batch("DROP TABLE IF EXISTS temp.storage_export_ids;");
    result
}

fn table_columns(db: &Database, table: &str) -> Result<Vec<String>> {
    let all = columns(db, table)?;
    if table == "matches" {
        return Ok(MATCH_COLUMNS.iter().filter(|c| all.iter().any(|a| a == *c)).map(|c| c.to_string()).collect());
    }
    if !all.iter().any(|c| c == "match_id") {
        return Ok(vec![]);
    }
    Ok(all)
}

fn export_csv(db: &Database, dir: &Path) -> Result<Vec<ExportedFile>> {
    let mut files = vec![];
    for (table, _) in TABLES {
        let cols = table_columns(db, table)?;
        if cols.is_empty() {
            continue;
        }
        let path: PathBuf = dir.join(format!("{table}.csv"));
        let mut w = BufWriter::new(File::create(&path)?);
        // UTF-8 BOM：Excel 直接打开时中文不乱码
        w.write_all(b"\xEF\xBB\xBF")?;
        writeln!(w, "{}", cols.join(","))?;
        let rows = for_each_row(db, table, &cols, |r| {
            let line: Vec<String> = (0..cols.len()).map(|i| csv_field(r.get_ref_unwrap(i))).collect();
            writeln!(w, "{}", line.join(","))
        })?;
        w.flush()?;
        files.push(ExportedFile { table: table.to_string(), path: path.to_string_lossy().into_owned(), rows });
    }
    Ok(files)
}

fn export_json(db: &Database, dir: &Path) -> Result<Vec<ExportedFile>> {
    let path = dir.join("cs-analyst-export.json");
    let mut w = BufWriter::new(File::create(&path)?);
    let header = serde_json::json!({
        "format": "cs-analyst-export",
        "version": 1,
        "exportedAt": chrono::Local::now().to_rfc3339(),
        "parserVersion": demo_parse::PARSER_VERSION,
    });
    let header = serde_json::to_string(&header).map_err(|e| DbError::Message(e.to_string()))?;
    // 逐行流式写出，避免把 shots 等大表整个读进内存
    write!(w, "{},\"tables\":{{", &header[..header.len() - 1])?;
    let mut files = vec![];
    let mut first_table = true;
    for (table, _) in TABLES {
        let cols = table_columns(db, table)?;
        if cols.is_empty() {
            continue;
        }
        if !first_table {
            w.write_all(b",")?;
        }
        first_table = false;
        write!(w, "\"{table}\":[")?;
        let mut first = true;
        let rows = for_each_row(db, table, &cols, |r| {
            let mut obj = serde_json::Map::with_capacity(cols.len());
            for (i, c) in cols.iter().enumerate() {
                obj.insert(c.clone(), json_value(r.get_ref_unwrap(i)));
            }
            if !first {
                w.write_all(b",")?;
            }
            first = false;
            serde_json::to_writer(&mut w, &obj).map_err(std::io::Error::other)
        })?;
        w.write_all(b"]")?;
        files.push(ExportedFile { table: table.to_string(), path: path.to_string_lossy().into_owned(), rows });
    }
    w.write_all(b"}}")?;
    w.flush()?;
    Ok(files)
}
