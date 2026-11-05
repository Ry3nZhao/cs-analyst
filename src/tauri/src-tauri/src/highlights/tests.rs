//! 集锦的样例回归测试和播放相关的单元测试。
//!
//! 样例 demo 在 `<sample-dir>`（`CS_ANALYST_SAMPLE_DEMOS` 可覆盖），缺文件时跳过。
//! 测试只用复制到临时目录里的副本，不碰原文件。

use super::*;
use crate::database::analysis::replace_match_analysis;
use rusqlite::params;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ME: u64 = 76561198000000042;

const OFFICIAL: &str = "Official/match730_003831848039841005782_0410013497_154_competitive.dem";
const PLATFORM: &str = "Platforms/g161-n-20260709134236148390830.dem";
const PRO: &str = "Profs/2026major/stage1/nrg-vs-big-m1-dust2-p1.dem";

fn sample(rel: &str) -> Option<PathBuf> {
    let root = std::env::var("CS_ANALYST_SAMPLE_DEMOS").unwrap_or_else(|_| "<sample-dir>".into());
    let p = PathBuf::from(root).join(rel);
    if p.exists() {
        Some(p)
    } else {
        eprintln!("skip: {} not found", p.display());
        None
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cs-analyst-hl-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 把样例复制到临时目录，完整走一遍入库（事件 → stats → aim → 集锦）。
fn ingest(db: &Arc<Mutex<Database>>, dir: &Path, id: &str, rel: &str) -> bool {
    let Some(src) = sample(rel) else { return false };
    let copy = dir.join(format!("{id}.dem"));
    std::fs::copy(&src, &copy).unwrap();
    {
        let g = db.lock();
        g.conn
            .execute(
                "INSERT INTO matches (id, platform, match_id, status, file_path, analysis_status, played_at, map_name)
                 VALUES (?1, 'imported', ?1, 'completed', ?2, 'pending', datetime('now', '-1 day'), NULL)",
                params![id, copy.to_string_lossy()],
            )
            .unwrap();
    }
    let demo = demo_parse::parse_demo_file(&copy).unwrap();
    {
        let g = db.lock();
        replace_match_analysis(&g, id, &demo, None, 0).unwrap();
        crate::stats::on_match_ingested(&g, id, &demo);
    }
    crate::aim::analyze_match(db, id, &copy, Some(&demo));
    on_match_analyzed(db, id);
    true
}

fn counts(db: &Database, m: &str) -> BTreeMap<String, i64> {
    let mut st = db.conn.prepare("SELECT kind, COUNT(*) FROM highlights WHERE match_id = ? GROUP BY kind").unwrap();
    st.query_map(params![m], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
}

fn mine(db: &Database, m: &str) -> Vec<(String, i64, i64, f64, String)> {
    let mut st = db
        .conn
        .prepare("SELECT kind, round, start_tick, score, detail FROM highlights WHERE match_id = ? AND steamid = ? ORDER BY round, start_tick, kind")
        .unwrap();
    st.query_map(params![m, ME as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn count_map(pairs: &[(&str, i64)]) -> BTreeMap<String, i64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn filters(steamid: Option<u64>) -> repo::HighlightFilters {
    repo::HighlightFilters { steamid: steamid.map(|s| s.to_string()), ..Default::default() }
}

/// 3 场样例：固定每条规则的命中数、主玩家的典型事件，并检查查询、设置和重算登记。
#[test]
fn sample_demos_regression() {
    let dir = temp_dir("samples");
    let db = Arc::new(Mutex::new(Database::open(dir.join("data")).unwrap()));
    for (id, rel) in [("official", OFFICIAL), ("platform", PLATFORM), ("pro", PRO)] {
        if !ingest(&db, &dir, id, rel) {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
    }
    let g = db.lock();
    for id in ["official", "platform", "pro"] {
        eprintln!("== {id}: {:?}", counts(&g, id));
        for row in mine(&g, id) {
            eprintln!("   me {row:?}");
        }
    }

    // ── 每条规则的命中数 ──
    assert_eq!(
        counts(&g, "official"),
        count_map(&[
            ("clutch_win", 1),
            ("flick_headshot", 3),
            ("lost_hp_duel", 3),
            ("multi_kill", 10),
            ("quick_kills", 1),
            ("smoke_kill", 3),
            ("swing_kill", 16),
            ("swing_loss", 16),
            ("team_damage", 2),
            ("team_flash", 7),
            ("untraded_opening_death", 14),
            ("unused_utility_death", 18),
            ("wallbang", 2),
        ])
    );
    assert_eq!(
        counts(&g, "platform"),
        count_map(&[
            ("blind_kill", 2),
            ("clutch_win", 1),
            ("flick_headshot", 1),
            ("lost_hp_duel", 4),
            ("multi_kill", 8),
            ("quick_kills", 3),
            ("smoke_kill", 1),
            ("swing_kill", 6),
            ("swing_loss", 6),
            ("team_damage", 2),
            ("team_flash", 11),
            ("untraded_opening_death", 13),
            ("unused_utility_death", 14),
            ("whiffed_spray", 5),
        ])
    );
    // ESL 职业 demo：没有致盲事件、可见性不可靠 → 没有闪到队友和甩枪
    assert_eq!(
        counts(&g, "pro"),
        count_map(&[
            ("blind_kill", 4),
            ("lost_hp_duel", 3),
            ("multi_kill", 7),
            ("quick_kills", 2),
            ("smoke_kill", 10),
            ("swing_kill", 6),
            ("swing_loss", 6),
            ("team_damage", 1),
            ("untraded_opening_death", 11),
            ("unused_utility_death", 4),
            ("wallbang", 3),
        ])
    );
    let available: String =
        g.conn.query_row("SELECT available FROM match_highlights WHERE match_id = 'pro'", [], |r| r.get(0)).unwrap();
    assert!(!available.contains("team_flash") && !available.contains("flick_headshot"), "{available}");

    // ── 和 stats 口径一致：多杀条数 = 3K+4K+5K 回合数，残局获胜条数 = 残局胜场 ──
    for id in ["official", "platform", "pro"] {
        let (mk, clutch): (i64, i64) = g
            .conn
            .query_row(
                "SELECT SUM(mk3 + mk4 + mk5),
                        SUM(clutch_1v1_wins + clutch_1v2_wins + clutch_1v3_wins + clutch_1v4_wins + clutch_1v5_wins)
                 FROM player_match_stats WHERE match_id = ?",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let c = counts(&g, id);
        assert_eq!(c.get("multi_kill").copied().unwrap_or(0), mk, "{id} multi_kill");
        assert_eq!(c.get("clutch_win").copied().unwrap_or(0), clutch, "{id} clutch_win");
    }

    // ── 主玩家的典型事件 ──
    let me_official = mine(&g, "official");
    assert_eq!(
        me_official.iter().map(|r| (r.0.as_str(), r.1)).collect::<Vec<_>>(),
        vec![
            ("unused_utility_death", 4),
            ("unused_utility_death", 9),
            ("untraded_opening_death", 12),
            ("swing_loss", 14),
            ("unused_utility_death", 14),
        ]
    );
    let opening = me_official.iter().find(|r| r.0 == "untraded_opening_death").unwrap();
    assert_eq!((opening.2, opening.3), (64245, 55.0));
    let d: serde_json::Value = serde_json::from_str(&opening.4).unwrap();
    assert_eq!((d["weapon"].as_str(), d["roundLost"].as_bool()), (Some("awp"), Some(true)));
    let swing = me_official.iter().find(|r| r.0 == "swing_loss").unwrap();
    let d: serde_json::Value = serde_json::from_str(&swing.4).unwrap();
    assert_eq!(d["deltaPp"].as_f64(), Some(-28.9));

    let me_platform = mine(&g, "platform");
    assert_eq!(
        me_platform.iter().map(|r| (r.0.as_str(), r.1, r.2)).collect::<Vec<_>>(),
        vec![
            ("multi_kill", 6, 31547),
            ("smoke_kill", 6, 33024),
            ("unused_utility_death", 9, 44318),
            ("unused_utility_death", 10, 48159),
            ("unused_utility_death", 11, 51825),
        ]
    );
    let mk: serde_json::Value = serde_json::from_str(&me_platform[0].4).unwrap();
    assert_eq!(mk["count"], 3);
    assert!(mine(&g, "pro").is_empty(), "主玩家不在职业比赛里");

    // ── 查询：主玩家跨场汇总 ──
    let all = repo::list_highlights(&g, &filters(Some(ME))).unwrap();
    assert_eq!((all.total, all.matches, all.pending), (10, 2, 0));
    assert!(all.items.windows(2).all(|w| w[0].weighted_score >= w[1].weighted_score), "按分数排序");
    assert!(all.names.contains_key(&ME.to_string()));
    assert!(all.names.contains_key("76561198377969427"), "细节里的凶手也有名字");
    let mistakes =
        repo::list_highlights(&g, &repo::HighlightFilters { category: Some("mistake".into()), ..filters(Some(ME)) }).unwrap();
    assert_eq!(mistakes.total, 8);
    assert!(mistakes.items.iter().all(|h| h.category == "mistake"));
    // 类型计数不受高光 / 失误筛选影响
    assert_eq!(mistakes.counts.iter().map(|c| c.count).sum::<i64>(), 10);
    let one = repo::list_highlights(
        &g,
        &repo::HighlightFilters {
            kinds: Some(vec!["multi_kill".into()]),
            match_id: Some("platform".into()),
            ..filters(Some(ME))
        },
    )
    .unwrap();
    assert_eq!((one.total, one.items[0].round, one.items[0].start_tick), (1, 6, 31547));

    // 时间筛选：把官匹挪到 40 天前，最近 30 天只剩平台那场
    g.conn.execute("UPDATE matches SET played_at = datetime('now', '-40 days') WHERE id = 'official'", []).unwrap();
    let recent = repo::list_highlights(&g, &repo::HighlightFilters { days: Some(30), ..filters(Some(ME)) }).unwrap();
    assert_eq!((recent.total, recent.matches), (5, 1));
    // 所有选手
    let everyone = repo::list_highlights(&g, &repo::HighlightFilters { limit: Some(5), ..filters(None) }).unwrap();
    assert_eq!((everyone.total, everyone.items.len()), (96 + 77 + 57, 5));

    // ── 设置：关掉规则、调权重 ──
    let mut s = repo::get_settings(&g).unwrap();
    assert_eq!(s.rules.len(), highlights::Kind::ALL.len());
    assert!(!s.netcon_enabled, "netconport 默认关闭");
    s.rules.iter_mut().find(|r| r.kind == "unused_utility_death").unwrap().enabled = false;
    s.rules.iter_mut().find(|r| r.kind == "multi_kill").unwrap().weight = 2.0;
    repo::update_settings(&g, &s).unwrap();
    let after = repo::list_highlights(&g, &filters(Some(ME))).unwrap();
    assert_eq!(after.total, 4);
    assert_eq!((after.items[0].kind.as_str(), after.items[0].weighted_score), ("multi_kill", 100.0));
    let incl = repo::list_highlights(&g, &repo::HighlightFilters { include_disabled: true, ..filters(Some(ME)) }).unwrap();
    assert_eq!(incl.total, 10);
    // 恢复默认后不留行
    for r in &mut s.rules {
        r.enabled = true;
        r.weight = r.default_weight;
    }
    repo::update_settings(&g, &s).unwrap();
    let n: i64 = g.conn.query_row("SELECT COUNT(*) FROM highlight_rules", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);

    // ── 重算登记：都是最新的；规则版本变了 / stats 重算过就重新排队 ──
    assert!(repo::list_matches_needing(&g).unwrap().is_empty());
    g.conn.execute("UPDATE match_highlights SET rules_version = 0 WHERE match_id = 'pro'", []).unwrap();
    g.conn.execute("UPDATE match_highlights SET stats_version = -1 WHERE match_id = 'official'", []).unwrap();
    let mut need = repo::list_matches_needing(&g).unwrap();
    need.sort();
    assert_eq!(need, vec!["official".to_string(), "pro".to_string()]);
    assert_eq!(recompute(&g, "pro").unwrap(), Some(57));
    assert_eq!(counts(&g, "pro").values().sum::<i64>(), 57, "重算是整场替换，不会重复");

    // 删除对局时级联删除
    g.conn.execute("DELETE FROM matches WHERE id = 'pro'", []).unwrap();
    let left: i64 = g.conn.query_row("SELECT COUNT(*) FROM highlights WHERE match_id = 'pro'", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);

    drop(g);
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------------------------
// 播放
// ---------------------------------------------------------------------------------------------

fn row(storage: &str, original: Option<i64>) -> repo::PlaybackRow {
    repo::PlaybackRow {
        match_id: "m/1".into(),
        start_tick: 10_000,
        round_start_tick: Some(9_000),
        file_path: None,
        demo_storage: storage.into(),
        demo_original_size: original,
    }
}

#[test]
fn goto_tick_and_commands() {
    assert_eq!(play::goto_tick(10_000, Some(5_000), 5), 10_000 - 320);
    assert_eq!(play::goto_tick(10_000, Some(9_900), 5), 9_900, "不早于回合开始");
    assert_eq!(play::goto_tick(100, None, 5), 0);

    let dir = temp_dir("play");
    let demo = dir.join("a b.dem");
    std::fs::write(&demo, b"HL2DEMO").unwrap();
    let info = play::prepare_with(7, &row("raw", None), Some(&demo), 5, &dir.join("tmp")).unwrap();
    assert_eq!(info.status, "ready");
    assert!(!info.decompressed);
    let p = play::cs2_path(&demo);
    assert!(!p.contains('\\'));
    assert_eq!(info.playdemo_command.as_deref(), Some(format!("playdemo \"{p}\"").as_str()));
    assert_eq!(info.goto_command, "demo_gototick 9680");
    assert_eq!(info.combined_command.as_deref(), Some(format!("playdemo \"{p}\"; demo_gototick 9680").as_str()));

    // 已删除 / 找不到
    let del = play::prepare_with(7, &row("deleted", None), Some(&demo), 5, &dir).unwrap();
    assert_eq!((del.status.as_str(), del.playdemo_command.is_none()), ("deleted", true));
    let missing = play::prepare_with(7, &row("raw", None), Some(&dir.join("nope.dem")), 5, &dir).unwrap();
    assert_eq!(missing.status, "missing");

    // 启动参数
    assert_eq!(
        play::launch_args("C:/x/a.dem", Some(2121), " -tools  -insecure "),
        vec!["-netconport", "2121", "-tools", "-insecure", "+playdemo", "C:/x/a.dem"]
    );
    assert_eq!(play::launch_args("C:/x/a.dem", None, ""), vec!["+playdemo", "C:/x/a.dem"]);
    assert_eq!(
        play::steam_url(&play::launch_args("C:/my demos/a.dem", None, "")),
        "steam://rungameid/730//+playdemo%20%22C:/my%20demos/a.dem%22"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn compressed_demo_is_decompressed_for_playback() {
    let dir = temp_dir("zst");
    let raw: Vec<u8> = (0..200_000u32).flat_map(|i| (i % 251).to_le_bytes()).collect();
    let zst = dir.join("m.dem.zst");
    std::fs::write(&zst, zstd::encode_all(&raw[..], 3).unwrap()).unwrap();
    let tmp = dir.join("playback");

    let info = play::prepare_with(1, &row("compressed", Some(raw.len() as i64)), Some(&zst), 5, &tmp).unwrap();
    assert_eq!(info.status, "ready");
    assert!(info.decompressed);
    let out = tmp.join("m_1.dem");
    assert_eq!(std::fs::read(&out).unwrap(), raw);
    assert_eq!(info.demo_path.as_deref(), Some(play::cs2_path(&out).as_str()));
    // 第二次复用
    play::prepare_with(1, &row("compressed", Some(raw.len() as i64)), Some(&zst), 5, &tmp).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), raw);
    // 大小对不上就报错，不留半成品
    let bad = repo::PlaybackRow { match_id: "bad".into(), ..row("compressed", Some(1)) };
    assert!(play::prepare_with(1, &bad, Some(&zst), 5, &tmp).is_err());
    assert!(!tmp.join("bad.dem").exists() && !tmp.join("bad.dem.part").exists());
    // 临时目录只保留最近 2 个
    for id in ["a", "b", "c"] {
        std::thread::sleep(std::time::Duration::from_millis(30));
        let r = repo::PlaybackRow { match_id: id.into(), ..row("compressed", None) };
        play::prepare_with(1, &r, Some(&zst), 5, &tmp).unwrap();
    }
    let mut left: Vec<String> =
        std::fs::read_dir(&tmp).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    left.sort();
    assert_eq!(left, vec!["b.dem".to_string(), "c.dem".to_string()]);
    let _ = std::fs::remove_dir_all(dir);
}
