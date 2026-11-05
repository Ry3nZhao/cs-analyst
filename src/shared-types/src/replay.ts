// 2D replay & heatmaps (W2-E). Mirrors src/tauri/src-tauri/src/replay/*.rs.
//
// Conventions:
// - steamid is a decimal string. Sides: 2 = T, 3 = CT. World coordinates in Hammer units.
// - `get_replay` returns a raw ArrayBuffer (Tauri binary IPC), not JSON:
//     0   "CSRP"            magic
//     4   u32               REPLAY_PACK_VERSION
//     8   u32               header length H (multiple of 4)
//     12  H bytes           ReplayHeader JSON (UTF-8, space padded)
//     12+H body             columns at `header.columns.<name>` bytes from the body start:
//         ticks i32[nFrames]
//         x, y, z, yaw f32[nFrames × nPlayers]   (index = frame * nPlayers + slot; yaw in degrees)
//         health, armor, flags u8[nFrames × nPlayers]
//         spottedBy u16[nFrames × nPlayers]      (bit i = slot i sees this player)
//   One frame every `tickStep` (4) ticks. All offsets are 4-byte aligned, so typed arrays view
//   the buffer without copying.

import type { BuyType } from './stats.js';

export const REPLAY_PACK_VERSION = 1;

/** `flags` bits per player per frame. */
export const REPLAY_FLAG = {
  alive: 1,
  ct: 2,
  scoped: 4,
  flashed: 8,
} as const;

export interface ReplayPlayer {
  /** Column index. */
  slot: number;
  steamid: string;
  name: string;
  teamStart: 2 | 3 | null;
}

export interface ReplayRound {
  number: number;
  startTick: number;
  freezeEndTick: number | null;
  endTick: number;
  winner: 2 | 3 | null;
  reason: string | null;
  /** Score after this round, by side (null when stats are missing). */
  scoreT: number | null;
  scoreCt: number | null;
}

export type Vec3 = [number, number, number];

export interface ReplayKill {
  tick: number;
  round: number;
  attacker: string | null;
  victim: string | null;
  assister: string | null;
  weapon: string;
  headshot: boolean;
  penetrated: boolean;
  noscope: boolean;
  thrusmoke: boolean;
  attackerBlind: boolean;
  assistedFlash: boolean;
  attackerPos: Vec3 | null;
  victimPos: Vec3 | null;
  attackerTeam: 2 | 3 | null;
  victimTeam: 2 | 3 | null;
}

export type GrenadeKind = 'flashbang' | 'hegrenade' | 'smokegrenade' | 'inferno' | 'decoy';

/** Detonation (or burn start for `inferno`). */
export interface ReplayGrenade {
  tick: number;
  round: number;
  thrower: string | null;
  kind: GrenadeKind | string;
  pos: Vec3 | null;
}

export interface ReplayBombEvent {
  tick: number;
  round: number;
  player: string | null;
  kind: 'plant_begin' | 'planted' | 'defuse_begin' | 'defused' | 'exploded';
  site: number | null;
  pos: Vec3 | null;
}

export interface ReplayColumns {
  ticks: number;
  x: number;
  y: number;
  z: number;
  yaw: number;
  health: number;
  armor: number;
  flags: number;
  spottedBy: number;
  bodyLen: number;
}

export interface ReplayHeader {
  matchId: string;
  mapName: string | null;
  platform: string;
  playedAt: string | null;
  tickRate: number;
  tickStep: number;
  nFrames: number;
  nPlayers: number;
  players: ReplayPlayer[];
  rounds: ReplayRound[];
  kills: ReplayKill[];
  grenades: ReplayGrenade[];
  bomb: ReplayBombEvent[];
  columns: ReplayColumns;
}

// ---- Heatmap (`get_heatmap_points`)

export type HeatKind = 'kill' | 'death' | 'presence';
/** Numeric kind codes in `HeatmapResult.kind`. */
export const HEAT_KIND = { kill: 0, death: 1, presence: 2 } as const;

export interface HeatmapFilters {
  matchIds?: string[];
  maps?: string[];
  /** Kill points use the attacker, death points the victim, presence the player. */
  steamids?: string[];
  /** Side of the player the point belongs to. */
  side?: 2 | 3;
  /** That player's team buy type in the round. */
  buyTypes?: BuyType[];
  rounds?: number[];
  /** Default ['kill', 'death']. `presence` = one sample per second of live play from replay files. */
  kinds?: HeatKind[];
  /** Default 200 000. */
  limit?: number;
}

/** Column-wise points. */
export interface HeatmapResult {
  kind: number[];
  x: number[];
  y: number[];
  z: number[];
  side: number[];
  matchIndex: number[];
  round: number[];
  matches: { id: string; mapName: string | null; playedAt: string | null }[];
  /** Points per map, most first (draw one map at a time). */
  maps: { mapName: string | null; points: number }[];
  truncated: boolean;
}

// ---- Radar backdrop (`get_radar`, `get_radar_image`, `get_radar_settings`, `update_radar_settings`)

export interface RadarSettings {
  /** Folder with `<map>_radar.png|jpg|webp` (or `<map>.png`), optional `<map>_lower_radar.*` and `<map>.txt`. */
  imageDir?: string | null;
  /** CS2 install (`…/Counter-Strike Global Offensive` or its `game/csgo`); null = detect via Steam. */
  cs2Dir?: string | null;
  /** false = never read the CS2 install. */
  useCs2?: boolean | null;
}

/** pixel = ((x − posX) / scale, (posY − y) / scale) on the 1024 px image. */
export interface RadarCalibration {
  posX: number;
  posY: number;
  scale: number;
  /** Below this z the `lower` image applies (Nuke, Vertigo, Train). */
  lowerMaxZ: number | null;
}

export interface RadarInfo {
  mapName: string;
  calibration: RadarCalibration | null;
  calibrationSource: 'user' | 'cs2' | 'builtin' | null;
  /** Images available; `get_radar_image(mapName, level)` returns their bytes. */
  levels: { level: 'default' | 'lower'; source: 'user' | 'cs2' }[];
  cs2Dir: string | null;
  settings: RadarSettings;
  note: string | null;
}
