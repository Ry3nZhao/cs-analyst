# 瞄准训练（W2-F）

页面：`/training`（`src/desktop/app/pages/training.vue`）。训练全部在前端跑，Rust 只负责成绩存取和读取 CS2 灵敏度。

| 位置 | 内容 |
|---|---|
| `src/desktop/app/training/core.ts` | 视角换算、投影、统计工具、模式接口 |
| `src/desktop/app/training/engine.ts` | rAF 循环、Pointer Lock、暂停、时间轴 |
| `src/desktop/app/training/draw.ts` | 公共绘制（点阵背景、准星、目标、特效池、HUD） |
| `src/desktop/app/training/modes/*.ts` | 6 个模式 |
| `src/desktop/app/training/meta.ts` | 模式元数据：主成绩的名称、单位、越高越好还是越低越好 |
| `src/desktop/app/training/spray-patterns.json` | 压枪弹道（由脚本导出，见下） |
| `src/tauri/src-tauri/src/training/` | 命令、存取、CS2 配置读取 |
| `migrations/sqlite/007_training.sql` | 表 |
| `src/shared-types/src/training.ts` | 命令的 JSON 结构 |

## 流程

训练列表（灵敏度设置 + 6 张模式卡，显示上次和最好成绩）→ 设置（选项 + 历史趋势）→ 全屏训练（覆盖整个窗口的 Canvas，Esc 暂停）→ 结果页（指标卡、逐次明细、历史趋势）。

## 灵敏度换算

- 每个鼠标计数转过的角度 = `sensitivity × m_yaw`，`m_yaw = m_pitch = 0.022`。
- 画面用 CS2 的投影：竖直视场固定 `2·atan(0.75·tan 45°) = 73.74°`（4:3 下水平 90°），水平视场随宽高比变（16:9 下 106.26°）。焦距 `f = (H/2) / tan(vfov/2)` 像素。
- 世界里的目标用方向（yaw, pitch）表示，按真实透视投影到屏幕。所以同样的鼠标位移，转过的角度和游戏里一样，和窗口大小无关；窗口小只是看到的范围小。
- 灵敏度来源，按优先级：
  1. 自动读取 `<Steam>/userdata/<accountid>/730/local/cfg/cs2_user_convars_0_slot0.vcfg` 里的 `"sensitivity"`。Steam 目录先查注册表 `HKCU\Software\Valve\Steam\SteamPath`，再试默认安装位置。多个账号时取最近修改的，界面上可以切换。
  2. 手动选择 `.vcfg` 或 `.cfg` 文件（`sensitivity 1.25` 这种写法也认，最后一行生效）。
  3. 手动输入。
- 结果存在 localStorage（`training.sensitivity`），每次训练的 settings 里也会记下当时的灵敏度、每计数角度、水平视场和是否锁定了鼠标。

## 输入

- 需要转视角的模式（甩枪、跟枪、微调、压枪）在点击「开始」时请求 Pointer Lock，优先用 `unadjustedMovement: true`，即不经过 Windows 鼠标加速的原始位移；不支持时退回普通锁定。
- 拿不到锁时退回光标位移，结果页会提示手感和游戏不一致。
- `localStorage['training.disablePointerLock'] = '1'` 会跳过锁定，只用于自动化测试，避免锁住真实鼠标。
- 时间：引擎的 `now` = `performance.now()` − 累计暂停时长，鼠标和键盘事件的 `timeStamp` 换算到同一条时间轴，命中和反应时间都按事件时间算，不按帧时间。
- 性能：`update` / `draw` 不新建数组和对象，特效用固定容量的环形池，HUD 文本只在数值变化时重建。右下角显示帧率，结果里记录平均帧率。

## 模式

| 模式 | 玩法 | 主成绩 | 其他指标 / 每次尝试 |
|---|---|---|---|
| 反应测试 `reaction` | 灰屏随机等 1.2–3.5 秒后变成强调色，点击。变色前点击算犯规，这次重来 | 中位数 ms（越低越好） | 平均、最好、最差、标准差、犯规数。刺激时间取变色那一帧的 rAF 时间戳 |
| 甩枪 `flick` | 目标出现在视线 10–38° 外（竖直分量压到 0.55），限时 1–2.5 秒，一次一个 | 命中率 = 命中目标 / 目标数 | 精度（命中 / 开枪）、命中目标的平均和中位用时、未命中时离目标边缘的距离 |
| 跟枪 `tracking` | 目标像对手 AD 一样变向横移，每 0.35–1.1 秒换速度，带加减速；准星留在目标上就算，不用开枪 | 在目标上的时间占比 | 平均偏离角度、每秒的占比。开始前 1 秒静止准备不计 |
| 微调 `micro` | 小目标（角半径 0.3–0.6°）出现在准星 1.2–4.5° 内 | 精度 = 命中 / 开枪 | 命中率、用时、偏离距离 |
| 急停节奏 `strafe` | 按住 A / D 跑起来，反向急停，速度进入阈值时点左键 | 急停成功率 | 时机（进入阈值到开枪）、开枪时的速度 |
| 压枪练习 `spray` | 选武器，按住左键连发，把准星沿弹道反方向拉回 | 控枪（%） | 平均误差、开始偏出的弹序、全程稳住的连发数，每发子弹的 err |

