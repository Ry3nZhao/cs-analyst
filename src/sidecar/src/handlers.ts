import {
  PROTOCOL_VERSION,
  RpcErrorCode,
  type ConfigureParams,
  type ConfigureResult,
  type DiscoverParams,
  type DownloadParams,
  type HelloParams,
  type HelloResult,
  type PingResult,
  type Resolve5EPlayPlayerIdParams,
  type ResolveValveAuthcodeParams,
  type SidecarEvent,
} from '@cs-analyst/downloader-protocol';
import { AdapterRegistry, isPlatform } from './adapters/registry.js';
import { JsonRpcServer, RpcError } from './rpc.js';

export const MODULE_NAME = 'cs-analyst-downloader';
export const MODULE_VERSION = '0.2.0';

export function sidecarLog(message: string, data?: unknown) {
  const extra = data === undefined ? '' : ` ${JSON.stringify(data)}`;
  console.error(`[sidecar] ${message}${extra}`);
}

function requireObject<T>(raw: unknown, method: string): T {
  if (!raw || typeof raw !== 'object') {
    throw new RpcError(RpcErrorCode.InvalidParams, `${method}: params must be an object`);
  }
  return raw as T;
}

export function registerHandlers(
  server: JsonRpcServer,
  registry: AdapterRegistry = new AdapterRegistry(),
  options: { onShutdown?: () => void } = {},
) {
  const emit = (event: SidecarEvent) => server.emit(event);
  const startedAt = Date.now();
  const preload = registry.preload();

  server.register('hello', async (raw): Promise<HelloResult> => {
    const params = (raw ?? {}) as Partial<HelloParams>;
    sidecarLog('hello', { hostProtocolVersion: params.protocolVersion, host: params.host });
    await preload;
    return {
      protocolVersion: PROTOCOL_VERSION,
      module: { name: MODULE_NAME, version: MODULE_VERSION },
      methods: server.methods(),
      adapters: registry.info(),
      pid: process.pid,
      runtime: process.version,
    };
  });

  server.register('ping', async (): Promise<PingResult> => ({
    pong: true,
    uptimeMs: Date.now() - startedAt,
    // 不算 ping 自己
    inflight: Math.max(0, server.inflight - 1),
  }));

  server.register('configure', async (raw): Promise<ConfigureResult> => {
    const params = requireObject<ConfigureParams>(raw, 'configure');
    for (const [platform, enabled] of Object.entries(params.adapters ?? {})) {
      if (isPlatform(platform) && typeof enabled === 'boolean') registry.setEnabled(platform, enabled);
    }
    sidecarLog('configure', params.adapters);
    return { adapters: registry.info() };
  });

  server.register('discover', async (raw) => {
    const params = requireObject<DiscoverParams>(raw, 'discover');
    sidecarLog('discover', { platform: params.platform, externalId: params.externalId, valveChannel: params.valveChannel });
    try {
      const matches = await registry.run(params.platform, (adapter) => adapter.discover(params));
      sidecarLog('discover ok', { platform: params.platform, count: matches.length });
      return matches;
    } catch (error) {
      sidecarLog('discover FAILED', { platform: params.platform, message: (error as Error).message });
      throw error;
    }
  });

  server.register('download', async (raw) => {
    const params = requireObject<DownloadParams>(raw, 'download');
    sidecarLog('download', {
      platform: params.platform,
      jobId: params.jobId,
      matchId: params.matchId,
      valveChannel: params.valveChannel,
      hasDemoUrl: Boolean(params.demoUrl),
    });
    try {
      await registry.run(params.platform, (adapter) => adapter.download(params, { emit }));
      sidecarLog('download ok', { jobId: params.jobId });
      return { ok: true };
    } catch (error) {
      sidecarLog('download FAILED', { jobId: params.jobId, message: (error as Error).message });
      throw error;
    }
  });

  server.register('resolve_valve_authcode_urls', async (raw) => {
    const params = requireObject<ResolveValveAuthcodeParams>(raw, 'resolve_valve_authcode_urls');
    sidecarLog('resolve_valve_authcode_urls', { count: params.items?.length ?? 0 });
    if (!params.bot?.username) {
      throw new RpcError(RpcErrorCode.InvalidParams, 'Valve bot username is required', { platform: 'valve' });
    }
    return registry.run('valve', async () => {
      const { resolveValveAuthcodeUrls } = await import('./adapters/valve-downloader/steam-gc.js');
      const { bot } = params;
      const login = {
        username: bot.username,
        password: bot.password ?? undefined,
        secret: bot.secret ?? undefined,
        refreshToken: bot.refreshToken ?? undefined,
      };
      return resolveValveAuthcodeUrls(login, params.items ?? [], emit);
    });
  });

  server.register('resolve_5eplay_player_id', async (raw) => {
    const params = requireObject<Resolve5EPlayPlayerIdParams>(raw, 'resolve_5eplay_player_id');
    return registry.run('5eplay', async () => {
      const { resolve5EPlayPlayerId } = await import('./adapters/5eplay/resolve-player-id.js');
      return { uuid: await resolve5EPlayPlayerId(params.input) };
    });
  });

  server.register('shutdown', async () => {
    sidecarLog('shutdown requested');
    setTimeout(() => (options.onShutdown ?? (() => process.exit(0)))(), 50);
    return { ok: true };
  });
}
