# CS Demo Analyst

Tauri 2 + Nuxt 4 桌面应用，Node sidecar 负责 demo 发现与下载，Rust 负责 DuckDB 编排与落盘。

## 结构

```
cs-demo-analyst/
├── apps/desktop/          # Nuxt 4 SPA (@nuxt/ui)
├── packages/sidecar/      # JSON-RPC 下载引擎
├── packages/shared-types/
├── crates/tauri-app/      # Rust + Tauri
└── migrations/duckdb/
```

## 数据与落盘

默认 `data_root`: `C:\CS-Analyst\Data`

- DB: `{data_root}/db/analyst.duckdb`
- Demo: `{data_root}/demos/{platform}/{account}/*.dem`

## 开发

需要：Node 20+、pnpm、Rust、**Visual Studio Build Tools (C++)**（Windows 打包 Tauri 必需）

```powershell
cd cs-demo-analyst
node C:\Users\<user>\AppData\Local\node\corepack\v1\pnpm\11.6.0\bin\pnpm.cjs install --ignore-scripts

node C:\Users\<user>\AppData\Local\node\corepack\v1\pnpm\11.6.0\bin\pnpm.cjs --filter @cs-demo-analyst/shared-types build
node C:\Users\<user>\AppData\Local\node\corepack\v1\pnpm\11.6.0\bin\pnpm.cjs --filter @cs-demo-analyst/sidecar build

cd apps/desktop
node .\node_modules\nuxt\bin\nuxt.mjs generate

cd ../../crates/tauri-app/src-tauri
cargo tauri dev
```

## 打包

```powershell
node scripts/bundle-sidecar.mjs
cd crates/tauri-app
cargo tauri build
```

`tauri.conf.json` 已设置 `signAndEditExecutable: false`（Windows），macOS 使用 `signingIdentity: "-"`（无签名）。各平台默认 bundle 目标：

| 平台 | 产物 |
|------|------|
| Windows | NSIS `.exe` |
| macOS | `.dmg` |
| Linux | `.AppImage`、`.deb`、`.rpm` |

## GitHub Actions（参考 CSDM）

与 [cs-demo-manager](https://github.com/akiver/cs-demo-manager) 类似，分为 **校验** 与 **发布** 两条流水线：

| Workflow | 触发 | 作用 |
|----------|------|------|
| `validate.yml` | push/PR → `main` | 编译 TS、Nuxt generate、`cargo check` |
| `release.yml` | 手动 `workflow_dispatch` | **Windows / macOS / Linux** 并行打包，汇总到 GitHub Release |

### 发布步骤

1. 将仓库推到 GitHub（`main` 分支）
2. Actions → **Release** → Run workflow
3. 选择 `patch` / `minor` 升版本（或 `none` 仅构建）
4. 勾选 **Create GitHub Release** 后，三平台产物会出现在 Releases 与各 job 的 Artifacts

Release 流程（对齐 CSDM 多平台思路）：

```text
prepare（升版本 + push）→ build matrix（win / mac / linux 并行）→ publish（汇总发 Release）
```

无需代码签名 Secret。

### 本地与 CI 对齐

```powershell
pnpm install --frozen-lockfile
pnpm run package
node scripts/collect-bundle.mjs windows   # 或 macos / linux
```

CI 使用 `pnpm/action-setup` + `actions/setup-node` + `dtolnay/rust-toolchain`；Linux runner 额外安装 `libwebkit2gtk` 等 Tauri 依赖。

## 平台适配器

| 平台 | Sidecar 模块 |
|------|----------------|
| 5EPlay | `adapters/5eplay` |
| Renown | `adapters/renown` |
| Valve Auth Code | `adapters/valve-authcode` |
| Valve Local | 占位（建议先用 Auth Code） |
