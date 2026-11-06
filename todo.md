# TODO

> 依据 [docs/roadmap.md](docs/roadmap.md) 拆成可以并行开发的任务。更新于 2026-11-05。
> 每个任务在独立的子会话（独立 worktree 和分支）里完成，做完由主会话合并。

## 已完成

- [x] P0 地基：workspace、迁移事务化、只发布 Windows、文档更新
- [x] P1 解析内核：vendor demoparser、`demo-parse`、解析队列、事件入库、回放文件、样例 demo 回归测试
- [x] 修复本地前端构建：node_modules 链接损坏，用 `pnpm install --frozen-lockfile --force` 重建即可
- [x] **第一波（W1-A～D）已合并到 `dev/analysis`（2026-11-05）**：
  - 全部 56 个 Rust 测试通过，`nuxt generate` 通过。
  - 真实应用里，迁移从 2 一路升到 5，三个回填任务跑完。
  - `get_match_detail` / `search_persons` / `get_head_to_head` / `get_aim_stats` 都用真实数据核对过，新主题页面也检查过。

**合并时发现、第二波要注意的点**：
- ESL 服务器的 demo 没有 `player_blind` 事件，可见性数据也不可靠，所以职业比赛没有 TTD 和准星位置数据。页面上要显示「不可用」，不能显示成 0。
- 启动时 stats、identity、aim 各有一个回填任务。aim 会重新解析 demo（每场约 2 秒），库大时首次启动会比较慢。之后可以考虑把它并入解析队列统一调度，并在界面上显示进度。

---

## 并行开发约定（所有子会话都要遵守）

1. **只改自己负责的文件。** 必须改共享文件时只做「登记」式的一两行改动，例如在 `lib.rs` 里注册命令、在 `MIGRATIONS` 里加一行。共享文件包括：
   - `src/tauri/src-tauri/src/lib.rs`、`build.rs`、`permissions/app-commands.toml`
   - `src/tauri/src-tauri/src/database/mod.rs`
   - `src/shared-types/src/index.ts`
2. **迁移编号已预分配，不要抢。**

   | 编号 | 归属 |
   |---|---|
   | `003` | W1-B 基础指标 |
   | `004` | W1-C 身份与对位 |
   | `005` | W1-D 枪法与压枪 |
   | `006` 起 | 第二波 |

3. **新的 Tauri 命令放在各自的模块里**，例如 `src/stats/commands.rs`，不要往 `src/commands.rs` 里堆。
4. **前后端共享的类型放在各自的文件里**，例如 `src/shared-types/src/stats.ts`，`index.ts` 只加一行 `export * from './stats'`。
5. **测试**：
   - Rust 侧：`cargo test --workspace`（在 `src/tauri/src-tauri` 下运行）。
   - 样例 demo 目录和主玩家见 docs/development.md（本机配置，不进仓库），新指标都要加样例回归测试，并固定关键数字。
   - 前端侧：`pnpm --filter @cs-analyst/desktop generate` 必须通过。
6. **数据口径**：
   - 自杀（attacker = victim，weapon = `world`）和队友伤害不算进正常统计。
   - ESL 服务器的 demo 没有 `player_blind` 事件。
   - 队伍编号 2 = T，3 = CT；SteamID 存为 INTEGER。
7. **纯本地、只支持 Windows、全程中文交流。**
8. **提交**：只提交到自己的分支，不 push，不合并到 master。完成后在最终消息里写清：分支名、做了什么、怎么验证、还有什么没做。

---

## 第一波 ✅ 已合并

### W1-A UI 主题重做（只动前端）

**目标**：去掉现在的紫色风格，换成简洁的**黑白灰**主题。颜色只用于两类场景：

- **功能语义**：成功 / 警告 / 错误 / 进行中，T 方 / CT 方，数值的好坏。
- **视觉引导**：主要操作按钮、当前选中项、需要用户注意的地方。

**负责范围**：`src/desktop/app/**`，包括 `assets/css/main.css`、`layouts/`、`pages/`、`components/`。

**要做的事**：
- [ ] 在 `main.css` 里定义设计 token：
  - 灰阶 8–10 级，作为背景、表面、边框、三级文字色。
  - 语义色：`success`、`warning`、`danger`、`info`、`t-side`、`ct-side`。
  - 一个用于引导的强调色。
  - 圆角、间距、字号、等宽数字（`font-variant-numeric: tabular-nums`）。
