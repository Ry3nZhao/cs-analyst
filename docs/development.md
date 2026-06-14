# 开发指南

[← 文档目录](README.md) · [项目介绍](../README.md)

## 环境要求

- Node.js **24.16.0**（见根目录 `.node-version`）
- pnpm 9+
- Rust（stable，[rustup](https://rustup.rs/) 安装；验证 `cargo -V`）
- **Windows**：Visual Studio Build Tools（含 C++），Tauri 打包必需
- **Linux**：`libwebkit2gtk` 等 Tauri 系统依赖（CI 已列出完整列表）

### 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"   # 或重开终端
cargo -V
```

若 `pnpm validate` 报 `cargo not found`，说明当前 shell 的 `PATH` 未包含 `~/.cargo/bin`。

### 安装 Node 24.16

任选其一，版本需与 `.node-version` 一致：

- [Node.js 官方安装包](https://nodejs.org/)（选 24.16.x LTS）
- [fnm](https://github.com/Schniz/fnm)：`fnm install 24.16.0 && fnm use 24.16.0`
- [nvm-windows](https://github.com/coreybutler/nvm-windows)：`nvm install 24.16.0 && nvm use 24.16.0`

验证：`node -v` 应输出 `v24.16.x`。

## 安装依赖

```powershell
cd cs-demo-analyst
pnpm install
```

## 本地调试

推荐一键启动（sidecar + Nuxt + Tauri dev）：

```powershell
pnpm dev
```

或分步构建后启动 Tauri：

```powershell
pnpm --filter @cs-demo-analyst/shared-types build
pnpm --filter @cs-demo-analyst/sidecar build
pnpm --filter @cs-demo-analyst/desktop generate

cd crates/tauri-app
cargo tauri dev
```

## 常用脚本

| 命令 | 说明 |
|------|------|
| `pnpm dev` | 本地开发 |
| `pnpm validate` | 本地跑完整 Validate（对齐 CI） |
| `pnpm validate:typescript` | 仅 TS / sidecar 编译 |
| `pnpm validate:frontend` | 仅 Nuxt generate，输出到 `build/validate/desktop/` |
| `pnpm validate:rust` | sidecar 打包 + 图标 + `cargo check` |
| `pnpm release` | 本地 Release：打包并收集安装包到 `build/release/` |
| `pnpm build` | 编译 shared-types、sidecar，并 Nuxt generate |
| `pnpm build:sidecar` | 打包 sidecar 到 Tauri resources |
| `pnpm sync-version` | 同步版本号到各包 |
| `pnpm generate-icons` | 生成应用图标 |

### Validate 输出目录

```
build/
└── validate/
    ├── desktop/          # 前端 generate 副本
    ├── typescript.json
    ├── frontend.json
    ├── rust.json
    └── summary.json
```

已安装依赖时可加 `--skip-install`：`pnpm validate --skip-install`

## Sidecar 调试

开发时 sidecar 默认加载 `packages/sidecar/dist/index.js`。可通过环境变量覆盖：

- `CS_DEMO_ANALYST_SIDECAR` — sidecar 入口脚本路径
- `CS_DEMO_ANALYST_NODE` — Node 可执行文件路径

## 数据目录

开发模式下数据落在 `target/debug/`（exe 所在目录）。详见 [项目介绍 — 数据存储](../README.md#数据存储)。
