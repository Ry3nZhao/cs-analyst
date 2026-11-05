-- CS Analyst migration 005: aim analysis (W1-D) — see docs/roadmap.md 2.3 / 2.4 and crates/aim.
-- SteamIDs are INTEGER. Angles in degrees, offsets in screen terms (v > 0 up, h > 0 right),
-- times in ms. Weapons use canonical names (ak47, m4a1 = M4A4, m4a1_silencer, galilar, ...).
-- Every row belongs to a match and is deleted with it.

-- One row per analysed match: algorithm version (aim::AIM_VERSION) and visibility quality.
CREATE TABLE IF NOT EXISTS match_aim (
  match_id TEXT PRIMARY KEY REFERENCES matches(id) ON DELETE CASCADE,
  aim_version INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('done', 'failed')),
  error TEXT,
  -- Share of first-damage contacts where the attacker saw the victim (spotted mask).
  visibility_quality REAL,
  -- Visibility metrics (TTD, crosshair, reaction, visible accuracy) only exist when 1.
  visibility_reliable INTEGER NOT NULL DEFAULT 0,
  analysis_ms INTEGER,
  analyzed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Per player × weapon summary of one match; weapon = '*' is the all-weapons row.
CREATE TABLE IF NOT EXISTS player_match_aim (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  steamid INTEGER NOT NULL,
  weapon TEXT NOT NULL,
  shots INTEGER NOT NULL,
  hits INTEGER NOT NULL,
  head_hits INTEGER NOT NULL,
  -- NULL when visibility is unreliable.
  vis_shots INTEGER,
  vis_hits INTEGER,
  first_shots INTEGER,
  first_hits INTEGER,
  cs_shots INTEGER NOT NULL,
  cs_ok INTEGER NOT NULL,
  ttd_n INTEGER NOT NULL,
  ttd_median_ms REAL,
  ttd_mean_ms REAL,
  reaction_n INTEGER NOT NULL,
  reaction_median_ms REAL,
  reaction_mean_ms REAL,
  cp_n INTEGER NOT NULL,
  cp_median_deg REAL,
  cp_mean_deg REAL,
  cp_v_median_abs REAL,
  cp_h_median_abs REAL,
  -- Signed mean vertical offset: < 0 crosshair tends to sit below the head.
  cp_v_mean REAL,
  sprays INTEGER NOT NULL,
  spray_control REAL,
  spray_err REAL,
  PRIMARY KEY (match_id, steamid, weapon)
);
CREATE INDEX IF NOT EXISTS idx_player_match_aim_steamid ON player_match_aim(steamid, weapon);

-- One row per contact sample (crates/aim/src/engage.rs): the observer saw the target at
-- onset_tick; role 'attacker' = observer dealt the contact's first damage at damage_tick.
CREATE TABLE IF NOT EXISTS aim_engagements (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  observer INTEGER NOT NULL,
  target INTEGER NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('attacker', 'victim')),
  weapon TEXT,
  onset_tick INTEGER NOT NULL,
  damage_tick INTEGER NOT NULL,
  ttd_ms REAL,
  reaction_ms REAL,
  cp_deg REAL,
  cp_v REAL,
  cp_h REAL,
  distance REAL,
  killed INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_aim_engagements_match ON aim_engagements(match_id, round);
CREATE INDEX IF NOT EXISTS idx_aim_engagements_observer ON aim_engagements(observer);

-- Sprays of the spray-analysed weapons (crates/aim/src/spray.rs).
CREATE TABLE IF NOT EXISTS sprays (
  id INTEGER PRIMARY KEY,
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  steamid INTEGER NOT NULL,
  round INTEGER NOT NULL,
  team INTEGER,
  weapon TEXT NOT NULL,
  start_tick INTEGER NOT NULL,
  end_tick INTEGER NOT NULL,
  shots INTEGER NOT NULL,
  hits INTEGER NOT NULL,
  head_hits INTEGER NOT NULL,
  target INTEGER,
  killed INTEGER NOT NULL,
  start_speed REAL,
  mean_err REAL NOT NULL,
  mean_err_v REAL NOT NULL,
  mean_err_h REAL NOT NULL,
  control REAL,
  break_shot INTEGER
);
CREATE INDEX IF NOT EXISTS idx_sprays_match ON sprays(match_id, steamid);
CREATE INDEX IF NOT EXISTS idx_sprays_player ON sprays(steamid, weapon);

-- Every bullet of a spray, relative to the first bullet.
CREATE TABLE IF NOT EXISTS spray_shots (
  spray_id INTEGER NOT NULL REFERENCES sprays(id) ON DELETE CASCADE,
  idx INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  pitch REAL NOT NULL,
  yaw REAL NOT NULL,
  punch_pitch REAL NOT NULL,
  punch_yaw REAL NOT NULL,
  -- recoil = where recoil alone moved the bullet (-2 × Δpunch); aim = crosshair movement (-Δview);
  -- err = recoil + aim (err_v > 0: under-compensated, < 0: over-compensated).
  recoil_v REAL NOT NULL,
  recoil_h REAL NOT NULL,
  aim_v REAL NOT NULL,
  aim_h REAL NOT NULL,
  err_v REAL NOT NULL,
  err_h REAL NOT NULL,
  hit INTEGER NOT NULL,
  hitgroup TEXT,
  -- Bullet relative to the target's head, when known.
  tgt_v REAL,
  tgt_h REAL,
  PRIMARY KEY (spray_id, idx)
);
