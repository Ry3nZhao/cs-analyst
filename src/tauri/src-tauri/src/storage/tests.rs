//! Storage tests. Everything runs in fresh temp directories; sample demos are only ever *copied*
//! (the originals in the sample folder are opened read-only).

use super::backup::{create_backup, list_backups, restore_backup, validate_backup};
use super::cleanup::{apply_plan, preview_cleanup, preview_policy, CleanupRules};
use super::demo::{compress_match, delete_match_demo};
use super::export::{export, Format};
use super::*;
use crate::database::Database;
use rusqlite::params;
use std::path::PathBuf;

fn temp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("cs-analyst-storage-{tag}-{}", uuid::Uuid::new_v4()))
}

fn sample(rel: &str) -> Option<PathBuf> {
    let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_else(|_| "<sample-dir>".into());
    let p = PathBuf::from(root).join(rel);
    p.exists().then_some(p)
}

fn insert_match(db: &Database, id: &str, file_rel: Option<&str>, played_at: &str) {
    db.conn
        .execute(
            "INSERT INTO matches (id, platform, match_id, status, file_path, played_at)
             VALUES (?1, 'imported', ?1, 'completed', ?2, ?3)",
            params![id, file_rel, played_at],
        )
        .unwrap();
}

/// Insert one row with type-appropriate placeholder values for every NOT NULL column.
fn fake_row(db: &Database, table: &str, match_id: &str, extra: &[(&str, i64)]) {
    let mut stmt = db.conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
    let cols: Vec<(String, String, bool, bool)> = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)? != 0, r.get::<_, Option<String>>(4)?.is_some()))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let mut names = vec![];
    let mut values = vec![];
    for (name, ty, notnull, has_default) in cols {
        if name == "match_id" {
            names.push(name);
            values.push(format!("'{match_id}'"));
        } else if let Some((_, v)) = extra.iter().find(|(n, _)| *n == name) {
            names.push(name);
            values.push(v.to_string());
        } else if notnull && !has_default {
            names.push(name);
            values.push(if ty.contains("TEXT") { "'A'".into() } else { "0".into() });
        }
    }
    db.conn
        .execute(&format!("INSERT INTO {table} ({}) VALUES ({})", names.join(","), values.join(",")), [])
        .unwrap();
}

/// Make a match look fully analysed (analysis, stats and aim rows present, current versions).
fn mark_fully_analysed(db: &Database, id: &str) {
    db.conn
        .execute(
            "UPDATE matches SET analysis_status = 'done', analyzer_version = ?2 WHERE id = ?1",
            params![id, demo_parse::PARSER_VERSION],
        )
        .unwrap();
    fake_row(db, "rounds", id, &[("number", 1)]);
    fake_row(db, "match_players", id, &[]);
    fake_row(db, "player_match_stats", id, &[]);
    db.conn
        .execute(
            "INSERT INTO match_aim (match_id, aim_version, status) VALUES (?1, ?2, 'done')",
            params![id, ::aim::AIM_VERSION],
        )
        .unwrap();
}

fn write_demo(root: &std::path::Path, name: &str, bytes: usize) -> String {
    let rel = format!("demos/imported/{name}");
    let p = root.join(&rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, vec![7u8; bytes]).unwrap();
    rel
}

