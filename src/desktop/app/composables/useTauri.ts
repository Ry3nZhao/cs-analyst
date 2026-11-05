import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AnalysisQueueStatusDto, AnalysisUpdatedDto, AppSettingsDto, BackgroundTaskDto, ImportSummaryDto, MatchDto, MetadataRefreshSummaryDto, SubAccountDto, UserDto, ValveBotConfigDto, ValveBotConfigUpdateDto } from '@cs-analyst/shared-types';

export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

export function listenJobsUpdated(cb: () => void) {
  let unlisten: (() => void) | null = null;
  listen('jobs_updated', cb).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export function listenJobProgress(cb: (payload: { jobId: string; progress: number }) => void) {
  let unlisten: (() => void) | null = null;
  listen<{ jobId: string; progress: number }>('job_progress', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export function listenBackgroundTask(cb: (payload: BackgroundTaskDto | null) => void) {
  let unlisten: (() => void) | null = null;
  listen<BackgroundTaskDto | null>('background_task_updated', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export function listenBackgroundTaskLog(cb: (message: string) => void) {
  let unlisten: (() => void) | null = null;
  listen<string>('background_task_log', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export function listenImportCompleted(cb: (payload: ImportSummaryDto) => void) {
  let unlisten: (() => void) | null = null;
  listen<ImportSummaryDto>('import_completed', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export function listenMetadataRefreshCompleted(cb: (payload: MetadataRefreshSummaryDto) => void) {
  let unlisten: (() => void) | null = null;
  listen<MetadataRefreshSummaryDto>('metadata_refresh_completed', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export function listenAnalysisUpdated(cb: (payload: AnalysisUpdatedDto) => void) {
  let unlisten: (() => void) | null = null;
  listen<AnalysisUpdatedDto>('analysis_updated', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export function listenAnalysisQueue(cb: (payload: AnalysisQueueStatusDto) => void) {
  let unlisten: (() => void) | null = null;
  listen<AnalysisQueueStatusDto>('analysis_queue', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

export type { AnalysisQueueStatusDto, AnalysisUpdatedDto, AppSettingsDto, BackgroundTaskDto, ImportSummaryDto, MatchDto, MetadataRefreshSummaryDto, SubAccountDto, UserDto, ValveBotConfigDto, ValveBotConfigUpdateDto };
