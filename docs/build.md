# 打包发布

[← 文档目录](README.md) · [项目介绍](../README.md)

## 本地打包

与 CI Release 对齐的一键流程：

```powershell
pnpm release
```

产物汇总到 `build/release/`（安装包 + `collect.json` / `summary.json`）。

分步执行：

```powershell
pnpm release:package
pnpm release:collect windows   # 或 macos / linux；省略参数时按当前系统自动选择
```

仍可使用兼容别名：

```powershell
pnpm package
pnpm collect-bundle windows
```

## 各平台产物

`tauri.conf.json` 未配置 Windows 代码签名（不设置 `signCommand` / `certificateThumbprint` 即跳过签名）；macOS 使用 `signingIdentity: "-"`（无签名）。

| 平台 | 默认产物 |
|------|----------|
| Windows | NSIS `.exe` |
| macOS | `.dmg` |
| Linux | `.AppImage`、`.deb`、`.rpm` |

## 输出目录

```
build/
└── release/
    ├── CS-Demo-Analyst_<version>_x64-setup.exe
    ├── package-step.json     # 打包步骤记录
    ├── collect.json          # 收集步骤记录
    └── summary.json          # 全流程完成标记
```

Tauri 原始 bundle 仍在 `crates/tauri-app/src-tauri/target/release/bundle/`。

## 发版

GitHub 上的自动构建与 Release 流程见 [CI / Release](ci.md)。
