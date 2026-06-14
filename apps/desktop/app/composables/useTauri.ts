import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppSettingsDto, DownloadJobDto, SubAccountDto, UserDto } from '@cs-demo-analyst/shared-types';

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

export type { AppSettingsDto, DownloadJobDto, SubAccountDto, UserDto };
