/** 下载模块状态（`get_downloader_status` / `downloader_status` 事件），设置页「下载模块」区块使用。 */

export type DownloaderState = 'stopped' | 'starting' | 'running' | 'restarting' | 'failed' | 'incompatible';

export interface DownloaderAdapterDto {
  platform: string;
  label: string;
  /** 是否启用（来自 platform_settings.enabled） */
  enabled: boolean;
  /** adapter 是否加载成功 */
  available: boolean;
  error?: string | null;
  capabilities: string[];
}

export interface DownloaderStatusDto {
  state: DownloaderState;
  pid: number | null;
  moduleName: string | null;
  moduleVersion: string | null;
  runtime: string | null;
  protocolVersion: number | null;
  hostProtocolVersion: number;
  restartCount: number;
  consecutiveFailures: number;
  startedAt: string | null;
  lastHeartbeatAt: string | null;
  lastHeartbeatMs: number | null;
  nextRestartInMs: number | null;
  lastExit: string | null;
  lastError: string | null;
  adapters: DownloaderAdapterDto[];
  methods: string[];
  scriptPath: string;
  logPath: string | null;
}
