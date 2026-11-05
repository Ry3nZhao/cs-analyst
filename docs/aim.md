# 枪法与压枪分析（W1-D）

代码：`crates/demo-parse/src/engagement.rs`（逐 tick 提取）、`crates/aim`（纯算法）、`src/aim/`（入库、回填、命令）。
迁移：`migrations/sqlite/005_aim.sql`。前端类型：`src/shared-types/src/aim.ts`。

## 约定

- 角度单位是度，时间单位是毫秒。
- 偏移一律换成**屏幕方向**：`v > 0` 向上，`h > 0` 向右。注意引擎里 pitch 向下为正，yaw 向左为正。
- 武器使用规范名，比如 `ak47`、`m4a1`（M4A4）、`m4a1_silencer`、`galilar`。像 `5e_2023pass1_usp_silencer` 这样的平台皮肤名，按最长的 `_<武器名>` 后缀归一。
- 刀、手雷和电击枪不计入枪法统计。
- 队友伤害和自伤不计入统计。

## 交火窗口：`demo_parse::parse_engagements_file`

- **窗口**：每次击杀和每次 `player_hurt` 都有一个窗口，前提是攻击者和受害者是两个不同的玩家。窗口覆盖该事件之前 N 秒（默认 3 秒），按 tick 逐个取，不跳 tick，并截断在本回合冻结时间结束处。
- **每个 tick 的内容**：双方的位置、pitch、yaw、速度、下蹲程度、存活状态、血量，以及 `sees_other`。`sees_other` 表示「对方的 `m_bSpottedByMask` 里有我」。
- **开枪可见性**：每次 `weapon_fire` 都记录当时能看到哪些存活的敌人，以及开枪者的速度。
- **速度算法**：速度用「这一 tick 减上一 tick 的位置 × 64」自己算。解析器自带的 `velocity_*` 只有在前几个 tick 也被提取时才正确，所以不用它（见下文「已知问题」）。
- **对现有功能的影响**：这是一次独立的解析，Release 下约 1.5–4 秒，不改 `parse_demo_file` 的输出，也不提升 `PARSER_VERSION`。

## 可见性质量

- **定义**：在所有「首次伤害」接触中，攻击者在伤害前 8 个 tick 内能看到受害者的比例。
- **样例结果**：
  - 官匹和 5E demo：0.75–0.98。
  - ESL 职业 demo：0.38–0.58，spotted 标记很稀疏，经常是固定 33 tick 一段，很多击杀根本没有可见记录。
- **处理方式**：低于 0.7 的对局，不产出依赖可见性的指标，包括 TTD、反应时间、准星位置、可见时命中率、首发命中率，对应字段为 NULL。压枪、命中率和急停照常计算。

## 枪法指标（`crates/aim/src/engage.rs`、`shots.rs`）

- **接触（contact）**：A 对敌人 V 造成首次伤害。如果 A 在 3 秒内没有再伤到 V，下一次伤害就算新的接触。
- **可见起点（onset）**：从首次伤害往回看，找 A 能看到 V 的那一段连续时间，取它的起点。
  - 中间不超过 8 个 tick 的空档视为连续。
  - 伤害前 8 个 tick 内 A 必须能看到 V，否则视为穿墙或穿烟，不产生样本。
  - 如果这一段一直延伸到窗口开头，起点未知，同样不产生样本。
- **TTD** = 首次伤害 tick − 起点。
- **反应时间** = 起点之后第一次开枪的 tick − 起点。
- **准星位置**：起点那一刻，观察者的视线和「眼睛 → 对方头部」方向之间的夹角，并拆成垂直分量（`v > 0` 表示准星在头上方）和水平分量。
  - 眼睛高度为 64 − 18 × 下蹲程度。
  - 攻击者和受害者两边都会算。
  - 夹角大于 90° 的样本丢弃，因为这时敌人在屏幕外。
- **命中**：开枪的同一个 tick 上对敌人造成了伤害。霰弹枪一枪只算一次。
- **爆头率** = 打中头的次数 / 命中次数。
- **可见时命中率**：只统计开枪时能看到至少一个敌人的射击。
- **首发**：同一把枪在此前 1.5 个射击间隔内没有开过枪。
- **首发命中率**：只统计能看到敌人时打出的首发。
- **急停率**：所有首发中，开枪时水平速度 ≤ 武器最大速度 × 34% 的比例。开镜的狙击枪、AUG 和 SG 553 用开镜后的最大速度。

**偏差说明**：spotted 标记比真实进入视野稍晚，所以 TTD 和准星位置会比用地图几何做射线检测的工具（如 Leetify）显得更快、更准。这些数字适合做相对比较，也就是人和人比、场和场比。

