-- CS Analyst migration 006: Swing (W2-B, docs/roadmap.md 2.2, docs/stats.md「Swing」).
-- Swing values are percentage points of round win probability. Written together with the 003
-- tables by the stats module; rows from older stats versions are recomputed by the backfill.

-- Per player per round.
ALTER TABLE player_round_stats ADD COLUMN swing REAL;
-- Per player per match: sum over rounds, and average per round played.
ALTER TABLE player_match_stats ADD COLUMN swing_total REAL;
ALTER TABLE player_match_stats ADD COLUMN swing REAL;

-- Win-probability curve: one row per event (seq 0 = freeze end, last = round end).
CREATE TABLE IF NOT EXISTS round_win_probability (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  round INTEGER NOT NULL,
  seq INTEGER NOT NULL,
  tick INTEGER NOT NULL,
  time_secs REAL NOT NULL,
  -- start | damage | kill | team_kill | suicide | world | planted | defused | exploded | end
  kind TEXT NOT NULL,
  player INTEGER,
  victim INTEGER,
  t_alive INTEGER NOT NULL,
  ct_alive INTEGER NOT NULL,
  bomb_planted INTEGER NOT NULL,
  -- T win probability (0–1) before / after the event
  p_before REAL NOT NULL,
  p_after REAL NOT NULL,
  PRIMARY KEY (match_id, round, seq)
);

-- State buckets visited per match, for local smoothing of the win model.
-- One row per (match, T alive, CT alive, planted): rounds passing through it and T wins among them.
CREATE TABLE IF NOT EXISTS win_prob_samples (
  match_id TEXT NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
  t_alive INTEGER NOT NULL,
  ct_alive INTEGER NOT NULL,
  planted INTEGER NOT NULL,
  rounds INTEGER NOT NULL,
  t_wins INTEGER NOT NULL,
  PRIMARY KEY (match_id, t_alive, ct_alive, planted)
);
