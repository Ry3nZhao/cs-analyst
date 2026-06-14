# 打包发布

[← 文档目录](README.md) · [项目介绍](../README.md)

## 本地打包

完整流程（图标 → 前端 → sidecar → 版本同步 → Tauri build）：

```powershell
pnpm package
```

或分步执行：

```powershell
node scripts/bundle-sidecar.mjs
cd crates/tauri-app
cargo tauri build
```

## 各平台产物

`tauri.conf.json` 已设置 `signAndEditExecutable: false`（Windows）；macOS 使用 `signingIdentity: "-"`（无签名）。

| 平台 | 默认产物 |
|------|----------|
| Windows | NSIS `.exe` |
| macOS | `.dmg` |
| Linux | `.AppImage`、`.deb`、`.rpm` |

## 收集构建产物

与 CI 对齐，可用脚本汇总 bundle 输出：

```powershell
pnpm install --frozen-lockfile
pnpm run package
node scripts/collect-bundle.mjs windows   # 或 macos / linux
```

## 发版

GitHub 上的自动构建与 Release 流程见 [CI / Release](ci.md)。
