-- CS Analyst migration 004: identities, persons and head-to-head (W1-C, docs/roadmap.md 2.7).
--
-- 一个 person 下挂多个 identity。identity = (platform, external_id)：
--   platform = 'steam'  : demo 里的 Steam64（external_id 就是十进制 steamid），对局数据只通过它关联
--   platform = '5eplay' / 'renown' / 'perfect' / ... : 平台账号，steamid 列记录它对应的 Steam64（可为空）
-- SteamID 一律存 INTEGER。h2h 物化在 steamid × steamid 粒度上，查询时按 person 的 steam 身份汇总，
-- 这样合并 / 拆分 / 撤销都不需要重算对位数据。

CREATE TABLE IF NOT EXISTS persons (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  -- 手动设置的显示名；NULL = 用最近一次出现时的游戏名
  display_name TEXT,
  note TEXT,
  -- JSON 字符串数组
  tags TEXT NOT NULL DEFAULT '[]',
  -- 「我关注的人」
  followed INTEGER NOT NULL DEFAULT 0,
  -- 由 users / sub_accounts 迁移来的人
  user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  -- 被合并后指向目标 person（行保留，用于撤销）
  merged_into INTEGER REFERENCES persons(id),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_persons_user ON persons(user_id);

CREATE TABLE IF NOT EXISTS identities (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  person_id INTEGER NOT NULL REFERENCES persons(id),
  platform TEXT NOT NULL,
  external_id TEXT NOT NULL,
  steamid INTEGER,
  sub_account_id TEXT REFERENCES sub_accounts(id) ON DELETE SET NULL,
  -- 最近一场里的名字 / 出现场数 / 最近出现时间（steam 身份由解析结果维护）
  last_name TEXT,
  match_count INTEGER NOT NULL DEFAULT 0,
  first_seen_at TEXT,
  last_seen_at TEXT,
  -- 1 = 用户手动拆分 / 撤销过，自动关联不再移动它
  manual INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(platform, external_id)
);
CREATE INDEX IF NOT EXISTS idx_identities_person ON identities(person_id);
CREATE INDEX IF NOT EXISTS idx_identities_steamid ON identities(steamid);

CREATE TABLE IF NOT EXISTS person_merges (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  target_id INTEGER NOT NULL REFERENCES persons(id),
  source_id INTEGER NOT NULL REFERENCES persons(id),
  -- JSON 数组：从 source 移到 target 的 identity id
  identity_ids TEXT NOT NULL,
  -- JSON：合并前两边的 display_name / note / tags / followed / user_id
  target_snapshot TEXT NOT NULL,
  source_snapshot TEXT NOT NULL,
  -- 1 = 自动关联产生的合并
  auto INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  undone_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_person_merges_target ON person_merges(target_id);

CREATE TABLE IF NOT EXISTS identity_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- 每人每场的胜负（按开局队伍分组，已处理换边）
CREATE TABLE IF NOT EXISTS h2h_player_results (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  steamid INTEGER NOT NULL,
  -- 开局阵营 2 = T, 3 = CT
  team INTEGER NOT NULL,
  rounds_won INTEGER NOT NULL,
  rounds_lost INTEGER NOT NULL,
  -- 1 胜 / 0 平 / -1 负
  result INTEGER NOT NULL,
  PRIMARY KEY (match_id, steamid)
);
CREATE INDEX IF NOT EXISTS idx_h2h_player_results_steamid ON h2h_player_results(steamid);

-- 每场每对选手的事实（a, b 两个方向都存）。删除对局时级联删除，触发器同步回 h2h_stats。
CREATE TABLE IF NOT EXISTS h2h_match (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  a INTEGER NOT NULL,
  b INTEGER NOT NULL,
  same_team INTEGER NOT NULL,
  -- a 的胜负：1 / 0 / -1
  result INTEGER NOT NULL,
  kills INTEGER NOT NULL DEFAULT 0,
  deaths INTEGER NOT NULL DEFAULT 0,
  damage INTEGER NOT NULL DEFAULT 0,
  damage_taken INTEGER NOT NULL DEFAULT 0,
  duels INTEGER NOT NULL DEFAULT 0,
  duels_won INTEGER NOT NULL DEFAULT 0,
  duels_lost INTEGER NOT NULL DEFAULT 0,
  opening_wins INTEGER NOT NULL DEFAULT 0,
  opening_losses INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (match_id, a, b)
);
CREATE INDEX IF NOT EXISTS idx_h2h_match_ab ON h2h_match(a, b);

-- 物化汇总：steamid a × steamid b（两个方向都存）。由 h2h_match 的触发器维护。
CREATE TABLE IF NOT EXISTS h2h_stats (
  a INTEGER NOT NULL,
  b INTEGER NOT NULL,
  matches_together INTEGER NOT NULL DEFAULT 0,
  wins_together INTEGER NOT NULL DEFAULT 0,
  losses_together INTEGER NOT NULL DEFAULT 0,
  matches_against INTEGER NOT NULL DEFAULT 0,
  wins_against INTEGER NOT NULL DEFAULT 0,
  losses_against INTEGER NOT NULL DEFAULT 0,
  kills INTEGER NOT NULL DEFAULT 0,
  deaths INTEGER NOT NULL DEFAULT 0,
  damage INTEGER NOT NULL DEFAULT 0,
  damage_taken INTEGER NOT NULL DEFAULT 0,
  duels INTEGER NOT NULL DEFAULT 0,
  duels_won INTEGER NOT NULL DEFAULT 0,
  duels_lost INTEGER NOT NULL DEFAULT 0,
  opening_wins INTEGER NOT NULL DEFAULT 0,
  opening_losses INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (a, b)
);

CREATE TRIGGER IF NOT EXISTS h2h_match_after_insert AFTER INSERT ON h2h_match
BEGIN
  INSERT INTO h2h_stats (a, b, matches_together, wins_together, losses_together,
    matches_against, wins_against, losses_against, kills, deaths, damage, damage_taken,
    duels, duels_won, duels_lost, opening_wins, opening_losses)
  VALUES (NEW.a, NEW.b,
    NEW.same_team,
    CASE WHEN NEW.same_team = 1 AND NEW.result = 1 THEN 1 ELSE 0 END,
    CASE WHEN NEW.same_team = 1 AND NEW.result = -1 THEN 1 ELSE 0 END,
    1 - NEW.same_team,
    CASE WHEN NEW.same_team = 0 AND NEW.result = 1 THEN 1 ELSE 0 END,
    CASE WHEN NEW.same_team = 0 AND NEW.result = -1 THEN 1 ELSE 0 END,
    NEW.kills, NEW.deaths, NEW.damage, NEW.damage_taken,
    NEW.duels, NEW.duels_won, NEW.duels_lost, NEW.opening_wins, NEW.opening_losses)
  ON CONFLICT (a, b) DO UPDATE SET
    matches_together = matches_together + excluded.matches_together,
    wins_together = wins_together + excluded.wins_together,
    losses_together = losses_together + excluded.losses_together,
    matches_against = matches_against + excluded.matches_against,
    wins_against = wins_against + excluded.wins_against,
    losses_against = losses_against + excluded.losses_against,
    kills = kills + excluded.kills,
    deaths = deaths + excluded.deaths,
    damage = damage + excluded.damage,
    damage_taken = damage_taken + excluded.damage_taken,
    duels = duels + excluded.duels,
    duels_won = duels_won + excluded.duels_won,
    duels_lost = duels_lost + excluded.duels_lost,
    opening_wins = opening_wins + excluded.opening_wins,
    opening_losses = opening_losses + excluded.opening_losses;
END;

CREATE TRIGGER IF NOT EXISTS h2h_match_after_delete AFTER DELETE ON h2h_match
BEGIN
  UPDATE h2h_stats SET
    matches_together = matches_together - OLD.same_team,
    wins_together = wins_together - (CASE WHEN OLD.same_team = 1 AND OLD.result = 1 THEN 1 ELSE 0 END),
    losses_together = losses_together - (CASE WHEN OLD.same_team = 1 AND OLD.result = -1 THEN 1 ELSE 0 END),
    matches_against = matches_against - (1 - OLD.same_team),
    wins_against = wins_against - (CASE WHEN OLD.same_team = 0 AND OLD.result = 1 THEN 1 ELSE 0 END),
    losses_against = losses_against - (CASE WHEN OLD.same_team = 0 AND OLD.result = -1 THEN 1 ELSE 0 END),
    kills = kills - OLD.kills,
    deaths = deaths - OLD.deaths,
    damage = damage - OLD.damage,
    damage_taken = damage_taken - OLD.damage_taken,
    duels = duels - OLD.duels,
    duels_won = duels_won - OLD.duels_won,
    duels_lost = duels_lost - OLD.duels_lost,
    opening_wins = opening_wins - OLD.opening_wins,
    opening_losses = opening_losses - OLD.opening_losses
  WHERE a = OLD.a AND b = OLD.b;
  DELETE FROM h2h_stats WHERE a = OLD.a AND b = OLD.b AND matches_together = 0 AND matches_against = 0;
END;
