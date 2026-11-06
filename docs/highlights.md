# 高光 / 失误集锦（W3-A）

[← 文档目录](README.md) · [路线图 2.9](roadmap.md#29-高光--失误集锦) · [基础指标](stats.md) · [枪法](aim.md)

代码：`crates/highlights`（规则，纯函数）、`src/tauri/src-tauri/src/highlights/`（读写库、回填、命令、CS2 播放）。
迁移：`migrations/sqlite/010_highlights.sql`。前端类型：`src/shared-types/src/highlights.ts`。
页面：`/highlights`；设置区块 `HighlightSettingsSection.vue`；类型说明 `app/utils/highlightGlossary.ts`（也登记进 `MetricHelp`，key 为 `highlight.<kind>`）。

## 数据流

```
解析 worker：事件入库 → stats → aim → highlights::on_match_analyzed（读事件表 → crates/highlights::detect → highlights 表）
启动 15 秒后回填，之后每 2 分钟检查一次：
  没算过 / RULES_VERSION 变了 / stats_version 变了（stats 重算过）/ 当时没有枪法数据、现在有了 → 整场重算
list_highlights：读 highlights 表，权重和开关在查询时应用
```

**不重新解析 demo**：所有规则都只用已入库的表（kills、damages、shots、blinds、bomb_events、rounds、
player_round_stats、round_win_probability、aim_engagements）。改任何规则或阈值都要提升
`highlights::RULES_VERSION`，启动后自动重算。阈值都在 `crates/highlights/src/rules.rs` 开头。

## 规则

分数是规则原始分（0–100），排序用「原始分 × 权重」。权重默认 0.7–1.0，设置里可以调成 0–3，也可以关掉某条规则（关掉的规则在查询时过滤，不删数据）。

### 高光

| 类型 | key | 规则 | 原始分 |
|---|---|---|---|
| 多杀 | `multi_kill` | 一回合击杀敌人 ≥ 3（与计分板多杀一致） | 3K 50 / 4K 75 / ACE 100，每个爆头 +2 |
| 快速连杀 | `quick_kills` | 相邻击杀间隔 ≤ 1.2 秒连成一串；两杀须在 0.8 秒内，三杀以上不限总时长 | 35 + 15 × (n − 2) + 速度加分 |
| 残局获胜 | `clutch_win` | 与 stats 的残局口径一致（本方第一次只剩自己、最终赢下回合），从最后一名队友阵亡开始 | 1v1 35 … 1v5 100 |
| 甩枪爆头 | `flick_headshot` | 对方进入视野时准星离头 ≥ 15°，500 ms 内造成第一次伤害，0.5 秒内爆头击杀。只在可见性可靠的对局（`match_aim.visibility_reliable`）出现 | 45 + 角度、速度加分 |
| 穿墙 / 穿烟 / 盲杀 / 跳杀 / 无镜狙 | `wallbang` / `smoke_kill` / `blind_kill` / `airborne_kill` / `noscope_kill` | 击杀事件自带的标记；穿墙、穿烟只算枪械，无镜狙只算 AWP / SSG 08 / SCAR-20 / G3SG1。一次击杀有多个标记时每个一条 | 40–50，爆头 +10，无镜狙按距离加分 |
| 关键击杀 | `swing_kill` | 胜率曲线上这次击杀让**本方**胜率提高 ≥ 25 个百分点 | 10 + 1.6 × 百分点 |
| 关键拆包 | `key_defuse` | 拆包成功时 T 还有人活着（拆包者下包后有击杀），或拆完时 C4 剩 ≤ 1 秒 | 45 + 10 × T 存活 (+20) |
| 忍者拆包 | `ninja_defuse` | 拆包成功时 T 还有人活着，且拆包者从下包到拆完没有击杀 | 65 + 10 × (T 存活 − 1) |

### 失误

| 类型 | key | 规则 | 原始分 |
|---|---|---|---|
| 首死没被补 | `untraded_opening_death` | 开局对枪失败且 5 秒内没被补（`opening_death = 1 AND traded = 0`） | 40，输掉回合 +15 |
| 死时道具没用 | `unused_utility_death` | 本回合买了道具（购买时间内，去掉死亡时成批重发的购买，按携带上限计）、一颗没扔就死了；价值取 stats 推算的未用道具和本回合购买额里较小的，≥ $200 | 20 + 价值 / 25（最多按 $1000 算） |
| 闪到队友 | `team_flash` | 同一颗闪光（同一投掷者、相差 ≤ 2 tick）让队友致盲 ≥ 1.5 秒，且敌人被致盲的总时长不超过队友。没有致盲事件的 demo（ESL）不出 | 15 + 12 × 队友致盲秒数 |
| 队友伤害 | `team_damage` | 一回合内对队友伤害合计 ≥ 25（含燃烧、手雷），或误杀队友 | 15 + 0.4 × 伤害 + 40 × 误杀 |
| 血优输对枪 | `lost_hp_duel` | 被击杀前 4 秒内只有双方互相伤害（没有第三人、自己没被闪），自己开过枪，交火开始时比对方多 ≥ 40 血 | 30 + 0.6 × (血差 − 40) |
| 近距离扫空 | `whiffed_spray` | 步枪 / 冲锋枪 / 机枪连发 ≥ 5 发（间隔 ≤ 1.6 × 射速间隔），没有对敌人造成伤害，2 秒内被 12 米内的敌人击杀 | 30 + 3 × (发数 − 5) |
| 大幅负 Swing | `swing_loss` | 自己的死亡（或误杀、自杀）让本方胜率下降 ≥ 25 个百分点 | 10 + 1.6 × 百分点 |

**没做的**：换弹时被杀。事件表里没有弹匣 / 换弹状态，要做得在 `demo-parse` 里加字段并提升 `PARSER_VERSION`。

**口径说明**：
- Swing 类用的是事件让本方胜率变了多少（曲线上的跳变），不是个人分到的 Swing 份额；胜率曲线是入库时的模型算的。
- 「道具没用」只看本回合买的道具：stats 的推算值会把以前回合的带进来（偶尔推出 $2000 这种不可能的值），所以和本回合购买额取小。捡起和丢弃的道具看不到。
- 比赛结束（最后一回合 `end_tick`）之后的击杀和伤害全部忽略，和 stats 一致。

## 样例回归（`cargo test --lib highlights::`）

3 场样例 demo（复制到临时目录后完整入库）的命中数：

| 类型 | 官匹 | 平台 | 职业（ESL） |
|---|---|---|---|
| 多杀 | 10 | 8 | 7 |
| 快速连杀 | 1 | 3 | 2 |
| 残局获胜 | 1 | 1 | 0 |
| 甩枪爆头 | 3 | 1 | —（可见性不可靠） |
| 穿墙 | 2 | 0 | 3 |
| 穿烟 | 3 | 1 | 10 |
| 盲杀 | 0 | 2 | 4 |
| 跳杀 / 无镜狙 | 0 | 0 | 0 |
| 关键击杀 | 16 | 6 | 6 |
| 关键 / 忍者拆包 | 0 | 0 | 0 |
| 首死没被补 | 14 | 13 | 11 |
| 死时道具没用 | 18 | 14 | 4 |
| 闪到队友 | 7 | 11 | —（无致盲事件） |
| 队友伤害 | 2 | 2 | 1 |
| 血优输对枪 | 3 | 4 | 3 |
| 近距离扫空 | 0 | 5 | 0 |
| 大幅负 Swing | 16 | 6 | 6 |

另外固定了主玩家（`CS_ANALYST_SAMPLE_STEAMID`）的事件：官匹第 12 回合首死被 AWP 打死且没被补（输掉回合，55 分）、第 14 回合死亡让本方胜率 53.1% → 24.3%（−28.9）；平台第 6 回合 M4A1-S 3K、同回合一次穿烟击杀。多杀、残局条数与 `player_match_stats` 的 3K+4K+5K、残局胜场一致。拆包、无镜狙、跳杀等样例里没有出现的规则用 `crates/highlights/src/tests.rs` 的合成数据覆盖。

## 表（迁移 010）

| 表 | 内容 |
|---|---|
| `highlights` | 每个时刻一行：match_id、round、start_tick / end_tick、steamid、kind、category（highlight / mistake）、score（原始分）、detail（JSON，steamid 为字符串） |
| `match_highlights` | 每场的计算记录：rules_version、stats_version、aim_done、status、count、available（这场能产出的规则） |
| `highlight_rules` | 用户改过的规则（enabled、weight）；没有行 = 默认 |
| `highlight_settings` | 单行：lead_secs（提前几秒，默认 5）、netcon_enabled（默认 0）、netcon_port（2121）、extra_launch_args |

都随 `matches` 级联删除（设置表除外）。

## 命令

- `list_highlights({ filters })` → `HighlightList`。filters：steamid、matchId、category、kinds、maps、days、from、to、limit（默认 500）、includeDisabled。单场按回合排序，跨场按加权分排序；`counts` 只受选手 / 对局 / 地图 / 时间约束；`names` 是细节里出现的 steamid → 名字。
- `get_highlight_settings` / `update_highlight_settings({ settings })`。
- `recompute_highlights({ matchIds? })`：手动重算，不传 = 所有已解析对局。
- `prepare_highlight_playback({ id })` → 控制台命令和 demo 状态。
- `launch_highlight_in_cs2({ id })` → 启动方式和提示；实验开关打开时通过 `highlight_netcon` 事件报告进度。

## 页面 `/highlights`

- 默认主玩家（`useMainPlayer()`），可以搜索切换选手；「回到我」切回主玩家。
- 查询参数（也用于从别的页面跳进来）：`steamid`、`type`（`highlight` / `mistake` / 具体 kind，逗号分隔）、`days`、`map`、`match`。例如 `/highlights?type=mistake&days=30` 是「我最近 30 天的所有失误」，`/highlights?match=<id>` 是一场的集锦（比赛详情页头部的「本场集锦」）。
- 每种类型旁的「?」是 `MetricHelp`，解释规则。

## 在 CS2 中播放

保底方案（默认）：
1. 「播放」弹窗给出两条命令：`playdemo "<demo 路径>"` 和 `demo_gototick <起点 − 5 秒>`（不早于回合开始），可以分别复制，也可以一次复制两条（分号分隔）。
2. 「用 CS2 打开」：`steam.exe -applaunch 730 +playdemo "<路径>"`（从注册表 `HKCU\Software\Valve\Steam` 找 steam.exe），找不到时打开 `steam://rungameid/730//+playdemo …`。打开后自动把跳转命令复制到剪贴板，demo 加载完在控制台粘贴即可。CS2 已经开着时 Steam 可能不传启动参数，这时用复制命令。
3. demo 已被压缩（`.dem.zst`）时先解压到 `%TEMP%\cs-analyst-playback\<对局>.dem`（大小和原始 demo 一致才用，只保留最近 2 个）；已删除的提示不可播放。

实验：自动跳转（`-netconport`，设置里默认关闭）：
- 打开后，启动参数多一个 `-netconport <端口>`（默认 2121）和「附加启动参数」。应用每 2 秒尝试连接 `127.0.0.1:<端口>`（最多 3 分钟），连上后读控制台输出，看到 demo 加载完成的日志（或 45 秒后）再等 3 秒，发送 `demo_gototick`。
- CS2 已经开着且端口可连时不再启动 Steam，直接发送 `playdemo`；如果正在播放的就是这个 demo，只发送跳转。
- 已知问题：Windows 上端口可能在 WSAStartup 之前初始化导致打不开，社区的绕法是加 `-tools`；也有人报告 `-insecure -netconport 2121` 可用（见 [roadmap 6.3](roadmap.md#63-cs2--netconport需要在本机实测)）。`-insecure` 会让这次启动进不了 VAC 服务器，只建议看 demo 时用。

### 本机实测 netconport 的步骤

1. 设置 → 集锦 → 打开「自动跳转（实验）」，端口保持 2121。
2. 关掉 CS2，在集锦页任选一条点「播放」→「用 CS2 打开」。弹窗里会依次显示「等待端口 → 已连接 → 等待加载 → 已跳转」。
3. 如果一直停在「等待端口」：
   - PowerShell 里运行 `Get-NetTCPConnection -LocalPort 2121 -State Listen`，看 CS2 有没有监听端口；
   - 没有的话在「附加启动参数」里填 `-tools` 再试一次，仍然不行再试 `-insecure`。
4. 能连上但跳转不对：看应用日志里 `[highlights] netcon` 开头的行；可以把「提前几秒」调大，或手动在控制台执行弹窗里的命令。
5. 结果请记到 `docs/roadmap.md` 6.3 节，可行的话把开关改成默认开启。
