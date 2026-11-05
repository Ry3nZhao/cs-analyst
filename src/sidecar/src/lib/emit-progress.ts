import type { DownloadProgressEvent, ResolveBatchEvent, ResolveProgressEvent, SidecarEvent } from '@cs-analyst/downloader-protocol';

export type EventEmitter = (event: SidecarEvent) => void;

export function emitResolveProgress(
  emit: EventEmitter | undefined,
  payload: Omit<ResolveProgressEvent, 'event'>,
): void {
  emit?.({ event: 'resolve.progress', ...payload });
}

export function emitResolveBatch(
  emit: EventEmitter | undefined,
  payload: Omit<ResolveBatchEvent, 'event'>,
): void {
  emit?.({ event: 'resolve.batch', ...payload });
}

export function emitDownloadProgress(
  emit: EventEmitter | undefined,
  payload: Omit<DownloadProgressEvent, 'event'>,
): void {
  emit?.({ event: 'download.progress', ...payload });
}
