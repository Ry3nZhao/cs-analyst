# CS Analyst — 本地开发与一键测试
#
# 用法:
#   just          列出所有命令
#   just test     一键构建 debug 并启动（读写下方 app-data 目录）
#   just dev      热重载开发模式

set windows-shell := ["powershell.exe", "-NoProfile", "-Command"]

# 已安装应用的本地数据目录（Tauri identifier: com.cs-analyst.app）
app-data := "C:/Users/<user>/AppData/Local/com.cs-analyst.app"

root := justfile_directory()
tauri-app := root / "src/tauri"
debug-exe := root / "src/tauri/src-tauri/target/debug/cs-analyst.exe"
release-exe := root / "src/tauri/src-tauri/target/release/cs-analyst.exe"

default:
    @just --list

# 一键本地测试：准备依赖 → debug 构建 → 启动应用
test: prep build-debug run-debug

# 一键 release 测试：完整打包后启动 release 可执行文件
test-release: build-release run-release

# 热重载开发（cargo tauri dev + Nuxt）
dev:
    pnpm dev

# 完整校验（对齐 CI）
validate *args:
    pnpm validate {{args}}

# 正式发布：打包并收集安装包到 build/release/
release:
    pnpm release

# 安装项目依赖
install:
    pnpm install

# 构建前置：shared-types、sidecar 与 Tauri resources
prep:
    pnpm --filter @cs-analyst/shared-types build
    pnpm --filter @cs-analyst/sidecar build
    pnpm build:sidecar

# Debug 构建（无 NSIS 安装包，速度较快）
build-debug: prep
    pnpm --filter @cs-analyst/tauri-app tauri build --debug --no-bundle

# Release 完整构建（含图标、前端 generate、NSIS 等）
build-release:
    pnpm package

# 启动 debug 构建产物（数据目录见 app-data；前台运行，前端/Rust 日志输出到本终端）
run-debug:
    if (-not (Test-Path '{{debug-exe}}')) { throw "未找到 {{debug-exe}}，请先运行: just build-debug" }
    Write-Host "数据目录: {{app-data}}"
    Write-Host "启动应用（日志输出于本终端，关闭窗口或 Ctrl+C 结束）..."
    & '{{debug-exe}}'

# 启动 release 构建产物
run-release:
    if (-not (Test-Path '{{release-exe}}')) { throw "未找到 {{release-exe}}，请先运行: just build-release" }
    Write-Host "数据目录: {{app-data}}"
    Start-Process '{{release-exe}}'

# 在资源管理器中打开应用数据目录
data:
    explorer '{{app-data}}'

# 查看数据目录顶层内容
data-ls:
    Get-ChildItem '{{app-data}}' | Format-Table Name, Mode, LastWriteTime

# 仅重置 state/（保留 db/ 与 demos/）
clean-state:
    Remove-Item -Recurse -Force '{{app-data}}/state' -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path '{{app-data}}/state' | Out-Null
    Write-Host '已重置 state/'

# 打开 SQLite 数据库所在目录
data-db:
    explorer '{{app-data}}/db'
