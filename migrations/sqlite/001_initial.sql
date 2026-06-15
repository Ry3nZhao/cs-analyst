-- CS Demo Analyst initial schema

CREATE TABLE IF NOT EXISTS app_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  data_root TEXT NOT NULL DEFAULT './',
  global_max_concurrent INTEGER NOT NULL DEFAULT 3,
  auto_discover_on_startup INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO app_settings (id) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM app_settings WHERE id = 1);

CREATE TABLE IF NOT EXISTS platform_settings (
  platform TEXT PRIMARY KEY CHECK (platform IN ('valve', 'renown', '5eplay')),
  enabled INTEGER NOT NULL DEFAULT 0,
  auto_discover INTEGER NOT NULL DEFAULT 1,
  discover_interval_min INTEGER NOT NULL DEFAULT 60,
  max_concurrent_jobs INTEGER NOT NULL DEFAULT 1,
  auto_retry_failed INTEGER NOT NULL DEFAULT 1,
  max_auto_attempts INTEGER,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO platform_settings (platform) SELECT 'valve' WHERE NOT EXISTS (SELECT 1 FROM platform_settings WHERE platform = 'valve');
INSERT INTO platform_settings (platform) SELECT 'renown' WHERE NOT EXISTS (SELECT 1 FROM platform_settings WHERE platform = 'renown');
INSERT INTO platform_settings (platform) SELECT '5eplay' WHERE NOT EXISTS (SELECT 1 FROM platform_settings WHERE platform = '5eplay');

CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  note TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS sub_accounts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  platform TEXT NOT NULL CHECK (platform IN ('valve', 'renown', '5eplay')),
  external_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  avatar_url TEXT,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled', 'error')),
  metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(platform, external_id)
);

CREATE TABLE IF NOT EXISTS valve_bot_config (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  username TEXT,
  password_enc TEXT,
  secret_enc TEXT,
  steam_api_key_enc TEXT,
  cron_schedule TEXT NOT NULL DEFAULT '0 * * * *',
  run_on_startup INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO valve_bot_config (id) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM valve_bot_config WHERE id = 1);

CREATE TABLE IF NOT EXISTS discovered_matches (
  id TEXT PRIMARY KEY,
  platform TEXT NOT NULL CHECK (platform IN ('valve', 'renown', '5eplay')),
  sub_account_id TEXT NOT NULL REFERENCES sub_accounts(id),
  match_id TEXT NOT NULL,
  share_code TEXT,
  demo_url TEXT,
  map_name TEXT,
  played_at TEXT,
  valve_channel TEXT CHECK (valve_channel IN ('local', 'authcode')),
  raw_payload TEXT NOT NULL DEFAULT '{}',
  discovered_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(platform, match_id)
);

CREATE INDEX IF NOT EXISTS idx_discovered_matches_share_code ON discovered_matches(share_code);

CREATE TABLE IF NOT EXISTS download_jobs (
  id TEXT PRIMARY KEY,
  discovered_match_id TEXT NOT NULL UNIQUE REFERENCES discovered_matches(id),
  status TEXT NOT NULL DEFAULT 'discovered' CHECK (status IN (
    'discovered', 'queued', 'downloading', 'completed',
    'failed', 'skipped', 'expired'
  )),
  progress REAL NOT NULL DEFAULT 0.0,
  priority INTEGER NOT NULL DEFAULT 0,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  failed_at TEXT,
  queued_at TEXT,
  started_at TEXT,
  finished_at TEXT
);

CREATE TABLE IF NOT EXISTS demos (
  id TEXT PRIMARY KEY,
  platform TEXT NOT NULL CHECK (platform IN ('valve', 'renown', '5eplay')),
  sub_account_id TEXT NOT NULL REFERENCES sub_accounts(id),
  match_id TEXT NOT NULL,
  share_code TEXT,
  file_path TEXT NOT NULL UNIQUE,
  file_name TEXT NOT NULL,
  file_size INTEGER,
  checksum TEXT,
  valve_channel TEXT CHECK (valve_channel IN ('local', 'authcode')),
  downloaded_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(platform, match_id)
);

CREATE TABLE IF NOT EXISTS sync_cursors (
  sub_account_id TEXT PRIMARY KEY REFERENCES sub_accounts(id),
  cursor_type TEXT NOT NULL,
  cursor_value TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

DROP VIEW IF EXISTS v_download_dashboard;
CREATE VIEW v_download_dashboard AS
SELECT
  j.id AS job_id,
  j.status,
  j.progress,
  j.attempts,
  j.last_error,
  dm.platform,
  dm.match_id,
  dm.map_name,
  dm.played_at,
  dm.valve_channel,
  dm.share_code,
  dm.demo_url,
  u.id AS user_id,
  u.display_name AS user_name,
  sa.display_name AS account_name,
  sa.external_id AS account_id
FROM download_jobs j
JOIN discovered_matches dm ON dm.id = j.discovered_match_id
JOIN sub_accounts sa ON sa.id = dm.sub_account_id
JOIN users u ON u.id = sa.user_id;