- [ ] 配置 `@nuxt/ui` 主题，`primary` 用中性色，用 `app.config.ts`。
- [ ] 重写侧边栏布局和现有 4 个页面（对局、账户、设置、首页）的样式。功能和交互逻辑保持不变。
- [ ] 抽出后续页面会反复用到的基础组件：
  - 数据卡 `StatTile`
  - 状态标签 `StatusChip`
  - 高密度表格样式
  - 区块标题 `SectionHeader`
  - 阵营标记 `SideBadge`（T / CT）
- [ ] 在 `docs/ui.md` 写一页简短的风格说明：配色用途、组件清单、正例和反例。

**完成标准**：
- `nuxt generate` 通过。
- 在 Tauri 应用里逐页检查过。
- 除了语义色和强调色，没有其他彩色。

### W1-B 基础指标计算（Rust，P2 的后端部分）

**负责范围**：
- 新建 `src/tauri/src-tauri/crates/stats/`，放纯计算逻辑，输入 `ParsedDemo` 或数据库里的行。
- 新建 `src/tauri/src-tauri/src/stats/`，放入库逻辑和命令。
- 迁移 `003`。
- 共享类型 `src/shared-types/src/stats.ts`。

**要做的事**：
- [ ] 每人每场的指标：K / D / A、ADR、KAST、HS%、HLTV 2.0、多杀、MVP、存活率。
- [ ] 开局对枪：尝试和成功次数，分 T / CT。
- [ ] 补枪：机会、成功、被补（时间窗 5 秒）。
- [ ] 残局 1vN：根据击杀顺序和存活人数推导。
- [ ] 道具：每颗闪光平均致盲人数和时长、闪光助攻、闪到队友、HE 伤害、燃烧伤害、每回合投掷数、死亡时没用掉的道具价值。
- [ ] 每回合：经济类型（全起 / 强起 / ECO / 手枪局）、胜负原因、比分走势（按队伍而非阵营，要处理换边）。
- [ ] 写入 `player_match_stats` / `player_round_stats`（迁移 003）。在解析 worker 入库之后调用，入口只加一行。
- [ ] 命令：
  - `get_match_detail(match_id)`：计分板、回合时间线、经济曲线、伤害矩阵、对枪矩阵。
  - `list_player_stats(steamid, filters)`。
- [ ] 样例回归测试：用 3 个样例 demo，挑若干名选手的 K / D / ADR / KAST 和计分板人工对照，然后固定下来。

**完成标准**：测试通过。命令返回的 JSON 结构写进 `shared-types`，交给第二波的前端使用。

### W1-C 身份与对位（Rust，P3 的后端部分）

**负责范围**：
- 新建 `src/tauri/src-tauri/src/identity/`。
- 迁移 `004`。
- 共享类型 `src/shared-types/src/identity.ts`。

**要做的事**：
- [ ] 表结构：
  - `persons`：一个人。
  - `identities`：一个身份，包括 platform、external_id、steamid。
  - `person_merges`：合并记录，用于撤销。
- [ ] 自动关联：
  - demo 里出现的每个 steamid 都对应一个身份；没有归属的身份自动建一个人。
  - 现有的 `sub_accounts` 迁移成「我关注的人」。
  - 5E / Renown 子账号通过 `metadata` 或 demo 关联到 steamid。
- [ ] 存量回填：已解析的对局，一次性生成全部身份。
- [ ] 手动操作：合并、拆分、撤销合并、备注、标签。
- [ ] 对位汇总：
  - 物化表 `h2h_stats`，按 person × person 统计：同队场次和胜率、对手场次和胜率、互杀数、互相造成的伤害、对枪胜率（每回合两人第一次交火的结果）、开局对枪战绩。
  - 每场解析完后增量更新。
- [ ] 命令：`search_persons`、`get_person`、`get_head_to_head(a, b)`、`merge_persons`、`split_identity`、`list_teammates(person)`（常一起组排的队友及组排胜率）。
- [ ] 测试：用样例数据检查主玩家在官匹和平台 demo 里能自动归为同一个人；合并和拆分可以来回操作且数据不丢。

### W1-D 枪法与压枪（Rust，P4 的后端部分）

**负责范围**：
- `src/tauri/src-tauri/crates/demo-parse/`，需要时新增逐 tick 提取的 API。
- 新建 `src/tauri/src-tauri/crates/aim/`。
- 迁移 `005`。
- 共享类型 `src/shared-types/src/aim.ts`。

