use super::main_player::{self, parse_steam_input};
use super::{notify, settings};
use crate::database::Database;
use rusqlite::params;
use std::path::PathBuf;

const ME: i64 = 76561198000000042;
const MATE: i64 = 76561198000000001;
const OTHER: i64 = 76561198000000002;

fn temp_db() -> (Database, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cs-analyst-onboarding-test-{}-{}", std::process::id(), uuid::Uuid::new_v4()));
    let db = Database::open(dir.clone()).expect("open db");
    (db, dir)
}

fn insert_match(db: &Database, id: &str, map: &str, players: &[(i64, &str)]) {
    db.conn
        .execute(
            "INSERT INTO matches (id, platform, match_id, status, file_path, map_name, played_at, analysis_status)
             VALUES (?1, 'imported', ?1, 'completed', 'demos/' || ?1 || '.dem', ?2, datetime('now'), 'done')",
            params![id, map],
        )
        .unwrap();
    for (slot, (steamid, name)) in players.iter().enumerate() {
        db.conn
            .execute(
                "INSERT INTO match_players (match_id, steamid, slot, name, team_start, team_end) VALUES (?, ?, ?, ?, 2, 2)",
                params![id, steamid, slot as i64, name],
            )
            .unwrap();
    }
    crate::identity::update_for_match(&db.conn, id).unwrap();
}

#[test]
fn parses_steam_inputs() {
    assert_eq!(parse_steam_input("76561198000000042"), Some(ME));
    assert_eq!(parse_steam_input(" https://steamcommunity.com/profiles/76561198000000042/ "), Some(ME));
    assert_eq!(parse_steam_input("[U:1:39734314]"), Some(ME));
    assert_eq!(parse_steam_input("STEAM_1:0:19867157"), Some(ME));
    assert_eq!(parse_steam_input("12345"), None);
    assert_eq!(parse_steam_input(""), None);
    assert_eq!(parse_steam_input("https://steamcommunity.com/id/vanity"), None);
}

#[test]
fn migration_008_defaults() {
    let (db, _dir) = temp_db();
    let s = settings::get(&db.conn).unwrap();
    assert_eq!(s, settings::ExperienceSettings::default());
    assert!(!settings::onboarding_completed(&db.conn).unwrap());
    assert_eq!(main_player::get_steamid(&db.conn).unwrap(), None);

    let updated = settings::ExperienceSettings {
        close_to_tray: false,
        notify_new_match: false,
        watch_replays: false,
        replays_dir: Some("  ".into()),
    };
    settings::update(&db.conn, &updated).unwrap();
    let back = settings::get(&db.conn).unwrap();
    assert!(!back.close_to_tray && !back.notify_new_match && !back.watch_replays);
    assert_eq!(back.replays_dir, None, "空白目录存成 NULL");

    settings::set_onboarding_completed(&db.conn, true).unwrap();
    assert!(settings::onboarding_completed(&db.conn).unwrap());
}

#[test]
fn set_and_get_main_player() {
    let (db, _dir) = temp_db();
    insert_match(&db, "m1", "de_dust2", &[(ME, "PlayerOne"), (MATE, "mate")]);
    main_player::set_steamid(&db.conn, Some(ME)).unwrap();
    let p = main_player::get(&db.conn).unwrap();
    assert_eq!(p.steamid.as_deref(), Some("76561198000000042"));
    assert_eq!(p.display_name.as_deref(), Some("PlayerOne"));
    assert!(p.person_id.is_some());
    assert_eq!(p.match_count, 1);
    assert_eq!(p.suggestion, None, "已设置时不给建议");

    // 不在任何对局里的 steamid 也可以设置，只是没有名字
    main_player::set_steamid(&db.conn, Some(OTHER)).unwrap();
    let p = main_player::get(&db.conn).unwrap();
    assert_eq!(p.display_name, None);
    assert_eq!(p.match_count, 0);

    main_player::set_steamid(&db.conn, None).unwrap();
    assert_eq!(main_player::get(&db.conn).unwrap().steamid, None);
}

