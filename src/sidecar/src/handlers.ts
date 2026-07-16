#!/usr/bin/env node
import readline from 'node:readline';
import type {
  DiscoverParams,
  DownloadParams,
  ResolveValveAuthcodeParams,
  SidecarEvent,
} from '@cs-analyst/shared-types';
import { discover5EPlay, download5EPlay, resolve5EPlayPlayerId } from './adapters/5eplay/index.js';
import { discoverRenown, downloadRenown } from './adapters/renown/index.js';
import { discoverValveAuthCode } from './adapters/valve-authcode/discover.js';
import { downloadValveDemo } from './adapters/valve-authcode/download.js';
import { resolveValveAuthcodeUrls } from './adapters/valve-downloader/steam-gc.js';
import { discoverValveLocal, downloadValveLocal } from './adapters/valve-local/index.js';
import { analyzeDem } from './dem-parser.js';
import type { JsonRpcServer } from './rpc.js';

function sidecarLog(message: string, data?: unknown) {
  const extra = data === undefined ? '' : ` ${JSON.stringify(data)}`;
  console.error(`[sidecar] ${message}${extra}`);
}

export function registerHandlers(server: JsonRpcServer) {
  const emit = (event: SidecarEvent) => server.emit(event);

  server.register('discover', async (raw: unknown) => {
    const params = raw as DiscoverParams;
    sidecarLog('discover', {
      platform: params.platform,
      externalId: params.externalId,
      valveChannel: params.valveChannel,
    });
    try {
      switch (params.platform) {
        case '5eplay':
          return await discover5EPlay(params.externalId);
        case 'renown':
          return await discoverRenown(params.externalId);
        case 'valve': {
          const channel = params.valveChannel ?? (params.metadata.source === 'auth_code' ? 'authcode' : 'local');
          if (channel === 'authcode') {
            const matches = await discoverValveAuthCode({
              steamId: params.externalId,
              authCode: String(params.metadata.auth_code ?? ''),
              oldestShareCode: String(params.metadata.oldest_share_code ?? ''),
              steamApiKey: String(params.metadata.steam_api_key ?? ''),
              lastShareCode: params.metadata.last_share_code as string | undefined,
            });
            sidecarLog('discover valve authcode ok', { count: matches.length });
            return matches;
          }
          return discoverValveLocal(params.externalId);
        }
        default:
          throw new Error(`Unknown platform: ${params.platform}`);
      }
    } catch (error) {
      sidecarLog('discover FAILED', {
        platform: params.platform,
        message: error instanceof Error ? error.message : String(error),
      });
      throw error;
    }
  });

  server.register('resolve_valve_authcode_urls', async (raw: unknown) => {
    const params = raw as ResolveValveAuthcodeParams;
    sidecarLog('resolve_valve_authcode_urls', { count: params.items.length });
    if (!params.bot.username) {
      throw new Error('Valve bot username is required');
    }
    return resolveValveAuthcodeUrls(params.bot, params.items, emit);
  });

  server.register('download', async (raw: unknown) => {
    const params = raw as DownloadParams;
    sidecarLog('download', {
      platform: params.platform,
      jobId: params.jobId,
      matchId: params.matchId,
      valveChannel: params.valveChannel,
      hasDemoUrl: Boolean(params.demoUrl),
    });
    try {
      switch (params.platform) {
        case '5eplay':
          await download5EPlay(params.demoUrl, params.outputPath, {
            jobId: params.jobId,
            emit,
          });
          break;
        case 'renown':
          await downloadRenown(params.demoUrl, params.outputPath, {
            jobId: params.jobId,
            emit,
          });
          break;
        case 'valve': {
          const channel = params.valveChannel ?? 'authcode';
          if (channel === 'local') {
            await downloadValveLocal(params.demoUrl, params.outputPath, {
              jobId: params.jobId,
              emit,
            });
          } else {
            await downloadValveDemo({
              outputPath: params.outputPath,
              demoUrl: params.demoUrl,
              jobId: params.jobId,
              emit,
            });
          }
          break;
        }
        default:
          throw new Error(`Unknown platform: ${params.platform}`);
      }
      sidecarLog('download ok', { jobId: params.jobId });
      return { ok: true };
    } catch (error) {
      sidecarLog('download FAILED', {
        jobId: params.jobId,
        message: error instanceof Error ? error.message : String(error),
      });
      throw error;
    }
  });

  server.register('analyze_dem', async (raw: unknown) => {
    const params = raw as { filePath: string };
    sidecarLog('analyze_dem', { filePath: params.filePath });
    return analyzeDem(params.filePath);
  });

  server.register('resolve_5eplay_player_id', async (raw: unknown) => {
    const params = raw as { input: string };
    const uuid = await resolve5EPlayPlayerId(params.input);
    return { uuid };
  });

  server.register('ping', async () => ({ pong: true }));
}
