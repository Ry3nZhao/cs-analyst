// Aim analysis (W1-D): `get_aim_stats` / `get_sprays`. Mirrors src/tauri/src-tauri/src/aim/commands.rs.
//
// Conventions:
// - steamid is a decimal string (beyond JS safe integers).
// - Angles in degrees. Offsets are screen terms: v > 0 = up, h > 0 = right.
// - Times in ms. Rates are 0–1 fractions; `control` is 0–100.
// - Weapons use canonical names: ak47, m4a1 (= M4A4), m4a1_silencer, galilar, famas, mp9, mac10, ...
// - Visibility metrics (TTD, reaction, crosshair, visible / first-shot accuracy) only come from
//   matches whose spotted-mask data is reliable (`reliableMatches`); ESL / FACEIT-style server
//   demos often are not. Spray, accuracy and counter-strafe stats use every match.

/** Spray-analysed weapons, in display order. */
export const SPRAY_WEAPONS = ['ak47', 'm4a1', 'm4a1_silencer', 'galilar', 'famas', 'mp9', 'mac10'] as const;
export type SprayWeapon = (typeof SPRAY_WEAPONS)[number];

export interface AimFilters {
  matchIds?: string[];
  platforms?: string[];
  maps?: string[];
  /** Inclusive bounds on the match's playedAt (ISO strings). */
  from?: string;
  to?: string;
  /** Only the newest N matches (after the other filters). */
  lastMatches?: number;
  /** Canonical weapon; default = all weapons. */
  weapon?: string;
}

export interface Dist {
  n: number;
  mean: number | null;
  median: number | null;
  p25: number | null;
  p75: number | null;
}

export interface Histogram {
  /** Bin i covers [i × binWidth, (i + 1) × binWidth); the last bin also holds the overflow. */
  binWidth: number;
  counts: number[];
}

export interface ShotRates {
  shots: number;
  hits: number;
  headHits: number;
  accuracy: number | null;
  /** headHits / hits */
  headshotRate: number | null;
  visShots: number;
  visHits: number;
  /** Hits / shots fired while an enemy was visible. */
  visibleAccuracy: number | null;
  firstShots: number;
  firstHits: number;
  /** Hit rate of the first bullet of a burst fired while an enemy was visible. */
  firstShotAccuracy: number | null;
  csShots: number;
  csOk: number;
  /** First bullets fired at ≤ 34 % of the weapon's max speed. */
  counterStrafeRate: number | null;
}

export interface AimOverall extends ShotRates {
  /** Time to damage: visibility onset → first damage (ms). */
  ttd: Dist;
  /** Visibility onset → first shot (ms). */
  reaction: Dist;
  /** Angle between crosshair and the enemy's head at visibility onset (°). */
  crosshair: Dist;
  /** Signed vertical component (> 0 crosshair above the head). */
  crosshairV: Dist;
  crosshairVAbs: Dist;
  crosshairHAbs: Dist;
  ttdHistogram: Histogram;
  crosshairHistogram: Histogram;
  sprays: number;
  /** Mean spray control (0–100). */
  sprayControl: number | null;
  /** Mean spray error (°). */
  sprayErr: number | null;
}

export interface WeaponAim extends ShotRates {
  weapon: string;
  ttdMedianMs: number | null;
  crosshairMedianDeg: number | null;
  sprays: number;
  sprayControl: number | null;
  sprayErr: number | null;
  /** First bullet whose median error exceeds 2° (null = never / too few sprays). */
  breakShot: number | null;
}

export interface AimTrendPoint {
  matchId: string;
  playedAt: string | null;
  mapName: string | null;
  platform: string;
  visibilityReliable: boolean;
  ttdMedianMs: number | null;
  reactionMedianMs: number | null;
  crosshairMedianDeg: number | null;
  accuracy: number | null;
  counterStrafeRate: number | null;
  sprays: number;
  sprayControl: number | null;
}

export interface AimStats {
  steamid: string;
  weapon: string | null;
  matches: number;
  reliableMatches: number;
  /** Restricted to `weapon` when the filter is set. */
  overall: AimOverall;
  /** Every weapon used (ignores the weapon filter), most shots first. */
  weapons: WeaponAim[];
  /** Oldest → newest. */
  trend: AimTrendPoint[];
}

/** One bullet of a spray, relative to the spray's first bullet. */
export interface SprayShot {
  /** 1-based bullet number. */
  idx: number;
  tick: number;
  pitch: number;
  yaw: number;
  punchPitch: number;
  punchYaw: number;
  /** Where recoil alone moved the bullet (−2 × Δpunch) — the ideal compensation, mirrored. */
  recoilV: number;
  recoilH: number;
  /** How the player moved the crosshair (−Δview). */
  aimV: number;
  aimH: number;
  /** Where the bullet landed relative to the first one (= recoil + aim). errV > 0: under-compensated (压少了), < 0: over (压多了). */
  errV: number;
  errH: number;
  hit: boolean;
  hitgroup: string | null;
  /** Bullet relative to the target's head, when known. */
  tgtV: number | null;
  tgtH: number | null;
}

export interface Spray {
  id: number;
  matchId: string;
  steamid: string;
  round: number;
  /** 2 = T, 3 = CT */
  team: number | null;
  weapon: string;
  startTick: number;
  endTick: number;
  hits: number;
  headHits: number;
  /** Enemy that took the most damage. */
  target: string | null;
  killed: boolean;
  /** Horizontal speed at the first bullet (u/s). */
  startSpeed: number | null;
  /** Mean |err| over bullets 2..n (°). */
  meanErr: number;
  meanErrV: number;
  meanErrH: number;
  /** Share of recoil cancelled out, 0–100. */
  control: number | null;
  /** First bullet with |err| > 2°. */
  breakShot: number | null;
  shots: SprayShot[];
}

export interface BulletAgg {
  idx: number;
  /** Sprays that reached this bullet. */
  n: number;
  recoilV: number;
  recoilH: number;
  aimV: number;
  aimH: number;
  errV: number;
  errH: number;
  /** Mean |err|. */
  err: number;
  errMedian: number;
}

export interface WeaponPattern {
  weapon: string;
  sprays: number;
  shots: number;
  hits: number;
  control: number | null;
  meanErr: number | null;
  /** First bullet whose median |err| exceeds 2° (≥ 3 sprays). */
  breakShot: number | null;
  bullets: BulletAgg[];
}

export interface SpraysResult {
  /** One match: chronological. All matches: newest first, at most 500. */
  sprays: Spray[];
  truncated: boolean;
  patterns: WeaponPattern[];
}

/** `invoke('get_aim_stats', GetAimStatsArgs)` */
export interface GetAimStatsArgs {
  steamid: string;
  filters?: AimFilters | null;
}

/** `invoke('get_sprays', GetSpraysArgs)` — omit matchId for all matches, weapon for all spray weapons. */
export interface GetSpraysArgs {
  matchId?: string | null;
  steamid: string;
  weapon?: SprayWeapon | null;
}
