-- CS Demo Analyst initial schema

CREATE TABLE IF NOT EXISTS app_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  data_root VARCHAR NOT NULL DEFAULT './',
  global_max_concurrent INTEGER NOT NULL DEFAULT 3,
  auto_discover_on_startup BOOLEAN NOT NULL DEFAULT TRUE,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO app_settings (id) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM app_settings WHERE id = 1);

CREATE TABLE IF NOT EXISTS platform_settings (
  platform VARCHAR PRIMARY KEY CHECK (platform IN ('valve', 'renown', '5eplay')),
  enabled BOOLEAN NOT NULL DEFAULT FALSE,
  auto_discover BOOLEAN NOT NULL DEFAULT TRUE,
  discover_interval_min INTEGER NOT NULL DEFAULT 60,
  max_concurrent_jobs INTEGER NOT NULL DEFAULT 1,
  auto_retry_failed BOOLEAN NOT NULL DEFAULT TRUE,
  max_auto_attempts INTEGER,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO platform_settings (platform) SELECT 'valve' WHERE NOT EXISTS (SELECT 1 FROM platform_settings WHERE platform = 'valve');
INSERT INTO platform_settings (platform) SELECT 'renown' WHERE NOT EXISTS (SELECT 1 FROM platform_settings WHERE platform = 'renown');
INSERT INTO platform_settings (platform) SELECT '5eplay' WHERE NOT EXISTS (SELECT 1 FROM platform_settings WHERE platform = '5eplay');

CREATE TABLE IF NOT EXISTS users (
  id VARCHAR PRIMARY KEY,
  display_name VARCHAR NOT NULL,
  note VARCHAR,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS sub_accounts (
  id VARCHAR PRIMARY KEY,
  user_id VARCHAR NOT NULL REFERENCES users(id),
  platform VARCHAR NOT NULL CHECK (platform IN ('valve', 'renown', '5eplay')),
  external_id VARCHAR NOT NULL,
  display_name VARCHAR NOT NULL,
  avatar_url VARCHAR,
  status VARCHAR NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled', 'error')),
  metadata JSON NOT NULL DEFAULT '{}',
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(platform, external_id)
);

CREATE TABLE IF NOT EXISTS valve_bot_config (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  username VARCHAR,
  password_enc VARCHAR,
  secret_enc VARCHAR,
  steam_api_key_enc VARCHAR,
  cron_schedule VARCHAR NOT NULL DEFAULT '0 * * * *',
  run_on_startup BOOLEAN NOT NULL DEFAULT TRUE,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO valve_bot_config (id) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM valve_bot_config WHERE id = 1);

CREATE TABLE IF NOT EXISTS discovered_matches (
  id VARCHAR PRIMARY KEY,
  platform VARCHAR NOT NULL CHECK (platform IN ('valve', 'renown', '5eplay')),
  sub_account_id VARCHAR NOT NULL REFERENCES sub_accounts(id),
  match_id VARCHAR NOT NULL,
  share_code VARCHAR,
  demo_url VARCHAR,
  map_name VARCHAR,
  played_at TIMESTAMP,
  valve_channel VARCHAR CHECK (valve_channel IN ('local', 'authcode')),
  raw_payload JSON NOT NULL DEFAULT '{}',
  discovered_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(platform, match_id)
);

CREATE INDEX IF NOT EXISTS idx_discovered_matches_share_code ON discovered_matches(share_code);

CREATE TABLE IF NOT EXISTS download_jobs (
  id VARCHAR PRIMARY KEY,
  discovered_match_id VARCHAR NOT NULL UNIQUE REFERENCES discovered_matches(id),
  status VARCHAR NOT NULL DEFAULT 'discovered' CHECK (status IN (
    'discovered', 'queued', 'downloading', 'completed',
    'failed', 'skipped', 'expired'
  )),
  progress DOUBLE NOT NULL DEFAULT 0.0,
  priority INTEGER NOT NULL DEFAULT 0,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error VARCHAR,
  failed_at TIMESTAMP,
  queued_at TIMESTAMP,
  started_at TIMESTAMP,
  finished_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS demos (
  id VARCHAR PRIMARY KEY,
  platform VARCHAR NOT NULL CHECK (platform IN ('valve', 'renown', '5eplay')),
  sub_account_id VARCHAR NOT NULL REFERENCES sub_accounts(id),
  match_id VARCHAR NOT NULL,
  share_code VARCHAR,
  file_path VARCHAR NOT NULL UNIQUE,
  file_name VARCHAR NOT NULL,
  file_size BIGINT,
  checksum VARCHAR,
  valve_channel VARCHAR CHECK (valve_channel IN ('local', 'authcode')),
  downloaded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(platform, match_id)
);

CREATE TABLE IF NOT EXISTS sync_cursors (
  sub_account_id VARCHAR PRIMARY KEY REFERENCES sub_accounts(id),
  cursor_type VARCHAR NOT NULL,
  cursor_value VARCHAR NOT NULL,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE OR REPLACE VIEW v_download_dashboard AS
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
