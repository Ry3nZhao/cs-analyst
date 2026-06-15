# CI / Release

[← 文档目录](README.md) · [项目介绍](../README.md)

与 [cs-demo-manager](https://github.com/akiver/cs-demo-manager) 类似，分为 **校验** 与 **发布** 两条流水线。

## Workflows

| Workflow | 触发 | 作用 |
|----------|------|------|
| `validate.yml` | push/PR → `master` | 编译 TS、Nuxt generate、`cargo check` |
| `release.yml` | Validate 成功后（`workflow_run`）；push tag `v*`；或手动 `workflow_dispatch` | 三平台并行打包 |

## 发布通道

| 通道 | 触发 | Tag | GitHub Release |
|------|------|-----|----------------|
| **Beta（自动）** | push `master` → Validate 成功 → Release | 自动移动 `latest-beta` | 预发布（`Latest Beta`） |
| **正式** | push tag `v*` 或手动 workflow + 勾选 Create Release | `v1.2.3` | 正式 Release，标为 Latest |

`latest-beta` **不会**触发新的 Release workflow（`on.push.tags` 仅匹配 `v*`），避免循环构建。

## 自动触发

| 事件 | Validate | Release 构建 | GitHub Release |
|------|----------|----------------|----------------|
| push `master` | 是 | Validate **成功**后自动触发 | **是**（`latest-beta` 预发布） |
| push tag `v*` | 否 | 是 | **是**（正式版 `v*`） |
| 手动 Run workflow | — | 是 | 可选（勾选 Create GitHub Release） |

日常开发：推到 `master` 先跑 **Validate**；全部 job 通过后 **Release** 构建三平台，并更新 **`latest-beta`** 预发布包。

Release 构建前会先执行 `pnpm build:sidecar`，再初始化 Rust cache / Tauri 编译（Tauri `build.rs` 依赖 `resources/sidecar/**`）。

## 正式发版

打 tag 并推送（会重新完整构建，不会复用 beta 产物）：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

或 Actions → **Release** → Run workflow（可升版本并创建 Release）。

### 手动发布步骤

1. Actions → **Release** → Run workflow
2. 选择 `patch` / `minor` 升版本（或 `none` 仅构建）
3. 勾选 **Create GitHub Release** 后，三平台产物会出现在 Releases 与各 job 的 Artifacts

### Beta 包获取

- GitHub **Releases** 页 → **Latest Beta**（tag `latest-beta`，预发布）
- 每次 push `master` 成功后，该 Release 会指向最新绿构建的 commit

无需代码签名 Secret。

## CI 环境

- **Node.js**：24.16.0（`.node-version`，与本地一致）
- **Actions**：`checkout` / `setup-node` / `pnpm/action-setup` / `upload-artifact` / `download-artifact` 均为 **v5**（Node 24 运行时）；`rust-cache@v2.9.1`
- **兜底**：workflow 级 `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true`（覆盖尚未升级的第三方 Action）
- **Rust**：`dtolnay/rust-toolchain@stable`；Linux runner 额外安装 `libwebkit2gtk` 等 Tauri 依赖
- **Windows Release**：`windows-2022`（自带 MSVC）

本地与 CI 对齐的打包命令见 [打包发布 — 收集构建产物](build.md#收集构建产物）。
