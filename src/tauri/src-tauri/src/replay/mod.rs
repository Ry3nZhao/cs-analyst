//! W2-E 2D replay and heatmaps (P6). Read-only on top of the analysis tables (`kills`,
//! `grenades`, `bomb_events`, `rounds`, `team_round_stats`) and the per-match replay file
//! `analysis/<id>.replay.zst` written by the parser. No migration.
//!
//! - `get_replay(matchId)`: one binary `ArrayBuffer` (header JSON + typed-array columns), see
//!   [`pack`].
//! - `get_heatmap_points(filters)`: kill / death / standing points across matches, see
//!   [`heatmap`].
//! - `get_radar(mapName)` / `get_radar_image(mapName, level)` / `get_radar_settings` /
//!   `update_radar_settings`: radar backdrop from the user's CS2 install or an image folder, see
//!   [`radar`].

pub mod commands;
pub mod heatmap;
pub mod pack;
pub mod radar;

#[cfg(test)]
mod tests;
