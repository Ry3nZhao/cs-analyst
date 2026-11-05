export type Platform = 'valve' | 'renown' | '5eplay' | 'imported';
export type ValveChannel = 'local' | 'authcode';
export type AccountStatus = 'active' | 'disabled' | 'error';
export type MatchStatus = 'queued' | 'downloading' | 'completed' | 'failed' | 'skipped' | 'expired';

/** @deprecated use MatchStatus */
export type JobStatus = MatchStatus;

export interface DiscoveredMatchDto {
  matchId: string;
  shareCode?: string | null;
  demoUrl?: string | null;
  mapName?: string | null;
  playedAt?: string | null;
  valveChannel?: ValveChannel | null;
  rawPayload: Record<string, unknown>;
}

export interface DiscoverParams {
  platform: Platform;
  subAccountId: string;
  externalId: string;
  metadata: Record<string, unknown>;
  valveChannel?: ValveChannel | null;
}

export interface DownloadParams {
  jobId: string;
  platform: Platform;
  outputPath: string;
  demoUrl: string;
  matchId: string;
  valveChannel?: ValveChannel | null;
  metadata?: Record<string, unknown>;
}

export interface JsonRpcRequest {
  id: string | number;
  method: string;
  params?: unknown;
}

export interface JsonRpcResponse {
  id: string | number | null;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

export interface SidecarEvent {
  event: string;
  [key: string]: unknown;
}

export type ResolveProgressPhase = 'started' | 'resolved' | 'failed';

export interface ResolveProgressEvent extends SidecarEvent {
  event: 'resolve.progress';
  jobId: string;
  matchId: string;
  shareCode: string;
  phase: ResolveProgressPhase;
  demoUrl?: string;
  playedAt?: string;
  mapName?: string;
  error?: string;
}

export type ResolveBatchPhase = 'logging_in' | 'resolving' | 'done';

export interface ResolveBatchEvent extends SidecarEvent {
  event: 'resolve.batch';
  total: number;
  completed: number;
  phase: ResolveBatchPhase;
}

export type DownloadProgressPhase = 'downloading' | 'extracting' | 'done';

export interface DownloadProgressEvent extends SidecarEvent {
  event: 'download.progress';
  jobId: string;
  progress: number;
  bytesDownloaded?: number;
  totalBytes?: number;
  phase: DownloadProgressPhase;
}

export interface ValveBotLoginDto {
  username: string;
  password?: string;
  secret?: string;
  refreshToken?: string;
}

export interface ResolveValveAuthcodeItemDto {
  jobId: string;
  shareCode: string;
  matchId: string;
}

export interface ResolveValveAuthcodeParams {
  bot: ValveBotLoginDto;
  items: ResolveValveAuthcodeItemDto[];
}

export interface ResolvedValveMatchDto {
  jobId: string;
  shareCode: string;
  matchId: string;
  demoUrl?: string;
  playedAt?: string;
  mapName?: string;
  error?: string;
}

export interface ResolveValveAuthcodeResultDto {
  resolved: ResolvedValveMatchDto[];
  refreshToken?: string;
}

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
