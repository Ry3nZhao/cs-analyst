export type Platform = 'valve' | 'renown' | '5eplay' | 'imported';
export type ValveChannel = 'local' | 'authcode';
export type AccountStatus = 'active' | 'disabled' | 'error';
export type MatchStatus = 'queued' | 'downloading' | 'completed' | 'failed' | 'skipped' | 'expired';

/** @deprecated use MatchStatus */
export type JobStatus = MatchStatus;

// 下载模块协议类型（DiscoverParams、JsonRpc*、进度事件等）已移到 `@cs-analyst/downloader-protocol`
// （src/sidecar/protocol），前端不依赖它们；设置页用到的模块状态见 ./downloader.ts。

export interface UserDto {
  id: string;
  displayName: string;
  note?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SubAccountDto {
  id: string;
  userId: string;
  platform: Platform;
  externalId: string;
  displayName: string;
  avatarUrl?: string | null;
  status: AccountStatus;
  metadata: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

/** A single match record — combines dem metadata, download status, and file info. */
export interface MatchDto {
  id: string;
  status: MatchStatus;
  progress: number;
  platform: Platform;
  matchId: string;
  /** 来源：与 platform 相同，仅平台名 */
  source: string;
  // Dem metadata
  mapName?: string | null;
  playedAt?: string | null;
  durationSeconds?: number | null;
  gameMode?: string | null;
  gameType?: number | null;
  shareCode?: string | null;
  checksum?: string | null;
  filePath?: string | null;
  fileName?: string | null;
  fileSize?: number | null;
  valveChannel?: ValveChannel | null;
  // CDemoFileHeader 1-12
  demoFileStamp?: string | null;
  networkProtocol?: number | null;
  serverName?: string | null;
  clientName?: string | null;
  gameDirectory?: string | null;
  fullpacketsVersion?: number | null;
  allowClientsideEntities?: boolean | null;
  demoHeaderField9?: number | null;
  demoHeaderField10?: string | null;
  demoVersionName?: string | null;
  demoVersionGuid?: string | null;
  // CDemoFileInfo 1-4
  playbackTime?: number | null;
  playbackTicks?: number | null;
  playbackFrames?: number | null;
  demGameInfo?: Record<string, unknown> | null;
  // Download details
  demoUrl?: string | null;
  attempts: number;
  lastError?: string | null;
  // Full demo analysis
  analysisStatus?: AnalysisStatus | null;
  analysisError?: string | null;
  analyzedAt?: string | null;
  analysisMs?: number | null;
  roundsPlayed?: number | null;
}

/** 完整解析状态（null = 尚未排队，例如还没下载完） */
export type AnalysisStatus = 'pending' | 'running' | 'done' | 'failed';

/** `analysis_updated` 事件 */
export interface AnalysisUpdatedDto {
  matchId: string;
  status: AnalysisStatus;
  error?: string | null;
  /** 队列中剩余数量 */
  queued: number;
}

/** `analysis_queue` 事件 / `get_analysis_status` */
export interface AnalysisQueueStatusDto {
  queued: number;
  running?: string | null;
}

/** @deprecated use MatchDto */
export type DownloadJobDto = MatchDto;

export interface MetadataRefreshResultDto {
  id: string;
  filePath: string;
  action: string;
  error?: string | null;
}

export interface MetadataRefreshSummaryDto {
  updated: number;
  merged: number;
  skipped: number;
  failed: number;
  results: MetadataRefreshResultDto[];
}

export interface ImportResultDto {
  path: string;
  action: string;
  error?: string | null;
}

export interface ImportSummaryDto {
  imported: number;
  skipped: number;
  results: ImportResultDto[];
}

export interface AppSettingsDto {
  dataRoot: string;
  globalMaxConcurrent: number;
  autoDiscoverOnStartup: boolean;
}

export interface BackgroundTaskDto {
  id: string;
  kind: string;
  label: string;
  status: string;
  progress: number;
  current?: string | null;
}

export interface ValveBotConfigDto {
  username: string | null;
  passwordSet: boolean;
  secretSet: boolean;
  steamApiKey: string | null;
  cronSchedule: string;
  runOnStartup: boolean;
}

export interface ValveBotConfigUpdateDto {
  username?: string | null;
  password?: string | null;
  secret?: string | null;
  steamApiKey?: string | null;
  cronSchedule?: string;
  runOnStartup?: boolean;
}
export * from './stats.js';
export * from './identity.js';
export * from './aim.js';
export * from './downloader.js';
export * from './replay.js';
export * from './training.js';
