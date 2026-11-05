import {
  PLATFORMS,
  RpcErrorCode,
  type AdapterInfo,
  type DiscoverParams,
  type DiscoveredMatchDto,
  type DownloadParams,
  type Platform,
} from '@cs-analyst/downloader-protocol';
import type { EventEmitter } from '../lib/emit-progress.js';
import { RpcError } from '../rpc.js';

/**
 * 一个平台的 adapter。各 adapter 按需 `import()`，加载失败（例如依赖缺失）只影响它自己；
 * 调用时抛出的异常也会被包装成带平台信息的 RPC 错误，不会影响其他平台。
 */
export interface AdapterModule {
  discover(params: DiscoverParams): Promise<DiscoveredMatchDto[]>;
  download(params: DownloadParams, ctx: { emit: EventEmitter }): Promise<void>;
}

export interface AdapterDefinition {
  label: string;
  capabilities: AdapterInfo['capabilities'];
  load: () => Promise<AdapterModule>;
}

const BUILTIN: Record<Platform, AdapterDefinition> = {
  valve: {
    label: 'Valve',
    capabilities: ['discover', 'download', 'resolve'],
    load: async () => (await import('./valve/index.js')).valveAdapter,
  },
  renown: {
    label: 'Renown',
    capabilities: ['discover', 'download'],
    load: async () => (await import('./renown/index.js')).renownAdapter,
  },
  '5eplay': {
    label: '5EPlay',
    capabilities: ['discover', 'download', 'resolve'],
    load: async () => (await import('./5eplay/index.js')).fiveEPlayAdapter,
  },
};

interface Slot {
  def: AdapterDefinition;
  enabled: boolean;
  loading?: Promise<AdapterModule>;
  error?: string;
  loaded: boolean;
}

export function isPlatform(value: unknown): value is Platform {
  return typeof value === 'string' && (PLATFORMS as readonly string[]).includes(value);
}

export class AdapterRegistry {
  private slots: Map<Platform, Slot>;

  constructor(defs: Record<Platform, AdapterDefinition> = BUILTIN) {
    this.slots = new Map(
      (Object.entries(defs) as [Platform, AdapterDefinition][]).map(([platform, def]) => [
        platform,
        { def, enabled: true, loaded: false },
      ]),
    );
  }

  /** 预加载全部 adapter（失败不抛出，只记录在 `info()` 里）。 */
  async preload(): Promise<void> {
    await Promise.all([...this.slots.keys()].map((p) => this.load(p).catch(() => undefined)));
  }

  setEnabled(platform: Platform, enabled: boolean) {
    const slot = this.slots.get(platform);
    if (slot) slot.enabled = enabled;
  }

  info(): AdapterInfo[] {
    return [...this.slots.entries()].map(([platform, slot]) => ({
      platform,
      label: slot.def.label,
      enabled: slot.enabled,
      available: slot.error === undefined,
      error: slot.error ?? null,
      capabilities: slot.def.capabilities,
    }));
  }

  /** 取一个已启用、已加载的 adapter；否则抛出对应错误码。 */
  async get(platform: unknown): Promise<{ platform: Platform; adapter: AdapterModule }> {
    if (!isPlatform(platform) || !this.slots.has(platform)) {
      throw new RpcError(RpcErrorCode.UnknownPlatform, `Unknown platform: ${String(platform)}`);
    }
    const slot = this.slots.get(platform)!;
    if (!slot.enabled) {
      throw new RpcError(RpcErrorCode.AdapterDisabled, `平台 ${slot.def.label} 已停用`, { platform });
    }
    try {
      return { platform, adapter: await this.load(platform) };
    } catch (error) {
      throw new RpcError(
        RpcErrorCode.AdapterUnavailable,
        `平台 ${slot.def.label} 的下载模块不可用: ${messageOf(error)}`,
        { platform },
      );
    }
  }

  /** 在 adapter 里执行一次调用，把异常包装成带平台信息的 RPC 错误。 */
  async run<T>(platform: unknown, fn: (adapter: AdapterModule, platform: Platform) => Promise<T>): Promise<T> {
    const { platform: p, adapter } = await this.get(platform);
    return guard(p, () => fn(adapter, p));
  }

  private load(platform: Platform): Promise<AdapterModule> {
    const slot = this.slots.get(platform)!;
    if (!slot.loading) {
      slot.loading = slot.def.load().then(
        (mod) => {
          slot.loaded = true;
          slot.error = undefined;
          return mod;
        },
        (error: unknown) => {
          slot.error = messageOf(error);
          // 允许之后重试加载
          slot.loading = undefined;
          throw error;
        },
      );
    }
    return slot.loading;
  }
}

/** 把 adapter 抛出的异常包装成 RPC 错误：`URL_EXPIRED` → UrlExpired，其余 → AdapterFailed。 */
export async function guard<T>(platform: Platform, fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (error) {
    if (error instanceof RpcError) throw error;
    const message = messageOf(error);
    const code = message.includes('URL_EXPIRED') ? RpcErrorCode.UrlExpired : RpcErrorCode.AdapterFailed;
    throw new RpcError(code, message, { platform });
  }
}

export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
