/**
 * 下载模块 JSON-RPC 协议（stdio，一行一个 JSON）。
 *
 * 这是主程序与下载模块之间唯一的边界：下载模块只依赖这个包，主程序（Rust）按 docs/downloader-protocol.md
 * 实现同一份协议。改动规则：
 * - 新增方法 / 可选字段 / 事件：不升版本，主程序通过 `hello.methods` 判断是否可用。
 * - 删除或改变已有方法、字段的含义：`PROTOCOL_VERSION` + 1，同时更新主程序支持的版本。
 */

/** 协议主版本。主程序只接受与自己相同的主版本。 */
export const PROTOCOL_VERSION = 1;

// ---------------------------------------------------------------------------------------------
// 基础

/** 下载模块支持的平台（每个平台一个 adapter）。 */
export type Platform = 'valve' | 'renown' | '5eplay';
export const PLATFORMS: readonly Platform[] = ['valve', 'renown', '5eplay'];
export type ValveChannel = 'local' | 'authcode';

export interface JsonRpcRequest {
  id: string | number;
  method: string;
  params?: unknown;
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: RpcErrorData;
}

export interface RpcErrorData {
  /** 出错的 adapter（若与某个平台有关） */
  platform?: Platform;
  [key: string]: unknown;
}

export interface JsonRpcResponse {
  id: string | number | null;
  result?: unknown;
  error?: JsonRpcError;
}

/** 错误码。-32700 ~ -32600 沿用 JSON-RPC 2.0；-32000 起是本协议自定义的。 */
export const RpcErrorCode = {
  ParseError: -32700,
  InvalidRequest: -32600,
  MethodNotFound: -32601,
  InvalidParams: -32602,
  /** 未分类的内部错误 */
  Internal: -32000,
  /** adapter 已被主程序停用（`configure`） */
  AdapterDisabled: -32001,
  /** adapter 模块加载失败（例如依赖缺失），其他平台不受影响 */
  AdapterUnavailable: -32002,
  /** adapter 执行时抛错 */
  AdapterFailed: -32003,
  /** demo 地址已失效（过期 / 404），重试无意义；message 以 `URL_EXPIRED` 开头 */
  UrlExpired: -32004,
  /** 未知平台 */
  UnknownPlatform: -32005,
} as const;
export type RpcErrorCode = (typeof RpcErrorCode)[keyof typeof RpcErrorCode];

// ---------------------------------------------------------------------------------------------
// 方法

export interface AdapterInfo {
  platform: Platform;
  label: string;
  /** 主程序是否启用（`configure` 设置；启动时默认全部启用） */
  enabled: boolean;
  /** 模块是否成功加载；false 时 `error` 给出原因 */
  available: boolean;
  error?: string | null;
  capabilities: Array<'discover' | 'download' | 'resolve'>;
}

/** `hello`：握手，主程序启动模块后第一个调用。 */
export interface HelloParams {
  /** 主程序实现的协议版本 */
  protocolVersion: number;
  host: { name: string; version: string };
}

export interface HelloResult {
  protocolVersion: number;
  module: { name: string; version: string };
  /** 本模块实现的方法名 */
  methods: string[];
  adapters: AdapterInfo[];
  pid: number;
  /** Node 版本 */
  runtime: string;
}

/** `ping`：心跳。 */
export interface PingResult {
  pong: true;
  uptimeMs: number;
  /** 正在处理的请求数 */
  inflight: number;
}

/** `configure`：设置 adapter 开关。未出现的平台保持不变。 */
export interface ConfigureParams {
  adapters: Partial<Record<Platform, boolean>>;
}

export interface ConfigureResult {
  adapters: AdapterInfo[];
}

/** `discover`：列出某个账号最近的比赛。 */
export interface DiscoverParams {
  platform: Platform;
  subAccountId: string;
  externalId: string;
  metadata: Record<string, unknown>;
  valveChannel?: ValveChannel | null;
}

