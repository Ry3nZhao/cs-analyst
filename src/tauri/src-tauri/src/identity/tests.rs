use super::*;
use crate::database::analysis::replace_match_analysis;
use std::path::PathBuf;
use std::sync::OnceLock;

const ME: i64 = demo_parse::sample::PLAYER as i64;

fn temp_db() -> (Database, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cs-analyst-identity-test-{}-{}", std::process::id(), uuid::Uuid::new_v4()));
    let db = Database::open(dir.clone()).expect("open db");
    (db, dir)
}

fn insert_match(db: &Database, id: &str, platform: &str, played_at: &str, sub_account: Option<&str>) {
    db.conn
        .execute(
            "INSERT INTO matches (id, platform, match_id, status, file_path, played_at, sub_account_id, analysis_status)
             VALUES (?1, ?2, ?1, 'completed', 'demos/' || ?1 || '.dem', ?3, ?4, 'done')",
            params![id, platform, played_at, sub_account],
        )
        .unwrap();
}

/// h2h_stats 和 h2h_player_results 的完整快照（排序后）
fn snapshot_h2h(c: &Connection) -> (Vec<Vec<i64>>, Vec<(String, i64, i64, i64, i64, i64)>) {
    let mut st = c.prepare("SELECT * FROM h2h_stats ORDER BY a, b").unwrap();
    let n = st.column_count();
    let stats = st
        .query_map([], |r| (0..n).map(|i| r.get::<_, i64>(i)).collect::<rusqlite::Result<Vec<_>>>())
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let mut st = c
        .prepare("SELECT match_id, steamid, team, rounds_won, rounds_lost, result FROM h2h_player_results ORDER BY match_id, steamid")
        .unwrap();
    let res = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    (stats, res)
}

