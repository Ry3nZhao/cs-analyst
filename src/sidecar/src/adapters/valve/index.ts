import type { DiscoverParams, DownloadParams } from '@cs-analyst/downloader-protocol';
import type { AdapterModule } from '../registry.js';
import { discoverValveAuthCode } from '../valve-authcode/discover.js';
import { downloadValveDemo } from '../valve-authcode/download.js';
import { discoverValveLocal, downloadValveLocal } from '../valve-local/index.js';

/** Valve：Auth Code 通道（分享码 + Steam GC）和 Local 通道（占位）。 */
export const valveAdapter: AdapterModule = {
  async discover(params: DiscoverParams) {
    const channel = params.valveChannel ?? (params.metadata.source === 'auth_code' ? 'authcode' : 'local');
    if (channel !== 'authcode') return discoverValveLocal(params.externalId);
    return discoverValveAuthCode({
      steamId: params.externalId,
      authCode: String(params.metadata.auth_code ?? ''),
      oldestShareCode: String(params.metadata.oldest_share_code ?? ''),
      steamApiKey: String(params.metadata.steam_api_key ?? ''),
      lastShareCode: params.metadata.last_share_code as string | undefined,
    });
  },
  async download(params: DownloadParams, { emit }) {
    const options = { jobId: params.jobId, emit };
    if ((params.valveChannel ?? 'authcode') === 'local') {
      await downloadValveLocal(params.demoUrl, params.outputPath, options);
    } else {
      await downloadValveDemo({ outputPath: params.outputPath, demoUrl: params.demoUrl, ...options });
    }
  },
};