命中判定：开枪那一刻视线和目标中心的夹角 ≤ 目标角半径。

### 急停的速度模型

64 tick 固定步长，一维（向右为正），参数取 CS2 默认值：

```
摩擦：speed > 0 时 drop = max(speed, sv_stopspeed) × sv_friction × dt
加速：add = maxspeed − v·wishdir；accelspeed = min(sv_accelerate × maxspeed × dt, add)
sv_friction = 5.2，sv_accelerate = 5.5，sv_stopspeed = 80
```

- AK-47（215 u/s）全速时反向急停约 5 tick（≈ 78 ms）进入阈值，只松键约 13 tick（≈ 200 ms）。急停键按太久会开始反向加速，窗口大约持续 9–10 tick。
- 阈值 = 最大速度 × 34%，和 W1-D 急停率一致。
- 开枪前 0.7 秒内没有跑到最大速度的 80% 的开枪不计（「先跑起来」）。

### 压枪

- **弹道**：`spray-patterns.json`，由 `python -I scripts/training/export_spray_patterns.py` 从 W1-D 的 `spray_shots` 表导出。每把武器、每个弹序取所有连发 `recoil_v` / `recoil_h` 的中位数（recoil = −2 × Δpunch，以第一发为原点）。至少 4 次连发打到的弹序才保留，所以目前序列只有 10–14 发（样例库只有 3 场）。解析的 demo 越多，重新导出后越长。射速取 `crates/aim/src/weapons.rs` 的 `cycle_s`。
- **误差**（和 docs/aim.md 一致，屏幕方向 v 向上、h 向右）：
  - 子弹 = 视角 + recoil
  - aim = Δ视角（相对第一发）
  - err = recoil + aim；err_v > 0 表示压少了
  - control = 在 |recoil| ≥ 1° 的弹序上 `clamp(1 − |err| / |recoil|, 0, 1)` 的平均 × 100
  - mean_err = 第 2–n 发 |err| 的平均；break_shot = 第一个 |err| > 2° 的弹序
- 画面上模拟视角抖动：相机上抬 recoil × 0.225（view_recoil_tracking 0.45 × punch），只影响画面，不影响弹道。
- 少于 4 发的连发不计。

## 表（迁移 007）

| 表 | 粒度 | 列 |
|---|---|---|
| `training_sessions` | 一轮训练 | `id`（uuid）、`mode`、`variant`（压枪的武器名，其他为空串）、`started_at`（ISO）、`duration_ms`、`score`（主成绩，NULL = 没有有效成绩）、`metrics`（JSON）、`settings`（JSON）、`created_at` |
| `training_results` | 一次尝试 | `session_id`（级联删除）、`idx`、`value`、`ok`、`data`（JSON） |

`training_results.value` 的含义：反应 = 反应时间 ms（犯规为 NULL、ok = 0）；甩枪 / 微调 = 目标用时 ms（漏掉为 NULL）；跟枪 = 这一秒在目标上的 %；急停 = 时机 ms（失败为 NULL）；压枪 = 这一发的 |err|°，`data` 里有 errV / errH / recoilV / recoilH。

迁移全部用 `IF NOT EXISTS`，可以重放。

## 命令

| 命令 | 参数 | 返回 |
|---|---|---|
| `save_training_result` | `{ result: TrainingResultInput }` | 新 id |
| `list_training_results` | `{ mode, range?, variant? }`，range = `7d` / `30d` / `90d` / `365d` / `all` | `TrainingSessionDto[]`，按开始时间升序 |
| `get_training_trials` | `{ sessionId }` | `TrainingTrialDto[]` |
| `delete_training_result` | `{ sessionId }` | 是否删除 |
| `detect_cs2_sensitivity` | — | `Cs2SensitivityDetect` |
| `read_cs2_sensitivity` | `{ path }` | 灵敏度 |

主成绩为 NULL 的训练（例如一次有效连发都没有）前端不保存。

## 已知限制

- WebView 有输入和显示延迟（合成器 + 垂直同步），反应测试只适合和自己比。
- 没有做「CS2 内的 FOV / 分辨率缩放」差异：游戏里 4:3 拉伸等设置会改变画面上的感觉，但角度换算仍然一致。
- 高 DPI 下 Chromium 的 `movementX` 在 `unadjustedMovement` 时是原始计数；退回普通锁定时可能被系统缩放影响。