/// 直接从 h2h_match GROUP BY 得到的汇总，用来核对触发器
fn aggregate_from_facts(c: &Connection) -> Vec<Vec<i64>> {
    let mut st = c
        .prepare(
            "SELECT a, b, SUM(same_team), SUM(same_team = 1 AND result = 1), SUM(same_team = 1 AND result = -1),
               SUM(1 - same_team), SUM(same_team = 0 AND result = 1), SUM(same_team = 0 AND result = -1),
               SUM(kills), SUM(deaths), SUM(damage), SUM(damage_taken), SUM(duels), SUM(duels_won), SUM(duels_lost),
               SUM(opening_wins), SUM(opening_losses)
             FROM h2h_match GROUP BY a, b ORDER BY a, b",
        )
        .unwrap();
    st.query_map([], |r| (0..17).map(|i| r.get::<_, i64>(i)).collect::<rusqlite::Result<Vec<_>>>())
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn person_of(c: &Connection, steamid: i64) -> i64 {
    c.query_row("SELECT person_id FROM identities WHERE platform = 'steam' AND steamid = ?", params![steamid], |r| r.get(0))
        .unwrap()
}

// ---------------------------------------------------------------------------------------------
// 合成数据：手工构造一场，核对口径
// ---------------------------------------------------------------------------------------------

const A1: i64 = 76561198000000001; // 开局 T
const A2: i64 = 76561198000000002; // 开局 T
const B1: i64 = 76561198000000011; // 开局 CT
const B2: i64 = 76561198000000012; // 开局 CT

fn synthetic(db: &Database, m: &str) {
    insert_match(db, m, "imported", "2026-01-01T00:00:00Z", None);
    let c = &db.conn;
    for (slot, (s, t, name)) in [(A1, 2, "a1"), (A2, 2, "a2"), (B1, 3, "b1"), (B2, 3, "b2")].iter().enumerate() {
        c.execute(
            "INSERT INTO match_players (match_id, steamid, slot, name, team_start, team_end) VALUES (?, ?, ?, ?, ?, ?)",
            params![m, s, slot as i64, name, t, 5 - t],
        )
        .unwrap();
    }
    // 3 回合：第 1、2 回合 A 在 T，第 3 回合换边 A 在 CT。胜者：T、CT、CT → A 队 1:2？
    // 第 1 回合 T(A) 赢；第 2 回合 CT(B) 赢；第 3 回合 CT(A) 赢 → A 队 2:1 胜
    for (n, w) in [(1, 2), (2, 3), (3, 3)] {
        c.execute(
            "INSERT INTO rounds (match_id, number, start_tick, end_tick, winner) VALUES (?, ?, ?, ?, ?)",
            params![m, n, n * 10000, n * 10000 + 9000, w],
        )
        .unwrap();
        for (s, start) in [(A1, 2), (A2, 2), (B1, 3), (B2, 3)] {
            let side = if n < 3 { start } else { 5 - start };
            c.execute(
                "INSERT INTO economies (match_id, round, player, team) VALUES (?, ?, ?, ?)",
                params![m, n, s, side],
            )
            .unwrap();
        }
    }
    let dmg = |round: i64, tick: i64, a: i64, v: i64, d: i64, after: i64, at: i64, vt: i64| {
        c.execute(
            "INSERT INTO damages (match_id, round, tick, attacker, victim, weapon, dmg_health, dmg_armor, health_after, armor_after, attacker_team, victim_team)
             VALUES (?, ?, ?, ?, ?, 'ak47', ?, 0, ?, 0, ?, ?)",
            params![m, round, tick, a, v, d, after, at, vt],
        )
        .unwrap();
    };
    let kill = |round: i64, tick: i64, a: Option<i64>, v: i64, at: i64, vt: i64, weapon: &str| {
        c.execute(
            "INSERT INTO kills (match_id, round, tick, attacker, victim, weapon, headshot, penetrated, noscope, thrusmoke,
               attacker_blind, attacker_in_air, assisted_flash, attacker_team, victim_team)
             VALUES (?, ?, ?, ?, ?, ?, 0, 0, 0, 0, 0, 0, 0, ?, ?)",
            params![m, round, tick, a, v, weapon, at, vt],
        )
        .unwrap();
    };
    // 回合 1：A2 自杀（不算开局），A1 打 B1 27 → B1 反打 A1 一枪 400 (实际 100) 击杀 —— 对枪 B1 赢，开局 B1
    kill(1, 10100, Some(A2), A2, 2, 2, "world");
    dmg(1, 10200, A1, B1, 27, 73, 2, 3);
    dmg(1, 10250, B1, A1, 400, 0, 3, 2);
    kill(1, 10250, Some(B1), A1, 3, 2, "awp");
    // 回合 1：队友伤害 B2 → B1，不计
    dmg(1, 10300, B2, B1, 50, 23, 3, 3);
    // 回合 2：A1 打 B2 30，间隔 > 3 秒后 B2 击杀 A1 —— 第一次交火平，击杀单独算；开局 B2
    dmg(2, 20000, A1, B2, 30, 70, 2, 3);
    dmg(2, 20500, B2, A1, 100, 0, 3, 2);
    kill(2, 20500, Some(B2), A1, 3, 2, "ak47");
    // 回合 3（换边，A 在 CT）：A1 击杀 B1 —— A1 对枪赢、开局
    dmg(3, 30000, A1, B1, 120, 0, 3, 2);
    kill(3, 30000, Some(A1), B1, 3, 2, "ak47");
}