**要做的事**：
- [ ] `demo-parse`：新增「交火窗口」的逐 tick 提取：每次击杀或伤害之前 N 秒，双方的视角、位置、速度、可见性。不改现有输出，也不提升 `PARSER_VERSION`，除非现有字段的含义变了。
- [ ] 枪法指标：Time to Damage、准星位置（夹角拆成垂直和水平）、反应时间、急停率（开枪时速度和武器精准阈值比较）、敌人可见时命中率、首发命中率。
- [ ] 压枪：
  - 识别连发。
  - 每一发的补偿误差：实际弹道 = 视角 + 后坐力 × 2，以第一发为原点。
  - 按武器汇总：平均弹道、第 N 发的平均误差、「第几发开始崩」。
  - 先做 AK-47、M4A4、M4A1-S、Galil、FAMAS、MP9、MAC-10。
- [ ] 写入 `sprays`、`spray_shots`、`player_match_aim`（迁移 005）。
- [ ] 命令：`get_aim_stats(steamid, filters)`、`get_sprays(match_id, steamid, weapon)`。
- [ ] 测试：用固定的 AK 连发样本，检查第 1–7 发的后坐力序列和补偿误差；TTD、准星位置的数值在合理范围内，并对几次击杀做人工抽查。

---

## 第二波 ✅ 已合并（2026-11-05），W3-C / W3-D 也已合并

- 合并后 `cargo test --workspace` 全部通过（107 个），`nuxt generate` 通过。
- **迁移只按 `user_version` 判断是否执行**：任何分支都不要在共享开发库上启动测试实例，一律用 `CS_ANALYST_DATA_ROOT` 指向一份数据副本。否则编号靠后的迁移先跑，前面的会被永久跳过。
- 已知遗留：Valve demo 导入时 sidecar 不返回 `matchId` 会被跳过（影响监听 replays 目录）；「更新元数据」读不了 `.dem.zst`；`get_sprays` 没有时间筛选，前端补算。

## 第二波

### 第二波约定（在「并行开发约定」基础上追加）

- **迁移编号**：
  - W2-B Swing 用 `006`。
  - W2-F 瞄准训练用 `007`。
  - 其他任务不建表；确实需要的话，先在最终消息里说明，不要占用其他编号。
- **路由归属**（每个任务只新建自己的页面）：

  | 路由 | 归属 |
  |---|---|
  | `/matches/[id]` | W2-A 比赛详情 |
  | `/players`、`/players/[steamid]` | W2-C 选手搜索、选手页（交手、组排），按 steamid 定位到人 |
  | `/aim` | W2-D 枪法与压枪 |
  | `/replay/[id]` | W2-E 2D 回放与热力图 |
  | `/training` | W2-F 瞄准训练 |

  需要跨页跳转的，直接链接到上面这些路由，不必等对方完成。
- **侧边栏导航**：在 `layouts/default.vue` 的导航数组里只加自己那一项。顺序约定为：对局、选手、枪法、训练、账户、设置。回放从比赛详情进入，不放进侧边栏。
- **UI**：
  - 遵循 `docs/ui.md`，用 W1-A 的 token 和组件（`StatTile` / `StatusChip` / `SectionHeader` / `SideBadge`、`.data-table`）。
  - 图表用 SVG 或 Canvas 自己画，不引入大型图表库。确实需要的话，先在最终消息里说明。
  - 数据不可用（例如 ESL demo 的致盲和可见性数据）时显示「不可用」，不能显示成 0。
