-- CS Analyst migration 003: derived per-match statistics (W1-B, docs/roadmap.md P2).
-- Computed by the `stats` crate from the 002 event tables; recomputed when stats_version changes.
-- Teams are side-independent: 'A' = started on CT, 'B' = started on T. Sides: 2 = T, 3 = CT.
-- Blind metrics are NULL (not 0) when the demo has no player_blind events (ESL servers).

CREATE TABLE IF NOT EXISTS player_match_stats (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  steamid INTEGER NOT NULL,
  stats_version INTEGER NOT NULL,
  name TEXT NOT NULL,
  team TEXT NOT NULL CHECK (team IN ('A', 'B')),
  start_side INTEGER,
  -- 1 win / 0 loss / NULL draw
  won INTEGER,
  rounds_played INTEGER NOT NULL,
  rounds_t INTEGER NOT NULL,
  rounds_ct INTEGER NOT NULL,

  kills INTEGER NOT NULL,
  deaths INTEGER NOT NULL,
  assists INTEGER NOT NULL,
  flash_assists INTEGER NOT NULL,
  headshot_kills INTEGER NOT NULL,
  team_kills INTEGER NOT NULL,
  suicides INTEGER NOT NULL,
  damage INTEGER NOT NULL,
  kast_rounds INTEGER NOT NULL,
  rounds_survived INTEGER NOT NULL,
  mvps INTEGER NOT NULL,
  mk2 INTEGER NOT NULL,
  mk3 INTEGER NOT NULL,
  mk4 INTEGER NOT NULL,
  mk5 INTEGER NOT NULL,

  opening_kills_t INTEGER NOT NULL,
  opening_deaths_t INTEGER NOT NULL,
  opening_kills_ct INTEGER NOT NULL,
  opening_deaths_ct INTEGER NOT NULL,

  trade_kill_opportunities INTEGER NOT NULL,
  trade_kill_attempts INTEGER NOT NULL,
  trade_kills INTEGER NOT NULL,
  traded_death_opportunities INTEGER NOT NULL,
  traded_deaths INTEGER NOT NULL,

  clutch_1v1_attempts INTEGER NOT NULL, clutch_1v1_wins INTEGER NOT NULL,
  clutch_1v2_attempts INTEGER NOT NULL, clutch_1v2_wins INTEGER NOT NULL,
  clutch_1v3_attempts INTEGER NOT NULL, clutch_1v3_wins INTEGER NOT NULL,
  clutch_1v4_attempts INTEGER NOT NULL, clutch_1v4_wins INTEGER NOT NULL,
  clutch_1v5_attempts INTEGER NOT NULL, clutch_1v5_wins INTEGER NOT NULL,
  clutch_kills INTEGER NOT NULL,

  flashes_thrown INTEGER NOT NULL,
  he_thrown INTEGER NOT NULL,
  smokes_thrown INTEGER NOT NULL,
  molotovs_thrown INTEGER NOT NULL,
  decoys_thrown INTEGER NOT NULL,
  enemies_flashed INTEGER,
  enemy_blind_seconds REAL,
  teammates_flashed INTEGER,
  teammate_blind_seconds REAL,
  he_damage INTEGER NOT NULL,
  he_team_damage INTEGER NOT NULL,
  fire_damage INTEGER NOT NULL,
  unused_utility_value INTEGER NOT NULL,

  -- Denormalized derived values for sorting / filtering.
  adr REAL NOT NULL,
  kast REAL NOT NULL,
  hs_pct REAL NOT NULL,
  rating REAL NOT NULL,
  PRIMARY KEY (match_id, steamid)
);
CREATE INDEX IF NOT EXISTS idx_player_match_stats_steamid ON player_match_stats(steamid);

CREATE TABLE IF NOT EXISTS player_round_stats (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  steamid INTEGER NOT NULL,
  team TEXT NOT NULL CHECK (team IN ('A', 'B')),
  side INTEGER NOT NULL,
  kills INTEGER NOT NULL,
  died INTEGER NOT NULL,
  death_tick INTEGER,
  assists INTEGER NOT NULL,
  flash_assists INTEGER NOT NULL,
  headshot_kills INTEGER NOT NULL,
  damage INTEGER NOT NULL,
  utility_damage INTEGER NOT NULL,
  kast INTEGER NOT NULL,
  traded INTEGER NOT NULL,
  trade_kills INTEGER NOT NULL,
  opening_kill INTEGER NOT NULL,
  opening_death INTEGER NOT NULL,
  clutch_vs INTEGER,
  clutch_won INTEGER,
  mvp INTEGER NOT NULL,
  equip_value INTEGER NOT NULL,
  balance INTEGER NOT NULL,
  utility_thrown INTEGER NOT NULL,
  enemies_flashed INTEGER,
  unused_utility_value INTEGER NOT NULL,
  PRIMARY KEY (match_id, round, steamid)
);
CREATE INDEX IF NOT EXISTS idx_player_round_stats_steamid ON player_round_stats(steamid);

-- One row per team per round: economy, buy type and the score after the round.
CREATE TABLE IF NOT EXISTS team_round_stats (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  team TEXT NOT NULL CHECK (team IN ('A', 'B')),
  side INTEGER NOT NULL,
  won INTEGER NOT NULL,
  score INTEGER NOT NULL,
  equip_value INTEGER NOT NULL,
  balance INTEGER NOT NULL,
  buy_type TEXT NOT NULL CHECK (buy_type IN ('pistol', 'eco', 'force', 'full')),
  kills INTEGER NOT NULL,
  survivors INTEGER NOT NULL,
  PRIMARY KEY (match_id, round, team)
);
