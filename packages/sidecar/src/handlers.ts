import type { DiscoverParams, DownloadParams } from '@cs-demo-analyst/shared-types';
import { discover5EPlay, download5EPlay } from './adapters/5eplay/index.js';
import { discoverRenown, downloadRenown } from './adapters/renown/index.js';
import { discoverValveAuthCode } from './adapters/valve-authcode/discover.js';
import { downloadValveDemo } from './adapters/valve-authcode/download.js';
import { discoverValveLocal, downloadValveLocal } from './adapters/valve-local/index.js';
import type { JsonRpcServer } from './rpc.js';

export function registerHandlers(server: JsonRpcServer) {
  server.register('discover', async (raw: unknown) => {
    const params = raw as DiscoverParams;
    switch (params.platform) {
      case '5eplay':
        return discover5EPlay(params.externalId);
      case 'renown':
        return discoverRenown(params.externalId);
      case 'valve': {
        const channel = params.valveChannel ?? (params.metadata.source === 'auth_code' ? 'authcode' : 'local');
        if (channel === 'authcode') {
          return discoverValveAuthCode({
            steamId: params.externalId,
            authCode: String(params.metadata.auth_code ?? ''),
            oldestShareCode: String(params.metadata.oldest_share_code ?? ''),
            steamApiKey: String(params.metadata.steam_api_key ?? ''),
            lastShareCode: params.metadata.last_share_code as string | undefined,
          });
        }
        return discoverValveLocal(params.externalId);
      }
      default:
        throw new Error(`Unknown platform: ${params.platform}`);
    }
  });

  server.register('download', async (raw: unknown) => {
    const params = raw as DownloadParams & {
      botUsername?: string;
      botPassword?: string;
      botSecret?: string;
      shareCode?: string;
    };

    switch (params.platform) {
      case '5eplay':
        await download5EPlay(params.demoUrl, params.outputPath);
        break;
      case 'renown':
        await downloadRenown(params.demoUrl, params.outputPath);
        break;
      case 'valve': {
        const channel = params.valveChannel ?? 'authcode';
        if (channel === 'local') {
          await downloadValveLocal(params.demoUrl, params.outputPath);
        } else {
          await downloadValveDemo({
            outputPath: params.outputPath,
            demoUrl: params.demoUrl,
            shareCode: params.shareCode,
            botUsername: params.botUsername,
            botPassword: params.botPassword,
            botSecret: params.botSecret,
          });
        }
        break;
      }
      default:
        throw new Error(`Unknown platform: ${params.platform}`);
    }

    return { ok: true };
  });

  server.register('ping', async () => ({ pong: true }));
}
