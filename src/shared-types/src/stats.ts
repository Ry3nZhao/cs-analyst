// Derived match / player statistics (W1-B) and Swing (W2-B). Mirrors the JSON of the Rust commands
// `get_match_detail`, `list_player_stats` and `get_round_win_probability`
// (src/tauri/src-tauri/src/stats, crates/stats).
// Metric definitions: docs/stats.md.

/** Steam64 as a decimal string (exceeds Number.MAX_SAFE_INTEGER). */
export type SteamId = string;

/** Engine side number: 2 = T, 3 = CT. */
export type Side = 2 | 3;

/** Side-independent team: A started on CT, B started on T. */
export type TeamId = 'A' | 'B';

export type BuyType = 'pistol' | 'eco' | 'force' | 'full';

/** Rates derived from the counts in {@link PlayerMatchStatsDto}. Percentages are 0–100. */
export interface DerivedStatsDto {
  kd: number;
  adr: number;
  kast: number;
  hsPct: number;
  kpr: number;
  dpr: number;
  apr: number;
  impact: number;
  /** HLTV Rating 2.0 (public approximation, reference only). */
  rating: number;
  survivalRate: number;
  openingAttempts: number;
  openingSuccessPct: number;
  tradeKillPct: number;
  tradedDeathPct: number;
  utilityDamage: number;
  utilityThrownPerRound: number;
  heDamagePerHe: number;
  /** null when blind data is unavailable (ESL demos). */
  enemiesFlashedPerFlash: number | null;
  enemyBlindSecondsPerFlash: number | null;
  teammatesFlashedPerFlash: number | null;
  /**
   * Average Swing per round played, in percentage points of round win probability
   * (e.g. 3.2 = +3.2 %). `swingTotal / roundsPlayed`. See docs/stats.md「Swing」.
   */
  swing: number;
}

/**
 * One player's stats for one match, or an aggregate over several (`matches` > 1, `team` null).
 * All counts are additive; `derived` is recomputed from them.
 */
export interface PlayerMatchStatsDto {
  steamid: SteamId;
  name: string;
  team: TeamId | null;
  /** Side in the player's first round. */
  startSide: Side | null;
  matches: number;
  wins: number;
  losses: number;
  draws: number;
  roundsPlayed: number;
  roundsT: number;
  roundsCt: number;

  kills: number;
  deaths: number;
  assists: number;
  flashAssists: number;
  headshotKills: number;
  teamKills: number;
  suicides: number;
  /** Effective damage to enemies (capped at remaining health). */
  damage: number;
  kastRounds: number;
  roundsSurvived: number;
  /** Approximated from the engine rule (planter / defuser / top fragger of the winners). */
  mvps: number;
  /** Index = kills in a round; use [2]..[5] for 2K–5K. */
  multiKills: number[];

  openingKillsT: number;
  openingDeathsT: number;
  openingKillsCt: number;
  openingDeathsCt: number;

  /** Teammate died while this player was alive (5 s window). */
  tradeKillOpportunities: number;
  /** ...and this player damaged the killer within 5 s. */
  tradeKillAttempts: number;
  /** ...and killed the killer within 5 s. */
  tradeKills: number;
  /** This player died with teammates alive. */
  tradedDeathOpportunities: number;
  /** ...and the killer died to a teammate within 5 s. */
  tradedDeaths: number;

  /** Index = enemies alive when the player became the last of their side (1..5). */
  clutchAttempts: number[];
  clutchWins: number[];
  clutchKills: number;

  flashesThrown: number;
  heThrown: number;
  smokesThrown: number;
  molotovsThrown: number;
  decoysThrown: number;
  /** Flashes thrown in matches with blind data (denominator for per-flash averages). */
  blindFlashes: number;
  /** null when blind data is unavailable — not 0. */
  enemiesFlashed: number | null;
  enemyBlindSeconds: number | null;
  teammatesFlashed: number | null;
  teammateBlindSeconds: number | null;
  heDamage: number;
  heTeamDamage: number;
  fireDamage: number;
  /** Approximate value of grenades still held at death, summed over deaths. */
  unusedUtilityValue: number;
  /** Sum of per-round Swing (percentage points); additive across matches. */
  swingTotal: number;

  derived: DerivedStatsDto;
}

export interface PlayerRoundStatsDto {
  round: number;
  steamid: SteamId;
  team: TeamId;
  side: Side;
  kills: number;
  died: boolean;
  deathTick: number | null;
  assists: number;
  flashAssists: number;
  headshotKills: number;
  damage: number;
  utilityDamage: number;
  kast: boolean;
  traded: boolean;
  tradeKills: number;
  openingKill: boolean;
  openingDeath: boolean;
  /** Enemies alive when this player became the last alive of their side. */
  clutchVs: number | null;
  clutchWon: boolean | null;
  mvp: boolean;
  /** Freeze-end equipment value plus purchases made after freeze end. */
  equipValue: number;
  /** Money at freeze end. */
  balance: number;
  utilityThrown: number;
  enemiesFlashed: number | null;
  unusedUtilityValue: number;
  /** Swing this round, in percentage points of round win probability. */
  swing: number;
}

/** One team in one round (economy curve + score progression). */
export interface TeamRoundStatsDto {
  round: number;
  team: TeamId;
  side: Side;
  won: boolean;
  /** This team's score after the round. */
  score: number;
  equipValue: number;
  balance: number;
  buyType: BuyType;
  kills: number;
  survivors: number;
}

export type KillKind = 'kill' | 'team_kill' | 'suicide' | 'world';

