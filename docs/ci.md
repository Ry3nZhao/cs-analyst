# CI / Release

[← 文档目录](README.md) · [项目介绍](../README.md)

与 [cs-demo-manager](https://github.com/akiver/cs-demo-manager) 类似，分为 **校验** 与 **发布** 两条流水线。

## Workflows

| Workflow | 触发 | 作用 |
|----------|------|------|
| `validate.yml` | push/PR → `master` | 编译 TS、Nuxt generate、`cargo check` |
| `release.yml` | Validate 成功后（`workflow_run`）；push tag `v*`；或手动 `workflow_dispatch` | 三平台并行打包 |

## 自动触发

| 事件 | Validate | Release 构建 | GitHub Release |
|------|----------|----------------|----------------|
| push `master` | 是 | Validate **成功**后自动触发 | 否 |
| push tag `v*` | 否 | 是 | **是**（自动发 Release） |
| 手动 Run workflow | — | 是 | 可选（勾选 Create GitHub Release） |

日常开发：推到 `master` 先跑 **Validate**；全部 job 通过后 **Release** 才会开始构建（避免校验失败仍浪费三平台打包资源）。

Release 构建前会先执行 `pnpm build:sidecar`，再初始化 Rust cache / Tauri 编译（Tauri `build.rs` 依赖 `resources/sidecar/**`）。

## 正式发版

打 tag 并推送：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

或 Actions → **Release** → Run workflow（可升版本并创建 Release）。

### 手动发布步骤

1. Actions → **Release** → Run workflow
2. 选择 `patch` / `minor` 升版本（或 `none` 仅构建）
3. 勾选 **Create GitHub Release** 后，三平台产物会出现在 Releases 与各 job 的 Artifacts

无需代码签名 Secret。

## CI 环境

- **Node.js**：24.16.0（`.node-version`，与本地一致）
- **Actions**：`checkout` / `setup-node` / `pnpm/action-setup` / `upload-artifact` / `download-artifact` 均为 **v5**（Node 24 运行时）；`rust-cache@v2.9.1`
- **兜底**：workflow 级 `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true`（覆盖尚未升级的第三方 Action）
- **Rust**：`dtolnay/rust-toolchain@stable`；Linux runner 额外安装 `libwebkit2gtk` 等 Tauri 依赖

本地与 CI 对齐的打包命令见 [打包发布 — 收集构建产物](build.md#收集构建产物)。
