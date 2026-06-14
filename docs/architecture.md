# 架构说明

[← 文档目录](README.md) · [项目介绍](../README.md)

## 整体分工

```
┌─────────────┐     Tauri IPC      ┌──────────────────┐
│  Nuxt 4 UI  │ ◄────────────────► │  Rust 编排层      │
│ apps/desktop│                    │  crates/tauri-app │
└─────────────┘                    └────────┬─────────┘
                                            │ JSON-RPC
                                            ▼
                                   ┌──────────────────┐
                                   │  Node sidecar     │
                                   │  packages/sidecar │
                                   └──────────────────┘
```

- **前端**（`apps/desktop`）：账号、下载队列、设置等界面
- **Rust**（`crates/tauri-app`）：DuckDB 持久化、下载调度、状态机、文件落盘
- **Sidecar**（`packages/sidecar`）：各平台 demo 发现与下载，通过 JSON-RPC 与 Rust 通信

## 仓库结构

```
cs-demo-analyst/
├── apps/desktop/          # Nuxt 4 SPA (@nuxt/ui)
├── packages/
│   ├── sidecar/           # JSON-RPC 下载引擎
│   └── shared-types/      # 前后端共享类型
├── crates/tauri-app/      # Rust + Tauri 壳
├── migrations/duckdb/     # 数据库 schema
└── scripts/               # 打包、图标、版本同步等脚本
```

## 数据层

DuckDB 单文件数据库，schema 见 `migrations/duckdb/001_initial.sql`。主要表：

- `users` / `sub_accounts` — 用户与子账号
- `download_jobs` / `discovered_matches` — 发现与下载任务
- `demos` — 已落盘录像记录
- `app_settings` / `platform_settings` — 全局与平台配置

## 平台适配器

Sidecar 按平台拆分适配器，Rust 侧统一调度：

| 平台 | Sidecar 模块 | 说明 |
|------|----------------|------|
| 5EPlay | `adapters/5eplay` | 5E 对战平台 |
| Renown | `adapters/renown` | Renown 平台 |
| Valve Auth Code | `adapters/valve-authcode` | Steam 分享码下载 |
| Valve Local | `adapters/valve-local` | 占位，建议先用 Auth Code |

Valve 通道间通过 `share_code` 跨通道去重，避免重复下载同一局。
