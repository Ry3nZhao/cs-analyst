/**
 * 个人主页（W3-B）：`get_profile({ steamid, options? })` → `ProfileDto`。
 * 后端：`src/tauri/src-tauri/src/profile/`，口径见 `docs/profile.md`。
 * steamid 一律是十进制字符串。所有 `number | null` 里的 null 表示「不可用」，不是 0。
 */

export interface ProfileOptionsDto {
  /** 趋势滑动窗口（场），默认 10 */
  window?: number;
  /** 只看最近 N 场；不传 = 全部 */
  lastMatches?: number;
}

export type ProfileMetricKey =
  | 'adr'
  | 'kast'
  | 'kd'
  | 'rating'
  | 'swing'
  | 'openingDuel'
  | 'clutch'
  | 'ttd'
  | 'crosshairPlacement'
  | 'counterStrafe'
  | 'sprayError';

export type ScoreSystem = 'premier' | 'competitive' | 'wingman';

/** tier = 同段位样本；all = 同段位样本不足，用全库；none = 全库也不够，不给百分位 */
export type SampleScope = 'tier' | 'all' | 'none';

export interface ProfileSummaryDto {
  matches: number;
  wins: number;
  losses: number;
  draws: number;
  /** 0–100 */
  winRate: number | null;
  rounds: number;
  kills: number;
  deaths: number;
  firstMatchAt: string | null;
  lastMatchAt: string | null;
}

export interface ProfileTierDto {
  system: ScoreSystem;
  systemLabel: string;
  score: number;
  bandLo: number;
  bandHi: number | null;
  bandLabel: string;
  /** 取分数的那一场 */
  matchId: string;
  at: string;
}

export interface ScorePointDto {
  matchId: string;
  at: string;
  value: number;
}

export interface ScoreSeriesDto {
  system: ScoreSystem;
  label: string;
  /** 旧 → 新 */
  points: ScorePointDto[];
  latest: number;
  best: number;
}

export interface ProfileSampleInfoDto {
  /** 同段位里（≥ 10 回合）的其他选手数；没有段位时为 0 */
  tierPlayers: number;
  /** 全库里（≥ 10 回合）的其他选手数 */
  libraryPlayers: number;
  minTierSample: number;
  minSample: number;
  confidentSample: number;
}

export interface PercentileDto {
  /** 0–100，越高越好（已按指标方向处理）；null = 样本不足或本人无数据 */
  value: number | null;
  scope: SampleScope;
  sampleSize: number;
  sampleMedian: number | null;
  lowConfidence: boolean;
}

export interface ProfileMetricDto {
  key: ProfileMetricKey;
  name: string;
  unit: string;
  higherIsBetter: boolean;
  category: 'basic' | 'aim';
  /** 范围内全部场次；null = 不可用 */
  value: number | null;
  /** 最近 window 场 */
  recent: number | null;
  matchesWithData: number;
  support: number;
  percentile: PercentileDto;
}

export interface ProfileTrendMatchDto {
  matchId: string;
  at: string;
  mapName: string | null;
  result: 'win' | 'loss' | 'draw';
}

export interface ProfileTrendSeriesDto {
  key: ProfileMetricKey;
  /** 每场的值（null = 该场不可用） */
  values: (number | null)[];
  /** 最近 window 场（有数据的）合并计算 */
  rolling: (number | null)[];
}

export interface ProfileTrendDto {
  window: number;
  /** 旧 → 新 */
  matches: ProfileTrendMatchDto[];
  series: ProfileTrendSeriesDto[];
}

export interface ProfileMapDto {
  mapName: string;
  matches: number;
  wins: number;
  losses: number;
  draws: number;
  winRate: number | null;
  rounds: number;
  adr: number | null;
  kast: number | null;
  kd: number | null;
  rating: number | null;
  swing: number | null;
}

export interface ProfileSideDto {
  /** 2 = T，3 = CT */
  side: 2 | 3;
  rounds: number;
  roundsWon: number;
  roundWinRate: number | null;
  kills: number;
  deaths: number;
  kd: number | null;
  adr: number | null;
  kast: number | null;
  openingKills: number;
  openingDeaths: number;
  openingRate: number | null;
  swing: number | null;
}

export interface ProfileWeaponDto {
  weapon: string;
  kills: number;
  headshotKills: number;
  hsPct: number | null;
  shots: number;
  hits: number;
  accuracy: number | null;
  headHitRate: number | null;
}

export interface PersonalBestDto {
  key: 'kills' | 'adr' | 'rating' | 'swing' | 'headshotKills' | 'multiKill' | 'clutch' | 'ttd';
  label: string;
  /** MetricHelp 的 key */
  metric: string;
  value: number;
  matchId: string;
  mapName: string | null;
  at: string;
  result: 'win' | 'loss' | 'draw';
  scoreOwn: number;
  scoreOpp: number;
}

export interface ProfileFocusDto {
  key: ProfileMetricKey;
  name: string;
  value: number;
  percentile: number;
  sampleMedian: number | null;
  scope: SampleScope;
  sampleSize: number;
}

export interface ProfileDto {
  steamid: string;
  /** 同一个人名下的全部 steam 身份 */
  steamids: string[];
  personId: number | null;
  displayName: string | null;
  window: number;
  lastMatches: number | null;
  summary: ProfileSummaryDto;
  tier: ProfileTierDto | null;
  scores: ScoreSeriesDto[];
  sample: ProfileSampleInfoDto;
  metrics: ProfileMetricDto[];
  trend: ProfileTrendDto;
  maps: ProfileMapDto[];
  sides: ProfileSideDto[];
  weapons: ProfileWeaponDto[];
  bests: PersonalBestDto[];
  /** 「重点改进」：百分位最低且低于 50 的至多 3 项 */
  focus: ProfileFocusDto[];
}
