/** W3-D 存储与数据。Rust 侧见 src/tauri/src-tauri/src/storage/。 */

/** 解析成功后对原始 demo 的处理 */
export type DemoPolicy = 'keep' | 'compress' | 'delete';
/** 原始 demo 当前状态 */
export type DemoStorageState = 'raw' | 'compressed' | 'deleted';

/** `get_storage_settings` / `update_storage_settings` */
export interface StorageSettingsDto {
  demoPolicy: DemoPolicy;
  /** 启动时按规则自动清理原始 demo */
  autoCleanup: boolean;
  /** 早于 N 天的对局；null = 不启用 */
  cleanupMaxAgeDays: number | null;
  /** demo 总占用上限（GB）；null = 不启用 */
  cleanupMaxTotalGb: number | null;
  lastCleanupAt?: string | null;
  lastCleanupSummary?: string | null;
}

/** `list_match_storage`：对局列表按 id 合并 */
export interface MatchStorageInfoDto {
  id: string;
  favorite: boolean;
  demoStorage: DemoStorageState;
  fileSize: number | null;
  demoOriginalSize: number | null;
}

export interface UsageGroupDto {
  key: string;
  label: string;
  matches: number;
  demoBytes: number;
  replayBytes: number;
}

/** `get_storage_usage` */
export interface StorageUsageDto {
  dataRoot: string;
  demoBytes: number;
  replayBytes: number;
  databaseBytes: number;
  backupBytes: number;
  exportBytes: number;
  /** 不在应用数据目录里的 demo（不参与压缩 / 清理） */
  externalDemoBytes: number;
  byPlatform: UsageGroupDto[];
  byAccount: UsageGroupDto[];
  rawCount: number;
  compressedCount: number;
  deletedCount: number;
  favoriteCount: number;
  savedBytes: number;
}

export interface CleanupRulesDto {
  maxAgeDays: number | null;
  maxTotalGb: number | null;
}

export type StoragePlanKind = 'cleanup' | 'compress' | 'delete';

export interface PlanItemDto {
  id: string;
  mapName: string | null;
  playedAt: string | null;
  platform: string | null;
  demoStorage: DemoStorageState;
  bytes: number;
  favorite: boolean;
  /** 会被处理；false 时 reason 说明跳过原因 */
  eligible: boolean;
  reason: string | null;
}

/** `preview_storage_plan` */
export interface StoragePlanDto {
  /** 传给 `apply_storage_plan`，只能用一次；新的预览会让旧令牌失效 */
  token: string;
  action: StoragePlanKind;
  items: PlanItemDto[];
  eligibleCount: number;
  eligibleBytes: number;
  skippedFavorites: number;
  totalDemoBytes: number;
}

/** `apply_storage_plan` */
export interface StoragePlanResultDto {
  action: StoragePlanKind;
  processed: number;
  deleted: number;
  bytesFreed: number;
  failed: { id: string; reason: string }[];
}

/** `storage_progress` 事件 */
export interface StorageProgressDto {
  op: string;
  done: number;
  total: number;
}

export type ExportFormat = 'csv' | 'json';

/** `export_matches` */
export interface ExportResultDto {
  dir: string;
  format: ExportFormat;
  matches: number;
  files: { table: string; path: string; rows: number }[];
}

export interface BackupManifestDto {
  createdAt: string;
  appVersion: string;
  schemaVersion: number;
  parserVersion: number;
  matches: number;
  includesReplays: boolean;
  replayFiles: number;
  /** manual / pre-restore */
  kind: string;
}

/** `create_backup` / `list_backups` */
export interface BackupInfoDto {
  name: string;
  path: string;
  bytes: number;
  manifest: BackupManifestDto | null;
}

/** `restore_backup`（需传 `confirm: 'RESTORE'`）；完成后发出 `storage_restored` 事件 */
export interface RestoreResultDto {
  preRestoreBackup: BackupInfoDto;
  schemaVersion: number;
  matches: number;
  replayFilesRestored: number;
}
