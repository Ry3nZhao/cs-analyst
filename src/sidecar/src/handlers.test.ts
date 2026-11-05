import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PROTOCOL_VERSION, RpcErrorCode, type JsonRpcResponse } from '@cs-analyst/downloader-protocol';
import { AdapterRegistry, type AdapterDefinition, type AdapterModule } from './adapters/registry.js';
import { MODULE_VERSION, registerHandlers } from './handlers.js';
import { JsonRpcServer } from './rpc.js';

function fake(overrides: Partial<AdapterModule> = {}): AdapterModule {
  return {
    discover: async (p) => [{ matchId: `${p.platform}-1`, rawPayload: {} }],
    download: async () => undefined,
    ...overrides,
  };
}

function setup(defs: Partial<Record<'valve' | 'renown' | '5eplay', AdapterDefinition>> = {}) {
  const def = (label: string, mod: AdapterModule = fake()): AdapterDefinition => ({
    label,
    capabilities: ['discover', 'download'],
    load: async () => mod,
  });
  const registry = new AdapterRegistry({
    valve: defs.valve ?? def('Valve'),
    renown: defs.renown ?? def('Renown'),
    '5eplay': defs['5eplay'] ?? def('5EPlay'),
  });
  const lines: string[] = [];
  const server = new JsonRpcServer((line) => lines.push(line));
  registerHandlers(server, registry, { onShutdown: () => undefined });
  let id = 0;
  const call = async (method: string, params?: unknown): Promise<JsonRpcResponse> => {
    const reqId = ++id;
    await server.handleLine(JSON.stringify({ id: reqId, method, params }));
    const found = lines.map((l) => JSON.parse(l) as JsonRpcResponse).find((r) => r.id === reqId);
    assert.ok(found, `no response for ${method}`);
    return found;
  };
  return { call, lines, server };
}

describe('handshake', () => {
  it('hello reports protocol version, methods and adapters', async () => {
    const { call } = setup();
    const res = await call('hello', { protocolVersion: PROTOCOL_VERSION, host: { name: 'test', version: '0' } });
    const result = res.result as { protocolVersion: number; methods: string[]; adapters: { platform: string; available: boolean }[] };
    assert.equal(result.protocolVersion, PROTOCOL_VERSION);
    for (const m of ['hello', 'ping', 'configure', 'discover', 'download', 'resolve_valve_authcode_urls', 'resolve_5eplay_player_id', 'shutdown']) {
      assert.ok(result.methods.includes(m), m);
    }
    assert.deepEqual(result.adapters.map((a) => a.platform), ['valve', 'renown', '5eplay']);
    assert.ok(result.adapters.every((a) => a.available));
  });

  it('module version matches package.json', () => {
    const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')) as { version: string };
    assert.equal(MODULE_VERSION, pkg.version);
  });
});

describe('errors', () => {
  it('unknown method / bad json / unknown platform', async () => {
    const { call, server, lines } = setup();
    assert.equal((await call('nope')).error?.code, RpcErrorCode.MethodNotFound);
    await server.handleLine('{not json');
    assert.equal((JSON.parse(lines.at(-1)!) as JsonRpcResponse).error?.code, RpcErrorCode.ParseError);
    const res = await call('discover', { platform: 'faceit', externalId: 'x', metadata: {} });
    assert.equal(res.error?.code, RpcErrorCode.UnknownPlatform);
  });
});

describe('adapter switches and isolation', () => {
  it('disabled adapter is rejected, others keep working', async () => {
    const { call } = setup();
    await call('configure', { adapters: { renown: false } });
    const off = await call('discover', { platform: 'renown', externalId: 'x', metadata: {} });
    assert.equal(off.error?.code, RpcErrorCode.AdapterDisabled);
    assert.equal(off.error?.data?.platform, 'renown');
    const on = await call('discover', { platform: '5eplay', externalId: 'x', metadata: {} });
    assert.deepEqual(on.result, [{ matchId: '5eplay-1', rawPayload: {} }]);
    await call('configure', { adapters: { renown: true } });
    assert.ok((await call('discover', { platform: 'renown', externalId: 'x', metadata: {} })).result);
  });

  it('adapter that fails to load or throws does not affect others', async () => {
    const { call } = setup({
      valve: { label: 'Valve', capabilities: ['discover'], load: async () => { throw new Error('missing steam-user'); } },
      renown: {
        label: 'Renown',
        capabilities: ['discover', 'download'],
        load: async () => fake({ download: async () => { throw new Error('URL_EXPIRED: gone'); }, discover: async () => { throw new Error('boom'); } }),
      },
    });
    const hello = (await call('hello', {})).result as { adapters: { platform: string; available: boolean; error: string | null }[] };
    assert.equal(hello.adapters.find((a) => a.platform === 'valve')?.available, false);
    assert.match(hello.adapters.find((a) => a.platform === 'valve')?.error ?? '', /missing steam-user/);

    const valve = await call('discover', { platform: 'valve', externalId: 'x', metadata: {} });
    assert.equal(valve.error?.code, RpcErrorCode.AdapterUnavailable);
    const renown = await call('discover', { platform: 'renown', externalId: 'x', metadata: {} });
    assert.equal(renown.error?.code, RpcErrorCode.AdapterFailed);
    assert.equal(renown.error?.data?.platform, 'renown');
    const expired = await call('download', { platform: 'renown', jobId: 'j', outputPath: 'o', demoUrl: 'u', matchId: 'm' });
    assert.equal(expired.error?.code, RpcErrorCode.UrlExpired);
    assert.match(expired.error?.message ?? '', /^URL_EXPIRED/);
    const ok = await call('discover', { platform: '5eplay', externalId: 'x', metadata: {} });
    assert.ok(Array.isArray(ok.result));
    const ping = await call('ping');
    assert.equal((ping.result as { pong: boolean }).pong, true);
  });
});