## 压枪（`crates/aim/src/spray.rs`）

- **连发**：同一个人用同一把武器，相邻两枪间隔 ≤ 射速间隔 × 1.5，至少 4 发。
  - 如果相邻两发之间，实际弹道方向跳动超过 5°，就从这里切开。这种情况是转移目标或甩枪，压枪误差没有意义。
- **每一发的计算**（以第一发为原点）：
  - `recoil = −2 × Δpunch`：只有后坐力时子弹会去哪里。把它镜像过来，就是理想的压枪轨迹。
  - `aim = −Δ视角`：玩家实际怎么拉的准星。
  - `err = recoil + aim`：子弹实际落在第一发的哪里。
    - `err_v > 0` 表示子弹偏高，压少了。
    - `err_v < 0` 表示压多了。
  - `tgt_v`、`tgt_h`：子弹相对目标头部的偏差。只有知道目标在这个 tick 的位置时才有值。
- **单次连发的汇总**：
  - `mean_err`：第 2–n 发 |err| 的平均值。
  - `control`：在后坐力 ≥ 1° 的那几发上，计算 `clamp(1 − |err|/|recoil|, 0, 1)` 再取平均，然后 × 100。含义是抵消掉的后坐力比例，和连发长度、武器无关。
  - `break_shot`：第一个 |err| > 2° 的子弹。2° 大约是 25 m 外一个身体的宽度。
- **按武器汇总**（`aim::aggregate`）：
  - 对每个弹序（第 N 发），分别求 recoil、aim、err 的平均值，以及 |err| 的平均值和中位数。
  - 「第几发开始崩」= 第一个「|err| 中位数 > 2° 且至少有 3 次连发打到这一发」的弹序。
- **固定的样例**：用户在官匹第 3 回合的一次 AK 连发。
  - punch 序列是 0 → −0.10 → −0.62 → −1.44 → −2.30 → −3.10 → −3.82，和 roadmap 里验证过的一致。
  - 第 7 发 err = (+2.95, +1.55)，也就是后半段压少了。
  - 第 6 发开始崩，control 为 57.9。

## 表（迁移 005）

| 表 | 粒度 | 要点 |
|---|---|---|
| `match_aim` | 每场一行 | `aim_version`、`status`（done / failed）、`visibility_quality`、`visibility_reliable` |
| `player_match_aim` | 每场 × 人 × 武器，`weapon='*'` 是该人所有武器的汇总 | 计数字段（shots / hits / head_hits / vis_* / first_* / cs_*），TTD、反应时间、准星的中位数和均值，以及 sprays / spray_control / spray_err |
| `aim_engagements` | 每个接触样本 | observer、target、role、weapon、onset_tick、damage_tick、ttd_ms、reaction_ms、cp_deg、cp_v、cp_h、distance、killed |
| `sprays` | 每次连发 | 起止 tick、命中数、目标、是否击杀、首发时的速度、mean_err、control、break_shot |
| `spray_shots` | 每发子弹 | 视角、punch、recoil、aim、err、是否命中、命中部位、tgt |

所有行都会随 `matches` 级联删除。重新分析时整场替换。

## 运行时机

- 解析 worker 入库成功后，调用 `crate::aim::analyze_match`，再解析一次交火窗口然后写库。失败只记到 `match_aim`，不影响主解析。
- 启动 8 秒后在后台回填：所有 `analysis_status='done'`、但没有 `match_aim` 或 `aim_version` 较旧的对局都会补算。
- 算法改了就提升 `aim::AIM_VERSION`，回填会自动重算。

## 命令

- `get_aim_stats({ steamid, filters? })` → `AimStats`
  - filters 支持：matchIds、platforms、maps、from、to、lastMatches、weapon。
- `get_sprays({ matchId?, steamid, weapon? })` → `SpraysResult`
  - 不传 matchId 时，最多列出最新的 500 次连发，但 patterns 用全部数据计算。

两个命令的 JSON 结构见 `src/shared-types/src/aim.ts`。steamid 一律用十进制字符串传递。

## 已知问题

- `demo_parse::Shot::velocity`（以及 `shots.vel_*` 列）在主解析里明显偏大，实测是真实速度的 1.5–4.6 倍，比如 905 u/s 对应真实的 190 u/s。原因应该是第二遍只提取采样 tick，而速度取的是相邻两个**已提取**行的位置差。aim 模块没有用它。要修主解析，得同时提升 `PARSER_VERSION`。
- ESL 职业 demo 没有可靠的可见性数据，所以无法用职业 demo 做 TTD 和准星位置的基准。
- 压枪误差里包含「有意跟枪」的成分。跟移动中的目标会让水平误差变大。`tgt_*` 字段可以用来区分。
