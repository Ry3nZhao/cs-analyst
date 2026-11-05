# 架构说明

[← 文档目录](README.md) · [项目介绍](../README.md) · [路线图](roadmap.md)

## 整体分工

```
┌──────────────┐     Tauri IPC     ┌──────────────────────────────┐
│  Nuxt 4 UI   │ ◄───────────────► │  Rust（Tauri）                │
│ src/desktop  │   命令 + 事件      │  src/tauri/src-tauri          │
└──────────────┘                   │  ├─ orchestrator  下载调度     │
                                   │  ├─ analysis      解析队列     │
                                   │  ├─ database      SQLite      │
                                   │  └─ crates/demo-parse         │
                                   └──────────┬───────────────────┘
                                              │ JSON-RPC（stdio）
                                              ▼
                                   ┌──────────────────────────────┐
                                   │  下载模块（Node sidecar）      │
                                   │  src/sidecar                  │
                                   └──────────────────────────────┘
```

- **前端**（`src/desktop`）：账号、比赛列表、设置等界面
- **Rust**（`src/tauri/src-tauri`）：SQLite 持久化、下载调度、demo 解析、文件落盘
- **下载模块**（`src/sidecar`）：各平台 demo 发现与下载。主程序通过 JSON-RPC 调用它，并负责它的启动和重启。计划将来拆成独立仓库，以 git submodule 引入（见路线图 3.2）

## 仓库结构

```
cs-analyst/
├── src/
│   ├── desktop/              # Nuxt 4 SPA (@nuxt/ui)
│   ├── sidecar/              # 下载模块（Node，JSON-RPC）
│   ├── shared-types/         # 前端 / sidecar 共享类型
│   └── tauri/src-tauri/      # Rust：Tauri 壳 + Cargo workspace 根
│       ├── src/              # 主 crate：orchestrator / analysis / database / commands
│       ├── crates/
│       │   └── demo-parse/   # demo → 规范化事件、开枪状态、回放帧
│       └── vendor/
│           └── demoparser/   # 第三方 CS2 demo 解析器（MIT，本地修改见其 README）
├── migrations/sqlite/        # 数据库 schema（按序号迁移）
├── docs/
└── scripts/                  # 开发、校验、打包脚本
```

## Demo 解析管线

```
下载完成 / 手动导入 / 元数据刷新 / 启动时补齐
        │  apply_dem_metadata（文件头有效）→ AnalysisQueue::enqueue
        ▼
AnalysisQueue（单 worker 线程，去重 FIFO）
        │  demo_parse::parse_demo_file  ← 不持有数据库锁
        │    第 1 遍：事件 + 规则属性（回合数、热身、比赛是否开始）→ 回合时间线
        │    第 2 遍：只取需要的 tick（每 4 tick + 所有事件 tick）→ 位置、视角、后坐力、可见性、经济
        ▼
replace_match_analysis（单事务：删旧行 → 写新行 → matches.analysis_status = done）
analysis/{id}.replay.zst（2D 回放帧，zstd）
        │
        ▼
事件 analysis_updated → 前端刷新
```

- **回合归属**：第 r 回合覆盖 [r 开始, r+1 开始)，回合结束后的补枪仍算第 r 回合。热身中的事件、比赛重开或回档前的事件会被丢弃
- **失败处理**：解析器 panic 会被捕获（release 使用 `panic = "unwind"`），记为 `failed` 并写入原因；自动重试 2 次，手动「重新解析」时计数清零
- **版本**：`demo_parse::PARSER_VERSION` 提升后，旧结果会在启动时自动重新解析
- **全精度 tick 数据不落盘**：单场解析只需约 1 秒，需要逐 tick 的指标（TTD、准星位置）改为重新解析 `.dem`

## 数据层

SQLite 单文件数据库，迁移文件在 `migrations/sqlite/`，通过 `PRAGMA user_version` 记录版本，每个迁移在单独的事务里执行。

| 迁移 | 主要表 |
|---|---|
| 001 | `users` / `sub_accounts`（用户与子账号）、`matches`（发现 + 下载 + 文件头元数据）、`sync_cursors`、`app_settings` / `platform_settings` / `valve_bot_config` |
| 002 | `matches.analysis_*`（解析状态）、`match_players`、`rounds`、`kills`、`damages`、`shots`、`grenades`、`blinds`、`bomb_events`、`purchases`、`economies` |
| 003 | `player_match_stats`、`player_round_stats`、`team_round_stats`（派生指标，口径见 [stats.md](stats.md)） |

约定：SteamID 存为 INTEGER（Steam64）；队伍编号 2 = T、3 = CT；所有解析表都随对局级联删除。

## 平台适配器

下载模块按平台拆分 adapter，由 Rust 侧统一调度：

| 平台 | 模块 | 说明 |
|------|------|------|
| 5EPlay | `adapters/5eplay` | 5E 对战平台 |
| Renown | `adapters/renown` | Renown 平台 |
| Valve Auth Code | `adapters/valve-authcode` | Steam 分享码下载 |
| Valve Local | `adapters/valve-local` | 占位，建议先用 Auth Code |

Valve 的两个下载通道之间按 `share_code` 去重，避免同一局被重复下载。
