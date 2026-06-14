# 开发指南

[← 文档目录](README.md) · [项目介绍](../README.md)

## 环境要求

- Node.js 20+
- pnpm 9+
- Rust（stable）
- **Windows**：Visual Studio Build Tools（含 C++），Tauri 打包必需
- **Linux**：`libwebkit2gtk` 等 Tauri 系统依赖（CI 已列出完整列表）

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
| `pnpm build` | 编译 shared-types、sidecar，并 Nuxt generate |
| `pnpm build:sidecar` | 打包 sidecar 到 Tauri resources |
| `pnpm sync-version` | 同步版本号到各包 |
| `pnpm generate-icons` | 生成应用图标 |

## Sidecar 调试

开发时 sidecar 默认加载 `packages/sidecar/dist/index.js`。可通过环境变量覆盖：

- `CS_DEMO_ANALYST_SIDECAR` — sidecar 入口脚本路径
- `CS_DEMO_ANALYST_NODE` — Node 可执行文件路径

## 数据目录

开发模式下数据落在 `target/debug/`（exe 所在目录）。详见 [项目介绍 — 数据存储](../README.md#数据存储)。
