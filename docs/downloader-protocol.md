# 下载模块协议

[← 文档目录](README.md) · [架构说明](architecture.md) · [路线图 3.2](roadmap.md)

下载模块（`src/sidecar`，Node）负责各平台 demo 的发现与下载。主程序（Rust）把它当成一个**被管理的子进程**：启动、握手、心跳、崩溃重启、超时、日志都由主程序负责，模块只管「给我账号，返回比赛和文件」。

两边唯一的边界就是本文档描述的 JSON-RPC 协议：

| 位置 | 内容 |
|---|---|
| `src/sidecar/protocol`（`@cs-analyst/downloader-protocol`） | 协议的 TS 类型、`PROTOCOL_VERSION`、错误码 |
| `src/tauri/src-tauri/src/sidecar/protocol.rs` | 同一份协议的 Rust 侧：版本、错误码、每个方法的超时 |
| `src/tauri/src-tauri/src/sidecar/mod.rs` | 进程管理（`SidecarManager`） |

**当前协议版本：1**

---

## 1. 传输

- 主程序用 `node <script>` 启动模块，stdin / stdout 走协议，**一行一个 JSON**（UTF-8，`\n` 结尾）。
- stderr 是自由格式的日志，主程序逐行收集（见 [§6](#6-主程序的管理策略)）。
- stdout 上只能写协议消息，不能 `console.log`。
- 主程序关闭 stdin 时模块应退出（应用退出时不会残留进程）。

消息分三种：

```jsonc
// 请求（主程序 → 模块）
{ "id": 7, "method": "download", "params": { ... } }
// 回复（模块 → 主程序），result 和 error 二选一
{ "id": 7, "result": { "ok": true } }
{ "id": 7, "error": { "code": -32004, "message": "URL_EXPIRED: ...", "data": { "platform": "renown" } } }
// 事件（模块 → 主程序，没有 id）
{ "event": "download.progress", "jobId": "...", "progress": 0.42, "phase": "downloading" }
```

- `id` 由主程序生成（正整数）。请求可以并发，回复顺序不保证。
- 字段名一律 camelCase。

## 2. 版本与兼容

- `PROTOCOL_VERSION` 是一个整数主版本。主程序只接受**相同主版本**的模块，否则把模块标成「版本不兼容」，不再自动重启。
- 下面的改动**不升版本**：新增方法、新增可选字段、新增事件。主程序通过 `hello.methods` 判断某个方法是否可用；调用模块没声明的方法会直接得到 `-32601`，不会发给模块。
- 下面的改动**必须升版本**：删除或改名方法 / 字段、改变已有字段的类型或含义、改变错误码含义。升版本时同时改 `protocol/src/index.ts` 和 `protocol.rs` 的 `PROTOCOL_VERSION`。
- 模块自身的版本（`module.version`，即 `src/sidecar/package.json` 的 `version`）只用于显示和排错，不参与兼容判断。

## 3. 方法

| 方法 | 用途 | 超时 |
|---|---|---|
| `hello` | 握手 | 20 s |
| `ping` | 心跳 | 5 s |
| `configure` | adapter 开关 | 10 s |
| `discover` | 列出账号最近的比赛 | 180 s |
| `download` | 下载并解压 demo | 1800 s |
| `resolve_valve_authcode_urls` | 分享码 → 下载地址（Steam GC） | 600 s |
| `resolve_5eplay_player_id` | 5E 用户名 / 链接 → UUID | 30 s |
| `shutdown` | 让模块退出 | 10 s |

超时由主程序计时；超时后主程序放弃这次调用（返回错误），模块那边的工作不会被打断。

### `hello`

主程序启动模块后的第一个调用。

```ts
params: { protocolVersion: number; host: { name: string; version: string } }
result: {
  protocolVersion: number;
  module: { name: string; version: string };
  methods: string[];          // 本模块实现的方法
  adapters: AdapterInfo[];
  pid: number;
  runtime: string;            // Node 版本
}

interface AdapterInfo {
  platform: 'valve' | 'renown' | '5eplay';
  label: string;
  enabled: boolean;           // 当前开关
  available: boolean;         // 模块是否加载成功
  error?: string | null;      // 加载失败原因
  capabilities: Array<'discover' | 'download' | 'resolve'>;
}
```

模块在回复 `hello` 之前会尝试加载全部 adapter，所以 `available` / `error` 是准确的。

### `ping`

```ts
params: {}
result: { pong: true; uptimeMs: number; inflight: number }
```

### `configure`

设置每个平台 adapter 的开关。没出现的平台保持不变；模块启动时默认全部启用。

```ts
params: { adapters: { valve?: boolean; renown?: boolean; '5eplay'?: boolean } }
result: { adapters: AdapterInfo[] }
```

主程序在三个时机下发开关（数据来自 `platform_settings.enabled`）：每次握手成功后、修改平台设置后、手动重启后。

### `discover`

```ts
params: {
  platform: Platform;
  subAccountId: string;
  externalId: string;                 // 平台账号 ID（Steam64 / 5E UUID / ...）
  metadata: Record<string, unknown>;  // 子账号 metadata；Valve 额外带 steam_api_key / last_share_code
  valveChannel?: 'local' | 'authcode' | null;
}
result: Array<{
  matchId: string;
  shareCode?: string | null;
  demoUrl?: string | null;
  mapName?: string | null;
  playedAt?: string | null;
  valveChannel?: 'local' | 'authcode' | null;
  rawPayload: Record<string, unknown>;
}>
```

### `download`

把 demo 下载到 `outputPath`（压缩包会解压成 `.dem`），期间发 `download.progress` 事件。

```ts
params: {
  jobId: string;          // 进度事件里原样带回
  platform: Platform;
  outputPath: string;     // 绝对路径，由主程序决定
  demoUrl: string;
  matchId: string;
  valveChannel?: 'local' | 'authcode' | null;
  metadata?: Record<string, unknown>;
}
result: { ok: true }
```

下载地址已失效（过期、404）时返回 `-32004`，主程序把这局标成 `expired`，不再重试。

### `resolve_valve_authcode_urls`

登录 Valve bot 账号，连接 CS2 GC，把分享码逐个解析成下载地址。期间发 `resolve.batch` / `resolve.progress` 事件。

```ts
params: {
  bot: { username: string; password?: string | null; secret?: string | null; refreshToken?: string | null };
  items: Array<{ jobId: string; shareCode: string; matchId: string }>;
}
result: {
  resolved: Array<{ jobId; shareCode; matchId; demoUrl?; playedAt?; mapName?; error? }>;
  refreshToken?: string;   // 新 token，主程序应保存
}
```

### `resolve_5eplay_player_id`

```ts
params: { input: string }   // 用户名、主页链接或 UUID
result: { uuid: string }
```

### `shutdown`

```ts
params: {}
result: { ok: true }        // 回复后约 50 ms 进程退出
```

## 4. 事件

| 事件 | 字段 | 说明 |
|---|---|---|
| `sidecar.ready` | `protocolVersion`, `version` | 进程启动完成。主程序不依赖它，以 `hello` 为准 |
| `download.progress` | `jobId`, `progress`（0–1）, `phase`（`downloading` / `extracting` / `done`）, `bytesDownloaded?`, `totalBytes?` | 下载进度 |
| `resolve.batch` | `total`, `completed`, `phase`（`logging_in` / `resolving` / `done`） | 分享码批量解析进度 |
| `resolve.progress` | `jobId`, `matchId`, `shareCode`, `phase`（`started` / `resolved` / `failed`）, `demoUrl?`, `playedAt?`, `mapName?`, `error?` | 单个分享码的结果；主程序据此立即更新数据库 |

## 5. 错误码

| 码 | 名称 | 含义 | 主程序的处理 |
|---|---|---|---|
| -32700 | ParseError | 请求不是合法 JSON（`id` 为 null） | 记日志 |
| -32600 | InvalidRequest | 缺少 `method` | 记日志 |
| -32601 | MethodNotFound | 方法不存在 | `hello` 返回它 → 模块版本过旧，标为不兼容 |
| -32602 | InvalidParams | 参数不合法 | 按失败处理 |
| -32000 | Internal | 未分类错误 | 按失败处理 |
| -32001 | AdapterDisabled | 该平台已被 `configure` 停用 | 按失败处理，message 为「平台 X 已停用」 |
| -32002 | AdapterUnavailable | 该平台的 adapter 加载失败 | 按失败处理；设置页显示加载错误 |
| -32003 | AdapterFailed | adapter 执行时抛错 | 按失败处理，可重试 |
| -32004 | UrlExpired | 下载地址失效；message 以 `URL_EXPIRED` 开头 | 标为 `expired`，不重试 |
| -32005 | UnknownPlatform | 平台名不认识 | 按失败处理 |

与平台有关的错误在 `error.data.platform` 里带上平台名。

**adapter 隔离**：每个 adapter 用 `import()` 按需加载，加载失败只影响它自己（-32002）；执行时抛出的异常被包装成带平台的错误（-32003 / -32004），不会影响其他平台的调用。adapter 里 EventEmitter 抛出的未捕获异常只记录日志，进程继续运行；如果因此卡死，由主程序的心跳发现并重启。

## 6. 主程序的管理策略

实现在 `SidecarManager`（`src/tauri/src-tauri/src/sidecar/mod.rs`），参数在 `SupervisorConfig`：

| 项目 | 策略 |
|---|---|
| 启动 | 应用启动时在后台线程拉起并握手；调用时如果模块没在运行，会先同步拉起 |
| 握手 | `hello`；协议版本不同或模块不支持 `hello` → 状态 `incompatible`，杀掉进程，不自动重启 |
| 心跳 | 每 5 s 一次 `ping`；连续 3 次失败判定卡死，强制结束进程，按崩溃处理 |
| 崩溃重启 | 进程退出后，在途请求立即失败；等待 `1 s × 2^(n-1)`（封顶 30 s）后自动重启，n 为连续失败次数 |
| 重启上限 | 连续失败超过 5 次 → 状态 `failed`，停止自动重启，等用户在设置页手动重启；进程稳定运行 60 s 后连续失败计数清零 |
| 超时 | 每个方法各自的超时（见 [§3](#3-方法)） |
| 日志 | stderr 和管理事件（启动、退出、心跳失败、重启）写入内存环形缓冲（500 行）和 `{数据目录}/logs/downloader.log`（超过 5 MB 时轮转为 `.log.1`） |
| 状态 | `get_downloader_status` 命令和 `downloader_status` 事件：状态、版本、协议版本、PID、自动重启次数、心跳耗时、上次退出、adapter 列表；设置页「下载模块」区块展示 |
| Windows | 以 `CREATE_NO_WINDOW` 启动，不弹控制台窗口 |

状态机：

```
stopped ──start──► starting ──hello ok──► running ──退出 / 心跳超时──► restarting ──退避到期──► starting
                       │                                                   │
                       ├── 版本不兼容 ──► incompatible                       └── 连续失败超过上限 ──► failed
                       └── 握手失败 ────► restarting
incompatible / failed ──手动重启──► starting
```

## 7. 模块的依赖边界

`src/sidecar` 现在对主仓库的依赖只有下面这些，拆分时逐项处理：

| 依赖 | 位置 | 说明 |
|---|---|---|
| `@cs-analyst/downloader-protocol` | `src/sidecar/protocol` | 已在 `src/sidecar` 目录内，随模块一起拆走 |
| pnpm workspace 注册 | 根目录 `pnpm-workspace.yaml` 的 `src/*` 和 `src/sidecar/protocol` | 拆分后改为 submodule 路径 |
| 根目录 `pnpm-lock.yaml` | 依赖锁定 | 拆分后模块自带 lockfile |
| TypeScript 版本 | 模块自己声明 `typescript` devDependency | 无共享 tsconfig，已独立 |
| 打包脚本 | `scripts/common/bundle-sidecar.mjs` | 主仓库读取 `src/sidecar/dist` 和 `protocol/dist`，复制到 `resources/sidecar` 并 `npm install --omit=dev` |
| 开发 / 校验脚本 | `scripts/dev.mjs`、`scripts/validate/*.mjs`、根 `package.json` 的 `build` | 按 `@cs-analyst/sidecar` 名字调用 `build` |
| 脚本路径 | Rust `resolve_sidecar_script()` | 开发时找 `src/sidecar/dist/index.js`，发布时找 `resources/sidecar/index.js`；可用 `CS_ANALYST_SIDECAR` 覆盖 |
| Node 可执行文件 | Rust 侧 `CS_ANALYST_NODE`，默认 PATH 里的 `node` | 与仓库无关 |

模块**不再**依赖：`@cs-analyst/shared-types`（协议类型已移出）、demo 解析（`dem-parser.ts` 已删除，文件头由 Rust `demo_parse::read_demo_file_meta` 读取）、数据库、主程序的任何 Rust 代码。

前端（`src/desktop`）不依赖协议包，只依赖 `shared-types` 里的 `DownloaderStatusDto`（设置页状态区块）。

## 8. 拆成 submodule 的步骤（尚未执行）

1. **建仓库**：用 `git subtree split --prefix=src/sidecar -b downloader-split` 保留历史，推到新仓库（例如 `cs-analyst-downloader`）。
2. **让模块能独立构建**：在新仓库根目录加 `pnpm-workspace.yaml`（`packages: ['.', 'protocol']`），生成自己的 `pnpm-lock.yaml`，加 CI（`pnpm test`）。
3. **主仓库替换为 submodule**：`git rm -r src/sidecar`，`git submodule add <url> src/sidecar`，路径保持不变，这样 `pnpm-workspace.yaml`、打包脚本、`resolve_sidecar_script()` 都不用改。
4. **锁定版本**：主仓库记录 submodule 的 commit；`hello.module.version` 用于排错。协议主版本由两边的 `PROTOCOL_VERSION` 共同约束，升级时先发模块、再升主程序。
5. **协议类型的来源**：Rust 侧的 `protocol.rs` 继续手写，以本文档为准；如果想自动校验，可以在模块 CI 里导出 JSON Schema，主仓库测试里比对。
6. **文档**：本文档移到模块仓库，主仓库 `docs/` 里保留链接。
7. **验证**：`pnpm --filter @cs-analyst/sidecar test`、`pnpm build:sidecar`、启动应用后设置页显示的模块版本 / 协议版本正确。
