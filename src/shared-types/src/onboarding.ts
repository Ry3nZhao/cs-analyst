// W3-C 上手体验：主玩家、向导、托盘 / 通知 / 监听目录。
// - steamid 一律是十进制字符串（超出 JS 安全整数范围）。

/** 推测出的主玩家候选人（只是建议，不会自动写入） */
export interface MainPlayerSuggestionDto {
  steamid: string;
  displayName: string | null;
  personId: number | null;
  /** valve = 已绑定的 Valve 子账号；account = 其他平台账号关联到的 steamid；followed = 我关注的人；frequent = 出场最多 */
  source: 'valve' | 'account' | 'followed' | 'frequent';
  matchCount: number;
}

/** `get_main_player` / `set_main_player` / 事件 `main_player_changed` */
export interface MainPlayerDto {
  /** null = 未设置 */
  steamid: string | null;
  displayName: string | null;
  personId: number | null;
  /** 已解析对局里的出场次数 */
  matchCount: number;
  /** 只在未设置时给出 */
  suggestion: MainPlayerSuggestionDto | null;
}

/** `list_main_player_candidates`：已解析对局里的选手 */
export interface PlayerCandidateDto {
  steamid: string;
  name: string;
  personId: number | null;
  matchCount: number;
  lastSeenAt: string | null;
  followed: boolean;
}

/** `get_onboarding_state` */
export interface OnboardingStateDto {
  /** 应该自动进入向导：没走过向导，且没有主玩家、账号、对局 */
  needsOnboarding: boolean;
  completed: boolean;
  mainPlayerSet: boolean;
  accountCount: number;
  matchCount: number;
  analyzedCount: number;
  dataRoot: string;
  defaultDataRoot: string;
  /** 由环境变量 CS_ANALYST_DATA_ROOT 指定，不能在应用里改 */
  dataRootLocked: boolean;
}

/** `get_experience_settings` / `update_experience_settings` */
export interface ExperienceSettingsDto {
  closeToTray: boolean;
  notifyNewMatch: boolean;
  watchReplays: boolean;
  /** 手动指定的 replays 目录；null = 自动检测 */
  replaysDir: string | null;
}

/** `get_watch_status` / 事件 `watch_status` */
export interface WatchStatusDto {
  enabled: boolean;
  dir: string | null;
  source: 'manual' | 'auto' | 'none';
  detectedDir: string | null;
  dirExists: boolean;
  watching: boolean;
  importedCount: number;
  lastImported: string | null;
  lastError: string | null;
}

/** 事件 `replay_imported` */
export interface ReplayImportedDto {
  path: string;
  action: 'imported' | 'skipped' | 'error';
  error: string | null;
}
