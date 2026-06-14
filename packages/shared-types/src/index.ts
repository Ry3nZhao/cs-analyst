export type Platform = 'valve' | 'renown' | '5eplay';
export type ValveChannel = 'local' | 'authcode';
export type AccountStatus = 'active' | 'disabled' | 'error';
export type JobStatus =
  | 'discovered'
  | 'queued'
  | 'downloading'
  | 'completed'
  | 'failed'
  | 'skipped'
  | 'expired';

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

export interface DownloadJobDto {
  jobId: string;
  status: JobStatus;
  progress: number;
  platform: Platform;
  matchId: string;
  mapName?: string | null;
  playedAt?: string | null;
  valveChannel?: ValveChannel | null;
  userId: string;
  userName: string;
  accountName: string;
  accountId: string;
  attempts: number;
  lastError?: string | null;
  demoUrl?: string | null;
  shareCode?: string | null;
}

export interface AppSettingsDto {
  dataRoot: string;
  globalMaxConcurrent: number;
  autoDiscoverOnStartup: boolean;
}
