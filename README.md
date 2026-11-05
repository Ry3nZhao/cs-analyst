# CS Analyst

CS2 录像自动发现、下载与分析的**纯本地**桌面应用。绑定多个平台账号后，后台持续拉取新 demo，下载完自动解析成可查询的比赛数据。不注册、不上传、无遥测。

**技术栈**：Tauri 2（Rust）· Nuxt 4 · SQLite · Node 下载模块

**平台**：目前只支持与测试 Windows。

## 功能

- 多用户、多子账号管理（5EPlay、Renown、Valve 等）
- 按平台调度发现与下载，支持失败自动重试与手动重试
- 跨通道去重（如 Valve `share_code`）
- 下载队列可视化，可暂停、取消、立即重试
- 下载 / 导入完成后自动完整解析 demo（Rust，单场约 1 秒）：回合、击杀、伤害、开枪（含视角与后坐力）、道具、致盲、下包、购买、经济，以及 2D 回放帧

## 支持平台

| 平台 | 状态 |
|------|------|
| 5EPlay | 已接入 |
| Renown | 已接入 |
| Valve（Auth Code） | 已接入 |
| Valve（本地） | 占位，建议先用 Auth Code |

## 数据存储

数据目录为 `%LOCALAPPDATA%\com.cs-analyst.app\`，可在应用 **设置** 中修改：

- 数据库：`db/analyst.db`（SQLite）
- 录像：`demos/{platform}/*.dem`
- 解析结果：事件写入数据库；2D 回放帧写入 `analysis/{对局 id}.replay.zst`
- 运行状态：`state/`

## 文档

开发与运维说明见 [docs/](docs/README.md)：

- [开发指南](docs/development.md) — 环境、依赖、本地调试
- [打包发布](docs/build.md) — 本地构建与各平台产物
- [CI / Release](docs/ci.md) — GitHub Actions 与发版流程
- [架构说明](docs/architecture.md) — 仓库结构、模块划分、解析管线、平台适配器
- [路线图](docs/roadmap.md) — 对标 Leetify 的分析能力规划