#[test]
fn synthetic_match_rules() {
    let (db, dir) = temp_db();
    synthetic(&db, "s1");
    update_for_match(&db.conn, "s1").unwrap();
    let f = h2h::compute_match(&db.conn, "s1").unwrap();

    // 换边后比分：A 队 2:1
    assert_eq!(f.players[&A1], h2h::PlayerResult { team: 2, rounds_won: 2, rounds_lost: 1, result: 1 });
    assert_eq!(f.players[&B1].result, -1);

    let a1b1 = &f.pairs[&(A1, B1)];
    assert!(!a1b1.same_team);
    assert_eq!((a1b1.kills, a1b1.deaths), (1, 1));
    assert_eq!(a1b1.damage, 27 + 100, "实际扣血：回合 3 B1 只剩 100");
    assert_eq!(a1b1.damage_taken, 100, "400 伤害按剩余 100 血计");
    assert_eq!((a1b1.duels, a1b1.duels_won, a1b1.duels_lost), (2, 1, 1));
    assert_eq!((a1b1.opening_wins, a1b1.opening_losses), (1, 1), "回合 1 自杀不算开局");

    let a1b2 = &f.pairs[&(A1, B2)];
    assert_eq!((a1b2.duels, a1b2.duels_won, a1b2.duels_lost), (1, 0, 0), "间隔超过 3 秒：第一次交火算平");
    assert_eq!((a1b2.kills, a1b2.deaths, a1b2.opening_losses), (0, 1, 1));

    // 自杀、队友伤害都不计
    let a2 = &f.pairs[&(A2, B1)];
    assert_eq!((a2.kills, a2.deaths, a2.damage), (0, 0, 0));
    let b2b1 = &f.pairs[&(B2, B1)];
    assert!(b2b1.same_team);
    assert_eq!(b2b1.damage, 0);

    // 命令层：同队、对手
    let (pa1, pa2, pb1) = (person_of(&db.conn, A1), person_of(&db.conn, A2), person_of(&db.conn, B1));
    let h = query::get_head_to_head(&db.conn, pa1, pb1).unwrap();
    assert_eq!((h.against.matches, h.against.wins, h.together.matches), (1, 1, 0));
    assert_eq!(h.duels, query::DuelDto { total: 2, won: 1, lost: 1, win_rate: Some(0.5) });
    let t = query::list_teammates(&db.conn, pa1, 1, 10).unwrap();
    assert_eq!(t.teammates.len(), 1);
    assert_eq!(t.teammates[0].person.id, pa2);
    assert_eq!(t.teammates[0].record.wins, 1);
    assert_eq!(t.overall.matches, 1);

    // 删除对局 → 触发器把 h2h_stats 清干净
    db.conn.execute("DELETE FROM matches WHERE id = 's1'", []).unwrap();
    let n: i64 = db.conn.query_row("SELECT COUNT(*) FROM h2h_stats", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sub_accounts_become_followed_person() {
    let (db, dir) = temp_db();
    synthetic(&db, "s1");
    update_for_match(&db.conn, "s1").unwrap();
    let demo_person = person_of(&db.conn, A1);
    let c = &db.conn;
    c.execute("INSERT INTO users (id, display_name) VALUES ('u1', '我')", []).unwrap();
    c.execute(
        "INSERT INTO sub_accounts (id, user_id, platform, external_id, display_name, metadata)
         VALUES ('sa1', 'u1', 'valve', ?, 'a1', '{}'),
                ('sa2', 'u1', 'renown', ?, 'a1_renown', '{}'),
                ('sa3', 'u1', '5eplay', 'uuid-1', 'a1_5e', ?)",
        params![A1.to_string(), A1.to_string(), format!("{{\"steamid\":\"{A1}\"}}")],
    )
    .unwrap();
    backfill(c).unwrap();

    let p = person_of(c, A1);
    let d = query::get_person(c, p).unwrap();
    assert!(d.person.followed);
    assert_eq!(d.person.user_id.as_deref(), Some("u1"));
    assert_eq!(d.person.display_name, "我");
    let mut plats: Vec<&str> = d.identities.iter().map(|i| i.platform.as_str()).collect();
    plats.sort();
    assert_eq!(plats, ["5eplay", "renown", "steam"]);
    assert_eq!(d.record.matches, 1);
    // 原来由 demo 自动生成的人被自动合并（可撤销）
    assert_eq!(query::resolve(c, demo_person).unwrap(), p);
    assert!(d.merges.iter().all(|m| m.auto));

    // 幂等
    backfill(c).unwrap();
    assert_eq!(query::get_person(c, p).unwrap(), d);

    // 撤销自动合并后，自动关联不会再把它并回去
    let mid = d.merges.iter().find(|m| m.source_id == demo_person).unwrap().id;
    manage::undo_merge(c, mid).unwrap();
    update_for_match(c, "s1").unwrap();
    assert_eq!(person_of(c, A1), demo_person);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------------------------
// 样例 demo（目录不存在则跳过）
// ---------------------------------------------------------------------------------------------

fn sample_root() -> PathBuf {
    PathBuf::from(std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_default())
}

/// (官匹 demo, 平台 demo)，都包含 ME。只解析一次。
fn samples() -> Option<&'static (demo_parse::ParsedDemo, demo_parse::ParsedDemo)> {
    static CACHE: OnceLock<Option<(demo_parse::ParsedDemo, demo_parse::ParsedDemo)>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let root = sample_root();
            let platform = root.join("Platforms/g161-n-20260709134236148390830.dem");
            if !platform.exists() {
                eprintln!("skip: {} not found", platform.display());
                return None;
            }
            let platform = demo_parse::parse_demo_file(&platform).unwrap();
            let mut official: Vec<PathBuf> = std::fs::read_dir(root.join("Official"))
                .ok()?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "dem"))
                .collect();
            official.sort_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX));
            for p in official {
                let d = demo_parse::parse_demo_file(&p).unwrap();
                if d.players.iter().any(|x| x.steamid as i64 == ME) {
                    eprintln!("official sample: {}", p.display());
                    return Some((d, platform));
                }
            }
            eprintln!("skip: no official demo contains {ME}");
            None
        })
        .as_ref()
}

