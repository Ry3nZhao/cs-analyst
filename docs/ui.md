# UI 风格说明

> 适用于 `src/desktop/app/**`。token 定义在 [`main.css`](../src/desktop/app/assets/css/main.css)，@nuxt/ui 主题在 [`app.config.ts`](../src/desktop/app/app.config.ts)。

## 一句话原则

**界面是黑白灰的，颜色只在有含义的地方出现。** 用户扫一眼屏幕，看到的彩色应当只有两种：

1. **功能语义**：状态（成功 / 警告 / 失败 / 进行中）、阵营（T / CT）、数值好坏。
2. **视觉引导**：当前选中项、焦点、进度、需要用户注意的点，统一用强调色 `--accent`。

主要操作按钮用**白底黑字**（`.btn-primary`），不用彩色。

## 配色

### 灰阶（背景、表面、边框、文字）

| token | 用途 |
|---|---|
| `--bg` | 应用底色 |
| `--bg-sunken` | 侧边栏、二级导航 |
| `--surface` | 卡片、表格容器、抽屉、弹窗 |
| `--surface-raised` | 输入框、下拉弹层 |
| `--surface-hover` / `--surface-active` | 叠加在任何表面上的悬停 / 选中底色 |
| `--border-subtle` / `--border` / `--border-strong` | 行分隔 / 卡片与输入框 / 悬停与分组强调 |
| `--text` | 一级文字：标题、关键数字、主键列 |
| `--text-2` | 二级文字：普通单元格、正文说明 |
| `--text-3` | 三级文字：表头、标签、时间、路径 |
| `--text-disabled` | 占位符、破折号、禁用 |

原始灰阶 `--gray-0`（最亮）到 `--gray-950`（最暗）共 11 级，**页面里只用上面的语义 token**，只有在定义新 token 时才引用 `--gray-*`。

### 语义色

| token | 含义 | 典型场景 |
|---|---|---|
| `--success` | 成功 / 好 | 已完成、已解析、正向变化、运行中的服务 |
| `--warning` | 需留意但不是错误 | 过期、缺少配置的提示条 |
| `--danger` | 失败 / 差 / 危险操作 | 下载失败、解析失败、删除按钮悬停、负向变化 |
| `--info` | 进行中 | 下载中、解析中（配合 `pulse`） |
| `--t-side` | T 方 | `SideBadge`、计分板分组、回合胜方 |
| `--ct-side` | CT 方 | 同上 |
| `--value-good` / `--value-bad` | 数值好坏 | 指标高于 / 低于基准（分别指向 success / danger） |

每个语义色都有 `-soft`（约 13% 透明度，作底色）和 `-line`（约 35%，作描边）两个配套 token，例如 `--danger-soft`、`--danger-line`。

### 强调色

`--accent`（青色）只用于引导：导航选中竖条、表格选中行竖条、进度条、输入框焦点环、后台任务胶囊的描边。配套 `--accent-soft`、`--accent-line`、`--focus-ring`。**不要**用它表示状态，也不要大面积铺底。

### 其他 token

- 圆角：`--radius-sm` 4 / `--radius-md` 6 / `--radius-lg` 8 / `--radius-full`
- 间距：`--space-1`…`--space-8`（4px 基准：4 / 8 / 12 / 16 / 20 / 24 / 32）
- 字号：`--text-xs` 11 / `--text-sm` 12 / `--text-md` 13 / `--text-lg` 15 / `--text-xl` 20 / `--text-2xl` 26
- 控件高度：`--control-h` 30、`--control-h-sm` 24；表格行高 `--row-h` 34
- 字体：`--font-sans`（Inter）、`--font-mono`（JetBrains Mono），构建时由 @nuxt/fonts 下载并随应用打包，运行时不联网
- **数字一律等宽**：`table`、`input[type=number]` 默认开启 `tabular-nums`，其他位置加 `.num`

## 组件清单

### Vue 组件（`components/`，自动导入）

| 组件 | 用途 | 主要参数 |
|---|---|---|
| `<StatTile>` | 数据卡，一个指标一张 | `label`、`value`、`unit`、`hint`、`tone`、`delta`、`higherIsBetter`、`compact` |
| `<StatusChip>` | 状态标签 | `tone`（neutral / success / warning / danger / info / accent）、`label`、`pulse`、`dot` |
| `<SectionHeader>` | 区块标题 + 右侧操作区 | `title`、`description`、`count`、`size`（md / lg），默认插槽放按钮 |
| `<SideBadge>` | T / CT 阵营标记 | `side`（'T' / 'CT' / 2 / 3）、`variant`（soft / solid / bar） |
| `<AppModal>` | 弹窗 | `v-model:open`、`title`，`#footer` 插槽 |

