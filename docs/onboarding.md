# 上手体验（W3-C）

[← 文档目录](README.md) · [UI](ui.md) · [路线图](roadmap.md)

代码：`src/tauri/src-tauri/src/onboarding/`（主玩家、设置、托盘、通知、数据目录、命令）、`src/tauri/src-tauri/src/watch/`（监听 CS2 replays）。
迁移：`migrations/sqlite/008_onboarding.sql`。前端类型：`src/shared-types/src/onboarding.ts`。

## 主玩家（「我」）

- 存在 `app_settings.main_player_steamid`（INTEGER，NULL = 未设置）。
- 命令：
  - `get_main_player()` → `MainPlayerDto`：`steamid`（十进制字符串）、`displayName`、`personId`、`matchCount`；未设置时带 `suggestion`。
  - `set_main_player({ steamid })`：接受 SteamID64、`steamcommunity.com/profiles/<id>`、`[U:1:n]`、`STEAM_X:Y:Z`；`null` 清除。成功后广播 `main_player_changed`。
  - `list_main_player_candidates({ query?, limit? })`：已解析对局里的选手，关注的人排前，再按场次。
- 候选推测（只建议，不写入），按顺序：已绑定的 Valve 子账号 → 其他平台账号关联到的 steamid → 「我关注的人」里出场最多的 → 出场最多的人（至少 2 场且多于第二名）。
- 显示名：手动名 > 身份最近的名字 > 最近一场里的名字。

前端统一用 `useMainPlayer()`（`app/composables/useMainPlayer.ts`），全应用共享一份状态：

```ts
const { steamid, displayName, personId, matchCount, isSet, suggestion,
        player, loaded, ready, refresh, set, clear, onChange } = useMainPlayer();
await ready;                                  // 首屏就要值时
watch(steamid, (id) => load(id));             // 响应式
const off = onChange((p) => load(p.steamid)); // 只在 steamid 变化时回调；在 setup 里调用会自动取消
```

## 首次启动向导 `/welcome`

- 自动进入条件：没走过向导（`onboarding_completed_at` 为空），且没有主玩家、子账号、对局。只在启动后第一次导航检查（`middleware/welcome.global.ts`）。
- 步骤：数据目录 → 获取对局（导入 / 去账户页绑定 / CS2 录像监听状态）→ 我是谁（`MainPlayerPicker`）→ 完成。每步可跳过；设置页「打开向导」可重新进入。
- 去账户页时右下角有「返回首次启动向导」入口（`OnboardingGlobal.vue`）。

## 数据目录

数据库就在数据目录里，所以「用哪个目录」不存数据库：

1. 环境变量 `CS_ANALYST_DATA_ROOT`（测试、便携使用；此时应用内不能改）；
2. 默认目录 `%LOCALAPPDATA%\com.cs-analyst.app\data_root.txt` 里写的路径；
3. 默认目录本身。

`set_data_root({ path })` 写指针文件后重启应用，旧目录的数据不会搬走。

## 托盘、通知、拖拽

- 托盘菜单：打开、立即发现、暂停 / 继续下载、退出。左键单击托盘图标打开窗口。
- 关闭窗口：`close_to_tray = 1` 时隐藏到托盘（托盘没建起来时照常关闭）。
- 通知：监听 `analysis_updated`（status = done），每场只通知一次（`match_notifications`；迁移时存量对局全部标为已通知）。内容如「de_dust2 13:9」，设置了主玩家且在场时按主玩家视角写比分并附胜负。Windows 上直接用 WinRT toast（点击后显示窗口并发 `app_navigate` 跳到 `/matches/<id>`），失败时退回 tauri-plugin-notification。
- 拖拽：`.dem` 拖进窗口走 `import_demos`（会移动文件，和「导入对局」一致）；后台有任务时每 2 秒重试，最多约 1 分钟。

## 监听 CS2 replays 目录

- 目录：设置里手动指定的优先；否则读注册表里的 Steam 路径 → `libraryfolders.vdf` → 每个库的 `steamapps/common/Counter-Strike Global Offensive/game/csgo/replays`。CS2 已安装但 replays 目录还没出现时，每 30 秒检查一次。
- 只处理监听开始后新出现 / 改名 / 写入的 `.dem`。大小连续 3 秒不变、且能以写权限打开（CS2 已关闭句柄）才导入；一直打不开的文件 30 秒后也照样导入。
- **复制**导入：`DownloadOrchestrator::import_demo_opts(app, path, copy = true)`，不挪走 CS2 的文件。导入成功后补一次 `enqueue_backlog()`，即使 sidecar 元数据解析失败也会排队解析。
- 事件：`watch_status`（状态变化）、`replay_imported`（每个文件的结果）。

## 设置（迁移 008 加在 `app_settings`）

| 列 | 默认 | 含义 |
|---|---|---|
| `main_player_steamid` | NULL | 主玩家 |
| `close_to_tray` | 1 | 关闭窗口时最小化到托盘 |
| `notify_new_match` | 1 | 新对局解析完成时通知 |
| `watch_replays` | 1 | 监听 CS2 replays 目录 |
| `replays_dir` | NULL | 手动指定的目录；NULL = 自动检测 |
| `onboarding_completed_at` | NULL | 向导完成 / 跳过的时间；已有对局或账号的老库迁移时直接填上 |

命令：`get_experience_settings` / `update_experience_settings`（返回最新的 `WatchStatusDto`）、`get_watch_status`、`detect_replays_dir`、`send_test_notification`、`get_onboarding_state`、`complete_onboarding`。

## 指标说明

- `<MetricHelp metric="adr" />`：「?」悬停显示，点击固定。`label` 传 `true` 显示指标名，传字符串显示自定义文字。
- 释义数据在 `app/utils/metricGlossary.ts`（`getMetric(key)`、`listMetrics(category)`，自动导入），内容来自 [stats.md](stats.md) 和 [aim.md](aim.md)（含 Swing）。改口径时两边一起改。参考值是粗略的经验区间，不是本地百分位。