- **「我」**：目前还没有「主玩家」设置，默认用「我关注的人」（W1-C）里的第一个。主玩家设置放在第三波的上手体验里做。
- **前端依赖**：worktree 里先跑 `pnpm install --frozen-lockfile`；如果构建时所有页面都报 500，改用 `--force` 重装。
- **验证**：
  - 页面类任务必须在 Tauri 应用里实际看过：用 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<端口>` 启动，再通过 CDP 截图（`Page.captureScreenshot`）和读 DOM 来检查。**不要截全屏**，用户正在用这台电脑。
  - 各会话的调试端口不同，避免互相冲突：A=9341，C=9343，D=9344，E=9345，F=9346。
  - 应用数据目录 `%LOCALAPPDATA%\com.cs-analyst.app` 里已有 3 场解析好的样例对局，可以直接用来测试。

| 任务 | 依赖 | 内容 |
|---|---|---|
| W2-A 比赛详情页 | W1-A、W1-B | 计分板（多标签）、回合时间线、经济曲线、伤害 / 对枪矩阵；点玩家可跳到对位页 |
| W2-B Swing | W1-B | 胜率模型、事件归因、每回合胜率曲线（P5） |
| W2-C 身份与对位页面 | W1-A、W1-C | 人员搜索、合并 / 拆分界面、交手页、组排统计 |
| W2-D 枪法与压枪页面 | W1-A、W1-D | 弹道图、误差曲线、武器汇总、趋势 |
| W2-E 2D 回放与热力图 | W1-A | 读取 `.replay.zst`，雷达图底图、倍速、按回合跳转；热力图（P6） |
| W2-F 瞄准训练 | W1-A | 反应、甩枪、跟枪、微调、急停节奏、压枪练习；灵敏度对齐 CS2（P8） |
| W2-G 下载模块化 | 无 | JSON-RPC 协议加版本号并写成文档、模块健康检查和崩溃重启、adapter 单独开关、去掉 `dem-parser.ts`、为拆成 submodule 理清边界（随时可以开始） |

## 第三波 ✅ 全部合并（2026-11-06）

- W3-A 高光/失误集锦、W3-B 个人主页合并后，`cargo test --workspace`、`nuxt generate`、Tauri debug 构建全部通过。
- 合并后补做：第二波页面统一改用 `useMainPlayer()` 并接入 `MetricHelp`；比赛详情页加入 Swing 列和回合胜率曲线；动态路由页的顶栏标题。
- 真实应用逐页 CDP 验收：主页、对局、比赛详情、选手、选手详情、枪法、训练、集锦、2D 回放、设置、账户、欢迎向导。
- 待用户亲测：CS2 `-netconport` 自动跳转（见 `docs/highlights.md`）；训练的 Pointer Lock 手感。
- 已知遗留：Valve demo 导入时 sidecar 不返回 `matchId`；「更新元数据」读不了 `.dem.zst`；「换弹时被杀」规则需要解析器加换弹状态；5E ELO 没有入库。

### 第三波任务

| 任务 | 依赖 | 开始时机 |
|---|---|---|
| W3-A 高光 / 失误集锦（P7）：识别规则、跨场汇总、CS2 联动（先在本机验证 `-netconport`） | W1-B、W1-D、W2-B、W3-C 的主玩家 | 第二波合并后 |
| W3-B 个人主页：趋势、地图 / 武器 / 侧别、个人最佳、本地同段位百分位、「重点改进」 | W1-B、W1-D、W2-B、W3-C 的主玩家 | 第二波合并后 |
| W3-C 上手体验：首次启动向导、**主玩家设置**、托盘、系统通知、拖拽导入、监听 CS2 replays 目录、指标说明组件 | W1-A | 立即开始 |
| W3-D 存储与数据：解析后保留 / 压缩 / 删除原始 demo、按天数或容量自动清理（收藏除外）、存储占用面板、CSV / JSON 导出、整库备份与恢复 | 无 | 立即开始 |

### 第三波约定

- **迁移编号**：W3-C 用 `008`，W3-D 用 `009`，W3-A 用 `010`，W3-B 用 `011`（不需要建表就不用）。
- **主玩家**：
  - W3-C 在 `app_settings` 里加主玩家 steamid，并提供 `get_main_player` / `set_main_player` 命令和前端组合式函数 `useMainPlayer()`。
  - 第二波页面里「我关注的人里的第一个」这个临时取法，在第二波合并后统一改成 `useMainPlayer()`。
- **路由归属**：

  | 路由 | 归属 |
  |---|---|
  | `/welcome` | W3-C 首次启动向导 |
  | `/highlights` | W3-A 集锦 |
  | `/`（首页即个人主页） | W3-B |
  | 设置页新增区块 | W3-C「主玩家 / 监听目录 / 通知」；W3-D「存储与备份」；W2-G「下载模块状态」 |

  三个设置区块各放一个组件，`settings.vue` 里只做引用。
- **指标说明**：
  - W3-C 提供 `MetricHelp` 组件和一份指标释义数据：定义、计算方式、好坏参考值，内容来自 `docs/stats.md`、`docs/aim.md` 和 Swing 文档。
  - 第二波各页面接入 `MetricHelp`，在全部合并后统一处理。
- 其他规则沿用「并行开发约定」和「第二波约定」：只改自己的文件；验证只通过 CDP 截取应用页面，不截全屏。调试端口：W3-A=9351，W3-B=9352，W3-C=9353，W3-D=9354。

## 待补

- [ ] Renown 样例 demo（本地暂时没有）
- [ ] 本机实测 CS2 `-netconport` 能否控制 demo 跳转