/// 主玩家在某个 demo 里的名字（不在仓库里写死）。
fn name_in(d: &demo_parse::ParsedDemo) -> &str {
    &d.players.iter().find(|p| p.steamid as i64 == ME).expect("main player in sample").name
}

fn ingest_samples(db: &Database, with_hook: bool) {
    let (official, platform) = samples().unwrap();
    db.conn.execute("INSERT INTO users (id, display_name) VALUES ('u1', 'me')", []).unwrap();
    // 5E 子账号只有平台内 id 和名字；名字取自平台 demo，靠场次交集 + 名字推断 steamid
    db.conn
        .execute(
            "INSERT INTO sub_accounts (id, user_id, platform, external_id, display_name) VALUES ('sa5e', 'u1', '5eplay', 'uuid-me', ?)",
            params![name_in(platform)],
        )
        .unwrap();
    insert_match(db, "off", "valve", "2026-07-01T00:00:00Z", None);
    insert_match(db, "plat", "5eplay", "2026-07-09T13:42:36Z", Some("sa5e"));
    replace_match_analysis(db, "off", official, None, 1).unwrap();
    if with_hook {
        on_match_analyzed(db, "off");
    }
    replace_match_analysis(db, "plat", platform, None, 1).unwrap();
    if with_hook {
        on_match_analyzed(db, "plat");
    }
}