export interface DiscoveredMatchDto {
  matchId: string;
  shareCode?: string | null;
  demoUrl?: string | null;
  mapName?: string | null;
  playedAt?: string | null;
  valveChannel?: ValveChannel | null;
  rawPayload: Record<string, unknown>;
}

export type DiscoverResult = DiscoveredMatchDto[];

/** `download`：把 demo 下载并解压到 `outputPath`。 */
export interface DownloadParams {
  jobId: string;
  platform: Platform;
  outputPath: string;
  demoUrl: string;
  matchId: string;
  valveChannel?: ValveChannel | null;
  metadata?: Record<string, unknown>;
}

export interface DownloadResult {
  ok: true;
}

/** `resolve_valve_authcode_urls`：登录 Steam GC，把分享码解析成下载地址。 */
export interface ValveBotLoginDto {
  username: string;
  password?: string | null;
  secret?: string | null;
  refreshToken?: string | null;
}

export interface ResolveValveAuthcodeItemDto {
  jobId: string;
  shareCode: string;
  matchId: string;
}

export interface ResolveValveAuthcodeParams {
  bot: ValveBotLoginDto;
  items: ResolveValveAuthcodeItemDto[];
}

export interface ResolvedValveMatchDto {
  jobId: string;
  shareCode: string;
  matchId: string;
  demoUrl?: string;
  playedAt?: string;
  mapName?: string;
  error?: string;
}

export interface ResolveValveAuthcodeResultDto {
  resolved: ResolvedValveMatchDto[];
  /** 新的 Steam refresh token，主程序应保存 */
  refreshToken?: string;
}

/** `resolve_5eplay_player_id`：把 5E 用户名 / 链接解析成 UUID。 */
export interface Resolve5EPlayPlayerIdParams {
  input: string;
}

export interface Resolve5EPlayPlayerIdResult {
  uuid: string;
}

/** `shutdown`：回复后进程退出。 */
export interface ShutdownResult {
  ok: true;
}

/** 方法名 → 参数 / 返回值。 */
export interface MethodMap {
  hello: { params: HelloParams; result: HelloResult };
  ping: { params: void; result: PingResult };
  configure: { params: ConfigureParams; result: ConfigureResult };
  discover: { params: DiscoverParams; result: DiscoverResult };
  download: { params: DownloadParams; result: DownloadResult };
  resolve_valve_authcode_urls: { params: ResolveValveAuthcodeParams; result: ResolveValveAuthcodeResultDto };
  resolve_5eplay_player_id: { params: Resolve5EPlayPlayerIdParams; result: Resolve5EPlayPlayerIdResult };
  shutdown: { params: void; result: ShutdownResult };
}
export type MethodName = keyof MethodMap;

// ---------------------------------------------------------------------------------------------
// 事件（模块 → 主程序，没有 id，带 `event` 字段）

export interface SidecarEvent {
  event: string;
  [key: string]: unknown;
}

/** 进程启动完成，可以接收请求。 */
export interface ReadyEvent extends SidecarEvent {
  event: 'sidecar.ready';
  protocolVersion: number;
  version: string;
}

export type ResolveProgressPhase = 'started' | 'resolved' | 'failed';

export interface ResolveProgressEvent extends SidecarEvent {
  event: 'resolve.progress';
  jobId: string;
  matchId: string;
  shareCode: string;
  phase: ResolveProgressPhase;
  demoUrl?: string;
  playedAt?: string;
  mapName?: string;
  error?: string;
}

export type ResolveBatchPhase = 'logging_in' | 'resolving' | 'done';

export interface ResolveBatchEvent extends SidecarEvent {
  event: 'resolve.batch';
  total: number;
  completed: number;
  phase: ResolveBatchPhase;
}

export type DownloadProgressPhase = 'downloading' | 'extracting' | 'done';

export interface DownloadProgressEvent extends SidecarEvent {
  event: 'download.progress';
  jobId: string;
  progress: number;
  bytesDownloaded?: number;
  totalBytes?: number;
  phase: DownloadProgressPhase;
}
