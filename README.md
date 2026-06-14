# CS Demo Analyst

CS2 录像自动发现与下载的桌面应用。绑定多个平台账号后，后台持续拉取新 demo 并统一落盘，便于后续分析。

**技术栈**：Tauri 2 · Nuxt 4 · Node sidecar · DuckDB

## 功能

- 多用户、多子账号管理（5EPlay、Renown、Valve 等）
- 按平台调度发现与下载，支持失败自动重试与手动重试
- 跨通道去重（如 Valve `share_code`）
- 下载队列可视化，可暂停、取消、立即重试

## 支持平台

| 平台 | 状态 |
|------|------|
| 5EPlay | 已接入 |
| Renown | 已接入 |
| Valve（Auth Code） | 已接入 |
| Valve（本地） | 占位，建议先用 Auth Code |

## 数据存储

默认数据目录为应用安装目录（`./`），即 exe 所在文件夹：

- 数据库：`db/analyst.duckdb`
- 录像：`demos/{platform}/{account}/*.dem`
- 运行状态：`state/`

可在应用 **设置** 中修改数据目录。

## 文档

开发与运维说明见 [docs/](docs/README.md)：

- [开发指南](docs/development.md) — 环境、依赖、本地调试
- [打包发布](docs/build.md) — 本地构建与各平台产物
- [CI / Release](docs/ci.md) — GitHub Actions 与发版流程
- [架构说明](docs/architecture.md) — 仓库结构、模块划分、平台适配器
