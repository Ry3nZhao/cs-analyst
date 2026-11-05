// W3-A 高光 / 失误集锦。命令见 src/tauri/src-tauri/src/highlights/commands.rs，口径见 docs/highlights.md。
// steamid 一律用十进制字符串。

export type HighlightCategory = 'highlight' | 'mistake';

export type HighlightKind =
  // 高光
  | 'multi_kill'
  | 'quick_kills'
  | 'clutch_win'
  | 'flick_headshot'
  | 'wallbang'
  | 'smoke_kill'
  | 'blind_kill'
  | 'airborne_kill'
  | 'noscope_kill'
  | 'swing_kill'
  | 'key_defuse'
  | 'ninja_defuse'
  // 失误
  | 'untraded_opening_death'
  | 'unused_utility_death'
  | 'team_flash'
  | 'team_damage'
  | 'lost_hp_duel'
  | 'whiffed_spray'
  | 'swing_loss';

/** `list_highlights({ filters })` */
export interface HighlightFilters {
  /** 不传 = 所有选手 */
  steamid?: string | null;
  matchId?: string | null;
  category?: HighlightCategory | null;
  kinds?: HighlightKind[] | null;
  maps?: string[] | null;
  /** 最近 N 天（按对局时间） */
  days?: number | null;
  /** ISO 日期（含） */
  from?: string | null;
  to?: string | null;
  /** 默认 500，最多 5000 */
  limit?: number | null;
  /** 包含设置里关掉的规则 */
  includeDisabled?: boolean;
}

/** 规则相关的细节。不同类型字段不同，steamid 是字符串；名字在 `HighlightList.names` 里查。 */
export interface HighlightDetail {
  count?: number;
  spanMs?: number;
  kills?: { tick: number; victim: string | null; weapon: string; headshot: boolean }[] | number;
  killList?: { tick: number; victim: string | null; weapon: string; headshot: boolean }[];
  vs?: number;
  side?: number;
  angle?: number;
  ttdMs?: number;
  weapon?: string | null;
  weapons?: string[];
  victim?: string | null;
  victims?: { steamid: string; duration?: number; damage?: number }[];
  killer?: string | null;
  headshot?: boolean;
  distance?: number | null;
  flags?: HighlightKind[];
  deltaPp?: number;
  before?: number;
  after?: number;
  event?: 'death' | 'team_kill' | 'suicide';
  tAlive?: number;
  ctAlive?: number;
  remainingSecs?: number;
  killsAfterPlant?: number;
  roundLost?: boolean;
  value?: number;
  teamSecs?: number;
  enemySecs?: number;
  damage?: number;
  teamKills?: string[];
  hp?: number;
  enemyHp?: number;
  dealt?: number;
  shots?: number;
  [key: string]: unknown;
}

export interface HighlightItem {
  id: number;
  matchId: string;
  mapName: string | null;
  playedAt: string | null;
  platform: string;
  round: number;
  startTick: number;
  endTick: number;
  steamid: string;
  playerName: string | null;
  /** 2 = T，3 = CT */
  side: number | null;
  kind: HighlightKind;
  category: HighlightCategory;
  /** 规则原始分 0–100 */
  score: number;
  /** 原始分 × 设置里的权重，用于排序 */
  weightedScore: number;
  detail: HighlightDetail;
  demoStorage: 'raw' | 'compressed' | 'deleted';
}

export interface HighlightKindCount {
  kind: HighlightKind;
  category: HighlightCategory;
  count: number;
}

export interface HighlightList {
  items: HighlightItem[];
  /** 符合筛选条件的总数（items 受 limit 限制） */
  total: number;
  /** 按类型计数，只受选手 / 对局 / 地图 / 时间约束 */
  counts: HighlightKindCount[];
  matches: number;
  maps: string[];
  /** 已解析但集锦还在后台计算的对局数 */
  pending: number;
  /** steamid → 名字 */
  names: Record<string, string>;
}

export interface HighlightRuleSetting {
  kind: HighlightKind;
  category: HighlightCategory;
  enabled: boolean;
  /** 0–3 */
  weight: number;
  defaultWeight: number;
}

/** `get_highlight_settings` / `update_highlight_settings({ settings })` */
export interface HighlightSettings {
  rules: HighlightRuleSetting[];
  /** 跳转到事件前几秒（0–30） */
  leadSecs: number;
  /** 实验：-netconport 自动跳转（默认关闭） */
  netconEnabled: boolean;
  netconPort: number;
  /** 附加的 CS2 启动参数，例如 -tools */
  extraLaunchArgs: string;
}

/** `prepare_highlight_playback({ id })` */
export interface HighlightPlayback {
  highlightId: number;
  matchId: string;
  status: 'ready' | 'deleted' | 'missing';
  message: string | null;
  demoPath: string | null;
  /** demo 是 .dem.zst，已解压到临时目录 */
  decompressed: boolean;
  startTick: number;
  gotoTick: number;
  leadSecs: number;
  playdemoCommand: string | null;
  gotoCommand: string;
  combinedCommand: string | null;
}

/** `launch_highlight_in_cs2({ id })` */
export interface HighlightLaunchResult {
  playback: HighlightPlayback;
  method: 'steam_exe' | 'steam_url' | 'netcon' | 'unavailable';
  autoGoto: boolean;
  message: string;
}

/** `highlight_netcon` 事件（实验开关打开时） */
export interface HighlightNetconEvent {
  highlightId: number;
  stage: 'connecting' | 'connected' | 'loading' | 'sent' | 'failed';
  message: string;
}