```vue
<StatTile label="ADR" :value="84.2" tone="success" :delta="+6.1" compact />
<StatusChip tone="info" pulse label="解析中" />
<SectionHeader title="计分板" :count="10" size="lg"><button class="btn">导出</button></SectionHeader>
<SideBadge :side="2" />  <!-- T -->
```

### 全局样式类（`main.css`）

| 类 | 说明 |
|---|---|
| `.btn-primary` | 白底主按钮，**每个视图最多一个** |
| `.btn` | 默认次要按钮 |
| `.btn-ghost` | 无边框按钮（关闭、低频操作） |
| `.btn-danger` | 危险操作，平时灰色，悬停才显红 |
| `.btn-sm`、`.btn-icon` | 小尺寸、方形图标按钮 |
| `.input`、`.select`、`.input-sm` | 输入框、下拉框 |
| `.field` / `.field-label` / `.field-hint` | 表单字段 |
| `.toggle` + `.toggle-knob` | 开关，开启为白色 |
| `.form-error` | 表单错误 |
| `.callout` + `.callout-warning` / `-danger` / `-info` | 提示条，只有左侧色条带颜色 |
| `.card`、`.card-pad` | 卡片容器 |
| `.tag` | 分类标签（来源、模式），永远灰色 |
| `.progress` + `.progress-fill` | 进度条（强调色） |
| `.kv-grid` | `<dl>` 键值对，用于详情抽屉 |
| `.num`、`.mono`、`.muted`、`.dim`、`.good`、`.bad` | 文本工具类 |

### 高密度表格

```html
<div class="table-wrap">
  <table class="data-table">           <!-- 加 .compact 进一步压缩 -->
    <thead><tr><th>玩家</th><th class="num">K</th><th class="actions"></th></tr></thead>
    <tbody>
      <tr class="clickable selected">
        <td class="strong">s1mple</td>
        <td class="num">24</td>
        <td class="actions"><button class="btn btn-sm">详情</button></td>
      </tr>
      <tr><td colspan="3" class="empty">暂无数据</td></tr>
    </tbody>
  </table>
</div>
```

- 表头吸顶，行高 34px（`.compact` 28px），只有横向分隔线。
- 数值列一律 `.num`：右对齐 + 等宽数字。
- 主键列（玩家名、地图）用 `.strong`，辅助列（时间、路径）用 `.muted`，长文本加 `.truncate`。
- 选中行用 `tr.selected`，左侧出现强调色竖条。

## 正例与反例

| ✅ 这样做 | ❌ 不要这样 |
|---|---|
| 数字保持白色，用 `StatTile` 的 `tone` 小色块表达语义 | 把大数字整个染成绿色 / 红色 / 紫色 |
| 「排队中」「跳过」用 neutral 灰色标签 | 每种状态都分配一个颜色，表格变成彩虹 |
| 来源、模式用灰色 `.tag` | 给每个平台一个品牌色 |
| 一个工具栏只有一个 `.btn-primary`（例如「导入对局」） | 工具栏里每个按钮一个颜色 |
| 删除按钮平时灰色，悬停才变红（`.btn-danger`） | 列表每一行都挂一个红色删除按钮 |
| 选中项 = 浅灰底 + 强调色竖条 | 选中项整块铺强调色 |
| T / CT 一律通过 `SideBadge` 或 `--t-side` / `--ct-side` | 用 `--warning` 表示 T、用 `--info` 表示 CT |
| 颜色写 `var(--danger)` 等语义 token | 在页面里写 `#f16060`、`rgba(...)` 或 `--gray-*` |
| 数值列右对齐 + `tabular-nums` | 数值左对齐、比例宽度数字导致上下错位 |
| 好坏用 `--value-good` / `--value-bad`，中性数值不上色 | 所有数值都上色 |

## @nuxt/ui

- `app.config.ts` 把 `primary` 和 `neutral` 都设为中性灰，`UButton` 等默认就是黑白灰。
- `main.css` 覆盖了 `--ui-bg`、`--ui-border`、`--ui-text*`、`--ui-success` 等变量，让 Toast、Slideover 和应用 token 一致。
- Toast 的 `color` 只用 `success` / `warning` / `error` / `neutral`，不要用 `primary`。