export interface KillEventDto {
  tick: number;
  /** Seconds since freeze end. */
  timeSecs: number;
  attacker: SteamId | null;
  victim: SteamId | null;
  assister: SteamId | null;
  weapon: string;
  headshot: boolean;
  assistedFlash: boolean;
  attackerSide: Side | null;
  victimSide: Side | null;
  kind: KillKind;
  opening: boolean;
  /** This kill avenged a teammate within 5 s. */
  trade: boolean;
  /** The victim was avenged within 5 s. */
  traded: boolean;
  afterRoundEnd: boolean;
}

export interface RoundDetailDto {
  number: number;
  winnerSide: Side | null;
  winnerTeam: TeamId | null;
  /** Engine reason, e.g. `t_killed`, `ct_killed`, `bomb_exploded`, `bomb_defused`, `target_saved`. */
  reason: string | null;
  /** Side team A played this round. */
  sideA: Side;
  scoreA: number;
  scoreB: number;
  durationSecs: number;
  startTick: number;
  freezeEndTick: number | null;
  endTick: number;
  bombPlanted: boolean;
  bombSite: number | null;
  mvp: SteamId | null;
  kills: KillEventDto[];
}

export interface TeamSummaryDto {
  team: TeamId;
  startSide: Side;
  score: number;
  /** null on a draw. */
  won: boolean | null;
  players: SteamId[];
}

/** attacker → victim, enemies only. */
export interface DamageCellDto {
  attacker: SteamId;
  victim: SteamId;
  damage: number;
  hits: number;
}

export interface DuelCellDto {
  attacker: SteamId;
  victim: SteamId;
  kills: number;
  headshotKills: number;
  openingKills: number;
}

/** `get_match_detail(matchId)`; null if the match is unknown or not analyzed yet. */
export interface MatchDetailDto {
  matchId: string;
  platform: string;
  mapName: string | null;
  playedAt: string | null;
  tickRate: number;
  roundsPlayed: number;
  statsVersion: number;
  /** false on demos without player_blind events: show blind metrics as "—". */
  blindDataAvailable: boolean;
  /** [team A, team B]. */
  teams: TeamSummaryDto[];
  scoreboard: PlayerMatchStatsDto[];
  rounds: RoundDetailDto[];
  playerRounds: PlayerRoundStatsDto[];
  /** Two rows per round (A and B). */
  economy: TeamRoundStatsDto[];
  damageMatrix: DamageCellDto[];
  duelMatrix: DuelCellDto[];
}

/** `list_player_stats(steamid, filters)` filters; all optional. */
export interface PlayerStatsFiltersDto {
  mapName?: string | null;
  platform?: string | null;
  /** Inclusive lower bound on played_at (ISO string). */
  since?: string | null;
  /** Exclusive upper bound on played_at. */
  until?: string | null;
  /** Page size for `matches` (default 50); the summary covers every filtered match. */
  limit?: number | null;
  offset?: number | null;
}

export interface PlayerMatchRowDto {
  matchId: string;
  platform: string;
  mapName: string | null;
  playedAt: string | null;
  scoreOwn: number;
  scoreOpponent: number;
  result: 'win' | 'loss' | 'draw';
  stats: PlayerMatchStatsDto;
}

export interface PlayerStatsListDto {
  steamid: SteamId;
  /** Matches matching the filters. */
  total: number;
  summary: PlayerMatchStatsDto | null;
  /** Newest first. */
  matches: PlayerMatchRowDto[];
}

// ---- Swing: round win probability (W2-B) ----

export type WinProbEventKind =
  | 'start'
  | 'damage'
  | 'kill'
  | 'team_kill'
  | 'suicide'
  | 'world'
  | 'planted'
  | 'defused'
  | 'exploded'
  | 'end';

/**
 * One event of a round's win-probability curve. Between two points the probability drifts with time
 * (from the previous `pAfter` to this `pBefore`, roughly linearly), then jumps to `pAfter`.
 */
export interface WinProbPointDto {
  tick: number;
  /** Seconds since freeze end. */
  timeSecs: number;
  kind: WinProbEventKind;
  /** Attacker / planter / defuser; null for start, end and world deaths. */
  player: SteamId | null;
  /** Victim of a kill or damage event. */
  victim: SteamId | null;
  /** Alive counts after the event. */
  tAlive: number;
  ctAlive: number;
  bombPlanted: boolean;
  /** T win probability, 0–1. The CT probability is 1 − p. */
  pBefore: number;
  pAfter: number;
}

export interface RoundWinProbabilityDto {
  round: number;
  winnerSide: Side | null;
  /** T win probability at freeze end. */
  pStart: number;
  /** First point is `start`, last is `end` (pAfter = 1 if T won, 0 if CT won). */
  points: WinProbPointDto[];
}

export interface WinModelInfoDto {
  /** Rounds in the local library behind the model's bucket samples. */
  localRounds: number;
  /** false while the library is too small (built-in prior only). */
  usesLocalData: boolean;
}

export interface PlayerRoundSwingDto {
  round: number;
  steamid: SteamId;
  team: TeamId;
  side: Side;
  /** Percentage points of round win probability. */
  swing: number;
}

/** `get_round_win_probability(matchId)`; null if the match is unknown or not analyzed yet. */
export interface MatchWinProbabilityDto {
  matchId: string;
  statsVersion: number;
  tickRate: number;
  model: WinModelInfoDto;
  rounds: RoundWinProbabilityDto[];
  playerRounds: PlayerRoundSwingDto[];
}
