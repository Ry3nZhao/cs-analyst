# CI / Release

[← 文档目录](README.md) · [项目介绍](../README.md)

与 [cs-demo-manager](https://github.com/akiver/cs-demo-manager) 类似，分为 **校验** 与 **发布** 两条流水线。

## Workflows

| Workflow | 触发 | 作用 |
|----------|------|------|
| `validate.yml` | push/PR → `master` | 编译 TS、Nuxt generate、`cargo check` |
| `release.yml` | push → `master`；push tag `v*`；或手动 `workflow_dispatch` | 三平台并行打包 |

## 自动触发

| 事件 | Validate | Release 构建 | GitHub Release |
|------|----------|----------------|----------------|
| push `master` | 是 | 是（产物在 Actions Artifacts） | 否 |
| push tag `v1.0.0` | 否 | 是 | **是**（自动发 Release） |
| 手动 Run workflow | — | 是 | 可选（勾选 Create GitHub Release） |

日常开发：推到 `master` 会自动跑 **Validate** + **Release 构建**（可下载 Artifacts，不会每次 commit 都发 Release）。

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

CI 使用 `pnpm/action-setup` + `actions/setup-node` + `dtolnay/rust-toolchain`；Linux runner 额外安装 `libwebkit2gtk` 等 Tauri 依赖。

本地与 CI 对齐的打包命令见 [打包发布 — 收集构建产物](build.md#收集构建产物)。
