-- CS Analyst migration 010: 高光 / 失误集锦（W3-A，docs/highlights.md）。
-- 由 crates/highlights 的规则从已入库的事件表算出，不重新解析 demo。
-- 规则版本（highlights::RULES_VERSION）或依赖的 stats / aim 结果变了，启动回填会整场重算。

-- 每个识别出的时刻一行。score 是规则原始分（0–100），用户的权重和开关在查询时再应用。
CREATE TABLE IF NOT EXISTS highlights (
  id INTEGER PRIMARY KEY,
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  start_tick INTEGER NOT NULL,
  end_tick INTEGER NOT NULL,
  steamid INTEGER NOT NULL,
  kind TEXT NOT NULL,
  category TEXT NOT NULL CHECK (category IN ('highlight', 'mistake')),
  score REAL NOT NULL,
  -- 规则相关的细节（JSON），steamid 为十进制字符串
  detail TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_highlights_player ON highlights(steamid, category, kind);
CREATE INDEX IF NOT EXISTS idx_highlights_match ON highlights(match_id, round);

-- 每场的计算记录：用哪个规则版本、基于哪个 stats 版本、当时有没有枪法数据。
CREATE TABLE IF NOT EXISTS match_highlights (
  match_id TEXT PRIMARY KEY REFERENCES matches(id) ON DELETE CASCADE,
  rules_version INTEGER NOT NULL,
  stats_version INTEGER,
  aim_done INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL CHECK (status IN ('done', 'failed')),
  error TEXT,
  count INTEGER NOT NULL DEFAULT 0,
  -- 这场能产出结果的规则（JSON 数组），例如没有致盲数据时不含 team_flash
  available TEXT NOT NULL DEFAULT '[]',
  computed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 用户对规则的设置；没有行 = 默认（开启、默认权重）。
CREATE TABLE IF NOT EXISTS highlight_rules (
  kind TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  weight REAL NOT NULL DEFAULT 1.0
);

-- 在 CS2 中播放的设置（单行）。netcon_enabled 是实验功能，默认关闭。
CREATE TABLE IF NOT EXISTS highlight_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  -- 提前多少秒开始播放
  lead_secs INTEGER NOT NULL DEFAULT 5,
  netcon_enabled INTEGER NOT NULL DEFAULT 0,
  netcon_port INTEGER NOT NULL DEFAULT 2121,
  -- 附加的 CS2 启动参数（例如 -tools / -insecure），只在启动 CS2 时使用
  extra_launch_args TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO highlight_settings (id) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM highlight_settings WHERE id = 1);