#[test]
fn suggests_from_valve_account_without_writing() {
    let (db, _dir) = temp_db();
    insert_match(&db, "m1", "de_dust2", &[(ME, "PlayerOne"), (MATE, "mate")]);
    db.conn.execute("INSERT INTO users (id, display_name) VALUES ('u1', 'me')", []).unwrap();
    db.conn
        .execute(
            "INSERT INTO sub_accounts (id, user_id, platform, external_id, display_name) VALUES ('s1', 'u1', 'valve', ?, 'me')",
            params![ME.to_string()],
        )
        .unwrap();
    let p = main_player::get(&db.conn).unwrap();
    assert_eq!(p.steamid, None);
    let s = p.suggestion.expect("有建议");
    assert_eq!(s.source, "valve");
    assert_eq!(s.steamid, ME.to_string());
    assert_eq!(s.match_count, 1);
    assert_eq!(main_player::get_steamid(&db.conn).unwrap(), None, "建议不写入");
}

#[test]
fn suggests_followed_then_frequent() {
    let (db, _dir) = temp_db();
    // 只有一场：不猜
    insert_match(&db, "m1", "de_dust2", &[(ME, "PlayerOne"), (MATE, "mate")]);
    assert_eq!(main_player::suggest(&db.conn).unwrap(), None);

    // ME 出场 2 次，MATE 1 次 → frequent
    insert_match(&db, "m2", "de_mirage", &[(ME, "PlayerOne"), (OTHER, "other")]);
    let s = main_player::suggest(&db.conn).unwrap().unwrap();
    assert_eq!((s.source.as_str(), s.steamid.as_str()), ("frequent", "76561198000000042"));

    // 关注了 MATE → followed 优先于 frequent
    db.conn
        .execute(
            "UPDATE persons SET followed = 1 WHERE id = (SELECT person_id FROM identities WHERE steamid = ?)",
            params![MATE],
        )
        .unwrap();
    let s = main_player::suggest(&db.conn).unwrap().unwrap();
    assert_eq!((s.source.as_str(), s.steamid.as_str()), ("followed", MATE.to_string().as_str()));
}

#[test]
fn candidates_sorted_and_filtered() {
    let (db, _dir) = temp_db();
    insert_match(&db, "m1", "de_dust2", &[(ME, "PlayerOne"), (MATE, "mate")]);
    insert_match(&db, "m2", "de_mirage", &[(ME, "PlayerAlt"), (OTHER, "other")]);
    let all = main_player::list_candidates(&db.conn, None, 10).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].steamid, ME.to_string());
    assert_eq!(all[0].match_count, 2);
    let hit = main_player::list_candidates(&db.conn, Some("playeralt"), 10).unwrap();
    assert_eq!(hit.len(), 1, "按历史名字也能搜到");
    let hit = main_player::list_candidates(&db.conn, Some("000000002"), 10).unwrap();
    assert_eq!(hit[0].steamid, OTHER.to_string());
}

#[test]
fn notification_once_and_summary() {
    let (db, _dir) = temp_db();
    insert_match(&db, "m1", "de_dust2", &[(ME, "PlayerOne")]);
    for (round, a, b) in [(1, 1, 0), (2, 1, 1), (3, 2, 1)] {
        for (team, score) in [("A", a), ("B", b)] {
            db.conn
                .execute(
                    "INSERT INTO team_round_stats (match_id, round, team, side, won, score, equip_value, balance, buy_type, kills, survivors)
                     VALUES ('m1', ?, ?, 2, 0, ?, 0, 0, 'full', 0, 0)",
                    params![round, team, score],
                )
                .unwrap();
        }
    }
    assert_eq!(notify::summary(&db.conn, "m1").unwrap().as_deref(), Some("de_dust2 2:1"));
    assert_eq!(notify::summary(&db.conn, "missing").unwrap(), None);

    // 主玩家视角
    db.conn
        .execute(
            "INSERT OR REPLACE INTO h2h_player_results (match_id, steamid, team, rounds_won, rounds_lost, result) VALUES ('m1', ?, 3, 9, 13, -1)",
            params![ME],
        )
        .unwrap();
    main_player::set_steamid(&db.conn, Some(ME)).unwrap();
    assert_eq!(notify::summary(&db.conn, "m1").unwrap().as_deref(), Some("de_dust2 9:13 · 负"));

    assert!(notify::mark_notified(&db.conn, "m1").unwrap());
    assert!(!notify::mark_notified(&db.conn, "m1").unwrap(), "同一场只通知一次");
}
