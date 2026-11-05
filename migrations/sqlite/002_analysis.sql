-- CS Analyst migration 002: demo analysis (parsed events) — see docs/roadmap.md P1.
-- SteamIDs are stored as INTEGER (Steam64 fits in i64). Team numbers: 2 = T, 3 = CT.
-- All analysis rows belong to a match and are deleted with it.

ALTER TABLE matches ADD COLUMN analysis_status TEXT
  CHECK (analysis_status IN ('pending', 'running', 'done', 'failed'));
ALTER TABLE matches ADD COLUMN analysis_error TEXT;
ALTER TABLE matches ADD COLUMN analysis_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE matches ADD COLUMN analyzer_version INTEGER;
ALTER TABLE matches ADD COLUMN analyzed_at TEXT;
ALTER TABLE matches ADD COLUMN analysis_ms INTEGER;
-- Relative to data_root, e.g. analysis/<match id>.replay.zst
ALTER TABLE matches ADD COLUMN replay_path TEXT;
ALTER TABLE matches ADD COLUMN rounds_played INTEGER;

CREATE INDEX IF NOT EXISTS idx_matches_analysis ON matches(analysis_status);

CREATE TABLE IF NOT EXISTS match_players (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  steamid INTEGER NOT NULL,
  slot INTEGER NOT NULL,
  name TEXT NOT NULL,
  team_start INTEGER,
  team_end INTEGER,
  rank INTEGER,
  rank_type INTEGER,
  PRIMARY KEY (match_id, steamid)
);
CREATE INDEX IF NOT EXISTS idx_match_players_steamid ON match_players(steamid);

CREATE TABLE IF NOT EXISTS rounds (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  number INTEGER NOT NULL,
  start_tick INTEGER NOT NULL,
  freeze_end_tick INTEGER,
  end_tick INTEGER NOT NULL,
  winner INTEGER,
  reason TEXT,
  PRIMARY KEY (match_id, number)
);

CREATE TABLE IF NOT EXISTS kills (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  attacker INTEGER,
  victim INTEGER,
  assister INTEGER,
  weapon TEXT NOT NULL,
  headshot INTEGER NOT NULL,
  penetrated INTEGER NOT NULL,
  noscope INTEGER NOT NULL,
  thrusmoke INTEGER NOT NULL,
  attacker_blind INTEGER NOT NULL,
  attacker_in_air INTEGER NOT NULL,
  assisted_flash INTEGER NOT NULL,
  distance REAL,
  attacker_x REAL, attacker_y REAL, attacker_z REAL,
  victim_x REAL, victim_y REAL, victim_z REAL,
  attacker_team INTEGER,
  victim_team INTEGER
);
CREATE INDEX IF NOT EXISTS idx_kills_match ON kills(match_id, round);
CREATE INDEX IF NOT EXISTS idx_kills_attacker ON kills(attacker);
CREATE INDEX IF NOT EXISTS idx_kills_victim ON kills(victim);

CREATE TABLE IF NOT EXISTS damages (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  attacker INTEGER,
  victim INTEGER,
  weapon TEXT NOT NULL,
  dmg_health INTEGER NOT NULL,
  dmg_armor INTEGER NOT NULL,
  health_after INTEGER NOT NULL,
  armor_after INTEGER NOT NULL,
  hitgroup TEXT,
  attacker_team INTEGER,
  victim_team INTEGER
);
CREATE INDEX IF NOT EXISTS idx_damages_match ON damages(match_id, round);
CREATE INDEX IF NOT EXISTS idx_damages_attacker ON damages(attacker);

CREATE TABLE IF NOT EXISTS shots (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  shooter INTEGER NOT NULL,
  weapon TEXT NOT NULL,
  x REAL, y REAL, z REAL,
  pitch REAL, yaw REAL,
  punch_pitch REAL, punch_yaw REAL,
  vel_x REAL, vel_y REAL, vel_z REAL,
  shots_fired INTEGER,
  accuracy_penalty REAL,
  is_scoped INTEGER,
  team INTEGER
);
CREATE INDEX IF NOT EXISTS idx_shots_match ON shots(match_id, round);
CREATE INDEX IF NOT EXISTS idx_shots_shooter ON shots(shooter, weapon);

CREATE TABLE IF NOT EXISTS grenades (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  thrower INTEGER,
  kind TEXT NOT NULL,
  entity_id INTEGER,
  x REAL, y REAL, z REAL
);
CREATE INDEX IF NOT EXISTS idx_grenades_match ON grenades(match_id, round);

CREATE TABLE IF NOT EXISTS blinds (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  attacker INTEGER,
  victim INTEGER,
  duration REAL NOT NULL,
  attacker_team INTEGER,
  victim_team INTEGER
);
CREATE INDEX IF NOT EXISTS idx_blinds_match ON blinds(match_id, round);

CREATE TABLE IF NOT EXISTS bomb_events (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  player INTEGER,
  kind TEXT NOT NULL CHECK (kind IN ('plant_begin', 'planted', 'defuse_begin', 'defused', 'exploded')),
  site INTEGER,
  x REAL, y REAL, z REAL
);
CREATE INDEX IF NOT EXISTS idx_bomb_events_match ON bomb_events(match_id, round);

CREATE TABLE IF NOT EXISTS purchases (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  player INTEGER NOT NULL,
  item TEXT NOT NULL,
  cost INTEGER,
  was_sold INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_purchases_match ON purchases(match_id, round);

CREATE TABLE IF NOT EXISTS economies (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  player INTEGER NOT NULL,
  team INTEGER,
  balance INTEGER,
  equip_value INTEGER,
  round_start_equip_value INTEGER,
  cash_spent_this_round INTEGER,
  armor INTEGER,
  has_helmet INTEGER,
  has_defuser INTEGER,
  PRIMARY KEY (match_id, round, player)
);