fn storage_of(db: &Database, id: &str) -> (String, Option<String>) {
    db.conn
        .query_row("SELECT demo_storage, file_path FROM matches WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
}

/// Compress a real demo, re-parse it from `.dem.zst`: the result must equal the original parse.
/// Then the delete path: refused until every safety condition holds.
#[test]
fn compressed_demo_reparses_identically() {
    let Some(src) = sample("Profs/2026major/stage1/nrg-vs-big-m1-dust2-p2.dem") else {
        eprintln!("skip: sample demo not found");
        return;
    };
    let root = temp_root("compress");
    let db = Database::open(root.clone()).unwrap();
    let rel = "demos/imported/nrg-vs-big.dem";
    std::fs::create_dir_all(root.join("demos/imported")).unwrap();
    std::fs::copy(&src, root.join(rel)).unwrap();
    let copy = root.join(rel);
    insert_match(&db, "m1", Some(rel), "2026-06-01 12:00:00");

    // Not analysed yet → refused.
    let db = Arc::new(Mutex::new(db));
    assert!(compress_match(&db, "m1").unwrap_err().contains("分析尚未完成"));

    let original = demo_parse::parse_demo_file(&copy).unwrap();
    {
        let g = db.lock();
        crate::database::analysis::replace_match_analysis(&g, "m1", &original, None, 1).unwrap();
        crate::stats::on_match_ingested(&g, "m1", &original);
    }
    let count = |t: &str| -> i64 {
        db.lock().conn.query_row(&format!("SELECT COUNT(*) FROM {t} WHERE match_id='m1'"), [], |r| r.get(0)).unwrap()
    };
    let before: Vec<i64> = ["rounds", "kills", "damages", "shots", "player_match_stats"].iter().map(|t| count(t)).collect();
    let raw_size = std::fs::metadata(&copy).unwrap().len();

    let out = compress_match(&db, "m1").unwrap();
    assert_eq!(out.bytes_before, raw_size);
    assert!(out.bytes_after < raw_size, "compressed {} vs raw {}", out.bytes_after, raw_size);
    assert!(!copy.exists(), "original .dem removed after verified compression");
    let (storage, path) = storage_of(&db.lock(), "m1");
    assert_eq!(storage, "compressed");
    let path = path.unwrap();
    assert!(path.ends_with(".dem.zst"));
    let zst = db.lock().resolve_path(&path);
    assert!(zst.exists());
    let original_size: i64 = db
        .lock()
        .conn
        .query_row("SELECT demo_original_size FROM matches WHERE id='m1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(original_size as u64, raw_size);

    // Transparent re-parse through the normal entry point (what the analysis worker calls).
    let reparsed = demo_parse::parse_demo_file(&zst).unwrap();
    assert_eq!(serde_json::to_vec(&original).unwrap(), serde_json::to_vec(&reparsed).unwrap());
    let explicit = demo_parse::parse_demo_zst(&zst).unwrap();
    assert_eq!(explicit.kills.len(), original.kills.len());
    let eng_raw = demo_parse::parse_engagements_file(&zst, Default::default()).unwrap();
    assert!(!eng_raw.windows.is_empty() || original.kills.is_empty());

    // Re-ingest from the compressed file → same rows.
    {
        let g = db.lock();
        crate::database::analysis::replace_match_analysis(&g, "m1", &reparsed, None, 1).unwrap();
        crate::stats::on_match_ingested(&g, "m1", &reparsed);
    }
    let after: Vec<i64> = ["rounds", "kills", "damages", "shots", "player_match_stats"].iter().map(|t| count(t)).collect();
    assert_eq!(before, after);
    assert!(compress_match(&db, "m1").unwrap_err().contains("已经压缩"));

    // Delete: aim data missing → refused; favorite → refused; otherwise ok.
    assert!(delete_match_demo(&db, "m1").unwrap_err().contains("枪法"));
    db.lock()
        .conn
        .execute("INSERT INTO match_aim (match_id, aim_version, status) VALUES ('m1', ?1, 'done')", [::aim::AIM_VERSION])
        .unwrap();
    set_favorite(&db.lock(), &["m1".into()], true).unwrap();
    assert!(delete_match_demo(&db, "m1").unwrap_err().contains("收藏"));
    set_favorite(&db.lock(), &["m1".into()], false).unwrap();
    // Re-analysis in progress → refused.
    db.lock().conn.execute("UPDATE matches SET analysis_status='pending' WHERE id='m1'", []).unwrap();
    assert!(delete_match_demo(&db, "m1").is_err());
    db.lock().conn.execute("UPDATE matches SET analysis_status='done' WHERE id='m1'", []).unwrap();

    delete_match_demo(&db, "m1").unwrap();
    assert!(!zst.exists());
    let (storage, path) = storage_of(&db.lock(), "m1");
    assert_eq!((storage.as_str(), path), ("deleted", None));
    assert_eq!(count("kills"), before[1], "analysed data kept");
    assert!(crate::database::analysis::list_matches_needing_analysis(&db.lock()).unwrap().is_empty());

    drop(db);
    let _ = std::fs::remove_dir_all(root);
}

/// Files outside `<data_root>/demos` are never compressed or deleted.
#[test]
fn never_touches_files_outside_data_root() {
    let root = temp_root("outside");
    let outside = temp_root("user-folder");
    std::fs::create_dir_all(&outside).unwrap();
    let user_file = outside.join("mine.dem");
    std::fs::write(&user_file, b"demo").unwrap();
    let db = Database::open(root.clone()).unwrap();
    insert_match(&db, "u1", Some(&user_file.to_string_lossy()), "2020-01-01 00:00:00");
    mark_fully_analysed(&db, "u1");
    assert!(check_demo_safe(&db, "u1", DemoAction::Delete).unwrap_err().contains("demos/"));
    let plan = preview_cleanup(&db, &CleanupRules { max_age_days: Some(1), max_total_gb: None }).unwrap();
    assert_eq!(plan.eligible_count, 0);
    let db = Arc::new(Mutex::new(db));
    apply_plan(&db, &plan.token, &|_, _| {}).unwrap();
    assert!(user_file.exists());
    drop(db);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}

/// Cleanup by age and by total size; favourites are never touched; tokens are single-use.
#[test]
fn cleanup_spares_favorites() {
    let root = temp_root("cleanup");
    let db = Database::open(root.clone()).unwrap();
    // 4 demos of 1 MB: old (favorite), old, recent, recent
    let specs = [("old-fav", "2020-01-01 00:00:00"), ("old", "2020-02-01 00:00:00"), ("new1", "2099-01-01 00:00:00"), ("new2", "2099-02-01 00:00:00")];
    for (id, at) in specs {
        let rel = write_demo(&root, &format!("{id}.dem"), 1 << 20);
        insert_match(&db, id, Some(&rel), at);
        mark_fully_analysed(&db, id);
    }
    // One not yet analysed: never selected.
    let rel = write_demo(&root, "pending.dem", 1 << 20);
    insert_match(&db, "pending", Some(&rel), "2019-01-01 00:00:00");
    set_favorite(&db, &["old-fav".into()], true).unwrap();

    let plan = preview_cleanup(&db, &CleanupRules { max_age_days: Some(30), max_total_gb: None }).unwrap();
    let eligible: Vec<&str> = plan.items.iter().filter(|i| i.eligible).map(|i| i.id.as_str()).collect();
    assert_eq!(eligible, vec!["old"]);
    assert_eq!(plan.skipped_favorites, 1);
    assert!(plan.items.iter().any(|i| i.id == "pending" && !i.eligible));
    assert_eq!(plan.total_demo_bytes, 5 << 20);

    let db = Arc::new(Mutex::new(db));
    let token = plan.token.clone();
    let r = apply_plan(&db, &token, &|_, _| {}).unwrap();
    assert_eq!((r.deleted, r.bytes_freed), (1, 1 << 20));
    assert!(apply_plan(&db, &token, &|_, _| {}).is_err(), "token is single-use");
    assert!(root.join("demos/imported/old-fav.dem").exists());
    assert!(!root.join("demos/imported/old.dem").exists());
    assert_eq!(storage_of(&db.lock(), "old").0, "deleted");

    // Size cap 3.5 MB with 4 MB left (fav, new1, new2, pending): deletes oldest non-favourite
    // eligible demos until under the cap → new1 only (favourite and pending are skipped).
    let plan = preview_cleanup(&db.lock(), &CleanupRules { max_age_days: None, max_total_gb: Some(3.5 / 1024.0) }).unwrap();
    let eligible: Vec<&str> = plan.items.iter().filter(|i| i.eligible).map(|i| i.id.as_str()).collect();
    assert_eq!(eligible, vec!["new1"]);
    assert!(!plan.items.iter().any(|i| i.id == "old-fav"));
    apply_plan(&db, &plan.token, &|_, _| {}).unwrap();
    assert!(root.join("demos/imported/old-fav.dem").exists());
    assert!(root.join("demos/imported/new2.dem").exists());
    assert!(root.join("demos/imported/pending.dem").exists());

    // A stale token (a newer preview replaced it) does nothing.
    let stale = preview_cleanup(&db.lock(), &CleanupRules { max_age_days: Some(1), max_total_gb: None }).unwrap().token;
    let _newer = preview_policy(&db.lock(), DemoAction::Compress, None).unwrap();
    assert!(apply_plan(&db, &stale, &|_, _| {}).is_err());
    assert!(root.join("demos/imported/new2.dem").exists());

    let settings = get_settings(&db.lock()).unwrap();
    assert!(settings.last_cleanup_summary.unwrap().contains("删除 1 个"));
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}

/// Backup → change data → restore gives back exactly the backed-up data; the pre-restore backup
/// holds the changed state; no `-wal` / `-shm` files are ever produced in backups.
#[test]
fn backup_and_restore_round_trip() {
    let root = temp_root("backup");
    let mut db = Database::open(root.clone()).unwrap();
    for (i, id) in ["a", "b", "c"].iter().enumerate() {
        insert_match(&db, id, None, &format!("2026-0{}-01 00:00:00", i + 1));
        mark_fully_analysed(&db, id);
    }
    set_favorite(&db, &["b".into()], true).unwrap();
    std::fs::write(root.join("analysis/a.replay.zst"), b"replay-a-v1").unwrap();
    let snapshot = |db: &Database| -> Vec<(String, i64, i64)> {
        let mut s = db
            .conn
            .prepare("SELECT m.id, m.favorite, (SELECT COUNT(*) FROM rounds r WHERE r.match_id = m.id) FROM matches m ORDER BY m.id")
            .unwrap();
        let rows = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        rows.map(|r| r.unwrap()).collect()
    };
    let original = snapshot(&db);

    let info = create_backup(&root, true, "manual").unwrap();
    let dir = PathBuf::from(&info.path);
    assert!(dir.join("analyst.db").is_file());
    assert!(!dir.join("analyst.db-wal").exists() && !dir.join("analyst.db-shm").exists());
    assert_eq!(std::fs::read(dir.join("analysis/a.replay.zst")).unwrap(), b"replay-a-v1");
    let m = info.manifest.unwrap();
    assert_eq!((m.matches, m.replay_files, m.includes_replays), (3, 1, true));

    // Change everything after the backup.
    db.conn.execute("DELETE FROM matches WHERE id = 'a'", []).unwrap();
    insert_match(&db, "d", None, "2026-05-01 00:00:00");
    set_favorite(&db, &["b".into()], false).unwrap();
    std::fs::write(root.join("analysis/a.replay.zst"), b"replay-a-v2").unwrap();
    let changed = snapshot(&db);
    assert_ne!(original, changed);

    // A raw copy with a -wal next to it is rejected.
    let bad = temp_root("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::copy(dir.join("analyst.db"), bad.join("analyst.db")).unwrap();
    std::fs::write(bad.join("analyst.db-wal"), b"x").unwrap();
    assert!(validate_backup(&db, &bad).unwrap_err().to_string().contains("-wal"));

    let r = restore_backup(&mut db, &dir).unwrap();
    assert_eq!(snapshot(&db), original);
    assert_eq!(r.matches, 3);
    assert_eq!(r.replay_files_restored, 1);
    assert_eq!(std::fs::read(root.join("analysis/a.replay.zst")).unwrap(), b"replay-a-v1");
    let mode: String = db.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode, "wal", "live database stays in WAL mode");
    let fk: i64 = db.conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 1);
    // Cascades still work on the restored database.
    db.conn.execute("DELETE FROM matches WHERE id = 'c'", []).unwrap();
    let left: i64 = db.conn.query_row("SELECT COUNT(*) FROM rounds WHERE match_id = 'c'", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);

    // The automatic pre-restore backup holds the changed state.
    let pre = PathBuf::from(&r.pre_restore_backup.path);
    assert!(pre.file_name().unwrap().to_string_lossy().ends_with("pre-restore"));
    let pre_db = rusqlite::Connection::open_with_flags(pre.join("analyst.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let ids: Vec<String> = pre_db
        .prepare("SELECT id FROM matches ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(ids, vec!["b", "c", "d"]);
    assert_eq!(std::fs::read(pre.join("analysis/a.replay.zst")).unwrap(), b"replay-a-v2");
    drop(pre_db);

    let all = list_backups(&db).unwrap();
    assert_eq!(all.len(), 2);
    for b in &all {
        let p = PathBuf::from(&b.path);
        assert!(!p.join("analyst.db-wal").exists() && !p.join("analyst.db-shm").exists());
    }
    drop(db);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(bad);
}

/// CSV and JSON export of selected matches; Steam64 ids survive as exact strings in JSON.
#[test]
fn export_csv_and_json() {
    let root = temp_root("export");
    let db = Database::open(root.clone()).unwrap();
    insert_match(&db, "a", None, "2026-01-01 00:00:00");
    insert_match(&db, "b", None, "2026-01-02 00:00:00");
    fake_row(&db, "match_players", "a", &[("steamid", 76561198000000042)]);
    fake_row(&db, "match_players", "b", &[("steamid", 76561198000000001)]);
    db.conn.execute("UPDATE match_players SET name = 'Player, \"quoted\"' WHERE match_id = 'a'", []).unwrap();
    let out = root.join("exports");

    let r = export(&db, Some(&["a".into()]), Format::Csv, &out).unwrap();
    assert_eq!(r.matches, 1);
    let players = r.files.iter().find(|f| f.table == "match_players").unwrap();
    assert_eq!(players.rows, 1);
    let text = std::fs::read_to_string(&players.path).unwrap();
    assert!(text.contains("76561198000000042"));
    assert!(text.contains("\"Player, \"\"quoted\"\"\""));
    assert!(!text.contains("76561198000000001"), "unselected match not exported");

    let r = export(&db, None, Format::Json, &out).unwrap();
    assert_eq!(r.matches, 2);
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&r.files[0].path).unwrap()).unwrap();
    assert_eq!(v["format"], "cs-analyst-export");
    let players = v["tables"]["match_players"].as_array().unwrap();
    assert_eq!(players.len(), 2);
    assert!(players.iter().any(|p| p["steamid"] == "76561198000000042"));
    assert_eq!(v["tables"]["matches"].as_array().unwrap().len(), 2);
    assert!(v["tables"]["matches"][0].get("raw_payload").is_none());

    assert!(export(&db, Some(&["nope".into()]), Format::Csv, &out).is_err());
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn usage_and_settings() {
    let root = temp_root("usage");
    let db = Database::open(root.clone()).unwrap();
    let rel = write_demo(&root, "x.dem", 1000);
    insert_match(&db, "x", Some(&rel), "2026-01-01 00:00:00");
    insert_match(&db, "y", None, "2026-01-01 00:00:00");
    std::fs::write(root.join("analysis/x.replay.zst"), vec![0u8; 300]).unwrap();
    db.conn.execute("UPDATE matches SET replay_path = 'analysis/x.replay.zst' WHERE id = 'x'", []).unwrap();
    let u = usage::compute(&db).unwrap();
    assert_eq!((u.demo_bytes, u.replay_bytes, u.raw_count), (1000, 300, 1));
    assert!(u.database_bytes > 0);
    assert_eq!(u.by_platform.len(), 1);
    assert_eq!(u.by_platform[0].matches, 2);
    assert_eq!(u.by_account[0].label, "未关联账号");

    let mut s = get_settings(&db).unwrap();
    assert_eq!(s.demo_policy, DemoPolicy::Keep);
    assert!(!s.auto_cleanup);
    s.auto_cleanup = true;
    assert!(update_settings(&db, &s).is_err(), "auto cleanup needs a rule");
    s.cleanup_max_age_days = Some(90);
    s.demo_policy = DemoPolicy::Compress;
    update_settings(&db, &s).unwrap();
    let s = get_settings(&db).unwrap();
    assert_eq!((s.demo_policy, s.auto_cleanup, s.cleanup_max_age_days), (DemoPolicy::Compress, true, Some(90)));
    drop(db);
    let _ = std::fs::remove_dir_all(root);
}