#[test]
fn sample_same_person_across_platforms() {
    if samples().is_none() {
        return;
    }
    let (db, dir) = temp_db();
    ingest_samples(&db, true);
    let c = &db.conn;

    let p = person_of(c, ME);
    let d = query::get_person(c, p).unwrap();
    let names: Vec<&str> = d.aliases.iter().map(|a| a.name.as_str()).collect();
    let (official, platform) = samples().unwrap();
    let (official_name, platform_name) = (name_in(official), name_in(platform));
    assert_ne!(official_name, platform_name, "samples should use two different names");
    assert!(names.contains(&official_name), "{names:?}");
    assert!(names.contains(&platform_name), "{names:?}");
    assert_eq!(d.person.steamids, vec![ME.to_string()]);
    assert_eq!(d.person.match_count, 2);
    assert_eq!(d.record.matches, 2);
    assert!(d.person.followed);
    // 5E 子账号没有 steamid，靠 demo（场次交集 + 名字）推断出来并挂到同一个人
    let five = d.identities.iter().find(|i| i.platform == "5eplay").expect("5eplay identity linked");
    assert_eq!(five.steamid.as_deref(), Some(ME.to_string().as_str()));

    // 搜索：历史名字和 steamid 都能找到
    let partial = official_name.chars().take(6).collect::<String>().to_lowercase();
    for q in [partial.as_str(), platform_name, &ME.to_string()] {
        let r = query::search_persons(c, Some(q), false, 10).unwrap();
        assert!(r.iter().any(|x| x.id == p), "search {q}");
    }

    // 平台那场：对手方向的击杀合计 = 15（与 database::analysis 测试一致：排除自杀）
    let k: i64 = c
        .query_row("SELECT SUM(kills) FROM h2h_match WHERE match_id = 'plat' AND a = ?", params![ME], |r| r.get(0))
        .unwrap();
    assert_eq!(k, 15);
    // 每场两队胜负相反，比分合计等于有胜者的回合数
    for m in ["off", "plat"] {
        let (wins, losses, n): (i64, i64, i64) = c
            .query_row(
                "SELECT SUM(result = 1), SUM(result = -1), COUNT(*) FROM h2h_player_results WHERE match_id = ?",
                params![m],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(n, 10, "{m}");
        assert!(wins == 5 && losses == 5 || wins == 0 && losses == 0, "{m}: {wins}/{losses}");
        let (won, lost): (i64, i64) = c
            .query_row(
                "SELECT rounds_won, rounds_lost FROM h2h_player_results WHERE match_id = ? AND steamid = ?",
                params![m, ME],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let rounds: i64 = c
            .query_row("SELECT COUNT(*) FROM rounds WHERE match_id = ? AND winner IN (2, 3)", params![m], |r| r.get(0))
            .unwrap();
        eprintln!("{m}: ME {won}:{lost} of {rounds} rounds");
        assert_eq!(won + lost, rounds, "{m}");
    }
    // 对枪：赢 + 输 <= 总数，开局对枪合计 = 有效首杀的回合数
    let (duels, won, lost, ow, ol): (i64, i64, i64, i64, i64) = c
        .query_row(
            "SELECT SUM(duels), SUM(duels_won), SUM(duels_lost), SUM(opening_wins), SUM(opening_losses) FROM h2h_match WHERE match_id = 'plat'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert!(won + lost <= duels && won == lost && ow == ol && ow > 0, "{duels} {won} {lost} {ow} {ol}");

    // ME 在两场里的关键数字（固定下来做回归）
    let mine = |m: &str| -> [i64; 9] {
        c.query_row(
            "SELECT SUM(kills), SUM(deaths), SUM(damage), SUM(damage_taken), SUM(duels), SUM(duels_won), SUM(duels_lost),
               SUM(opening_wins), SUM(opening_losses) FROM h2h_match WHERE match_id = ? AND a = ?",
            params![m, ME],
            |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?]),
        )
        .unwrap()
    };
    // K, D, 伤害, 承伤, 对枪, 赢, 输, 开局赢, 开局输
    assert_eq!(mine("off"), [10, 13, 1665, 1373, 30, 8, 12, 0, 2]);
    assert_eq!(mine("plat"), [15, 6, 1318, 713, 25, 14, 6, 1, 0]);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sample_incremental_equals_rebuild() {
    if samples().is_none() {
        return;
    }
    let (db, dir) = temp_db();
    ingest_samples(&db, true);
    let c = &db.conn;
    let incremental = snapshot_h2h(c);
    assert!(!incremental.0.is_empty());
    let as_stats: Vec<Vec<i64>> = incremental.0.clone();
    assert_eq!(as_stats, aggregate_from_facts(c), "触发器维护的 h2h_stats = h2h_match 直接汇总");

    // 同一场重复增量更新（重新解析）不会重复累计
    on_match_analyzed(&db, "plat");
    on_match_analyzed(&db, "off");
    assert_eq!(snapshot_h2h(c), incremental);

    assert_eq!(h2h::rebuild_all(c).unwrap(), 2);
    assert_eq!(snapshot_h2h(c), incremental, "全量重建 = 增量更新");

    // 不走 hook、只做存量回填，结果也一样（身份、对位都一致）
    let (db2, dir2) = temp_db();
    ingest_samples(&db2, false);
    assert_eq!(backfill(&db2.conn).unwrap(), 2);
    assert_eq!(snapshot_h2h(&db2.conn), incremental);
    let p1 = query::get_person(c, person_of(c, ME)).unwrap();
    let p2 = query::get_person(&db2.conn, person_of(&db2.conn, ME)).unwrap();
    assert_eq!(p1.aliases, p2.aliases);
    assert_eq!(p1.record, p2.record);
    drop(db);
    drop(db2);
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir2);
}

#[test]
fn sample_merge_split_round_trip() {
    if samples().is_none() {
        return;
    }
    let (db, dir) = temp_db();
    ingest_samples(&db, true);
    let c = &db.conn;
    let me = person_of(c, ME);

    // 平台那场里的一个对手和一个队友
    let (opp, mate): (i64, i64) = {
        let team = |s: i64| -> i64 {
            c.query_row("SELECT team FROM h2h_player_results WHERE match_id='plat' AND steamid=?", params![s], |r| r.get(0))
                .unwrap()
        };
        let mine = team(ME);
        let mut st = c.prepare("SELECT steamid FROM h2h_player_results WHERE match_id='plat' AND steamid <> ? ORDER BY steamid").unwrap();
        let others: Vec<i64> = st.query_map(params![ME], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        let opp = *others.iter().find(|s| team(**s) != mine).unwrap();
        let mate = *others.iter().find(|s| team(**s) == mine).unwrap();
        (person_of(c, opp), person_of(c, mate))
    };
    let h_before = query::get_head_to_head(c, me, opp).unwrap();
    assert_eq!(h_before.against.matches, 1);
    let mate_before = query::get_person(c, mate).unwrap();
    let me_before = query::get_person(c, me).unwrap();
    let strip = |d: &query::PersonDetail| {
        let mut d = d.clone();
        for i in &mut d.identities {
            i.manual = false;
        }
        d
    };

    // 合并：队友并进对手 → 对手名下两个 steamid，交手数据变成两人之和
    manage::update_person(c, mate, &manage::PersonPatch { note: Some("队友".into()), tags: Some(vec!["组排".into()]), ..Default::default() })
        .unwrap();
    let mate_noted = query::get_person(c, mate).unwrap();
    let mid = manage::merge_persons(c, opp, mate).unwrap();
    let merged = query::get_person(c, opp).unwrap();
    assert_eq!(merged.person.steamids.len(), 2);
    assert_eq!(merged.person.tags, vec!["组排".to_string()]);
    assert_eq!(merged.person.note.as_deref(), Some("队友"));
    assert_eq!(query::resolve(c, mate).unwrap(), opp);
    let h_merged = query::get_head_to_head(c, me, opp).unwrap();
    assert_eq!((h_merged.against.matches, h_merged.together.matches), (1, 1));
    assert!(manage::merge_persons(c, me, mate).is_err(), "已被合并的人不能再合并");

    // 撤销 → 两边都恢复
    manage::undo_merge(c, mid).unwrap();
    assert!(manage::undo_merge(c, mid).is_err());
    assert_eq!(strip(&query::get_person(c, mate).unwrap()), strip(&mate_noted));
    assert_eq!(query::get_head_to_head(c, me, opp).unwrap(), h_before);
    manage::update_person(c, mate, &manage::PersonPatch { note: Some("".into()), tags: Some(vec![]), ..Default::default() }).unwrap();
    assert_eq!(strip(&query::get_person(c, mate).unwrap()), strip(&mate_before));

    // 拆分我的 steam 身份 → 新的人拿走全部对局数据；再合并回来 → 和原来一样
    let steam_identity = me_before.identities.iter().find(|i| i.platform == "steam").unwrap().id;
    let split = manage::split_identity(c, steam_identity, None).unwrap();
    assert_ne!(split, me);
    let h_split = query::get_head_to_head(c, split, opp).unwrap();
    assert_eq!(h_split.against, h_before.against);
    assert_eq!(h_split.duels, h_before.duels);
    assert_eq!(query::get_head_to_head(c, me, opp).unwrap().against.matches, 0);
    // 拆出来的身份是手动的，自动关联不会再把它并回去
    on_match_analyzed(&db, "plat");
    assert_eq!(person_of(c, ME), split);
    assert!(manage::split_identity(c, steam_identity, None).is_err(), "只剩一个身份不能再拆");

    manage::merge_persons(c, me, split).unwrap();
    let back = query::get_person(c, me).unwrap();
    assert_eq!(back.person.steamids, me_before.person.steamids);
    assert_eq!(back.aliases, me_before.aliases);
    assert_eq!(back.record, me_before.record);
    let h_back = query::get_head_to_head(c, me, opp).unwrap();
    assert_eq!((h_back.against, h_back.duels, h_back.kills, h_back.damage), (h_before.against, h_before.duels, h_before.kills, h_before.damage));
    // 组排队友
    let t = query::list_teammates(c, me, 1, 20).unwrap();
    assert!(t.teammates.iter().any(|x| x.person.id == mate));
    assert_eq!(t.overall.matches, 2);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

