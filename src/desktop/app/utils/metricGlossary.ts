// W3-C：指标释义数据，供 <MetricHelp metric="adr" /> 使用（Nuxt 自动导入）。
// 口径来自 docs/stats.md（基础指标、Swing）、docs/aim.md（枪法、压枪）和 docs/training.md（瞄准训练）。改口径时两边一起改。
// 参考值是粗略的经验区间（社区常见水平），只用来帮用户建立直觉，不是本地百分位。

export type MetricCategory = 'basic' | 'opening' | 'trade' | 'clutch' | 'utility' | 'economy' | 'aim' | 'spray' | 'swing' | 'training';

export interface MetricReference {
  /** 例如「≥ 85」「< 450 ms」 */
  range: string;
  /** 例如「优秀」「平均」 */
  label: string;
  tone?: 'good' | 'neutral' | 'bad';
}

export interface MetricDefinition {
  key: string;
  /** 显示名（含英文缩写） */
  name: string;
  category: MetricCategory;
  /** 一句话：它衡量什么 */
  definition: string;
  /** 怎么算的（口径） */
  calculation: string;
  unit?: string;
  /** true = 越高越好；false = 越低越好；undefined = 没有好坏 */
  higherIsBetter?: boolean;
  references?: MetricReference[];
  /** 需要注意的限制，例如数据不可用的情况 */
  caveat?: string;
  /** 文档出处 */
  source: 'docs/stats.md' | 'docs/aim.md' | 'docs/training.md';
}

export const METRIC_CATEGORY_LABELS: Record<MetricCategory, string> = {
  basic: '基础',
  opening: '开局对枪',
  trade: '补枪',
  clutch: '残局',
  utility: '道具',
  economy: '经济',
  aim: '枪法',
  spray: '压枪',
  swing: 'Swing',
  training: '瞄准训练',
};

const BLIND_CAVEAT = 'ESL 等职业服务器的 demo 没有致盲事件，这类场次显示「不可用」，不是 0。';
const TRAINING_CAVEAT = '训练在应用窗口里运行，WebView 有一定的输入和显示延迟，成绩只适合和自己以前的成绩比。';
const VIS_CAVEAT = '依赖 demo 里的可见性标记。可见性质量低于 0.7 的对局（例如 ESL 职业 demo）不产出这个指标。spotted 标记比真实进入视野稍晚，数值会比 Leetify 这类用地图几何的工具显得更好，适合人和人、场和场之间比较。';

const list: MetricDefinition[] = [
  // ── 基础 ──
  {
    key: 'kills',
    name: '击杀 K',
    category: 'basic',
    definition: '击杀敌人的次数。',
    calculation: '只算击杀敌人；自杀（摔死、C4 炸死等）和击杀队友不计入。',
    higherIsBetter: true,
    source: 'docs/stats.md',
  },
  {
    key: 'deaths',
    name: '死亡 D',
    category: 'basic',
    definition: '阵亡次数。',
    calculation: '每条死亡事件都算，包括自杀，和游戏内计分板一致；比赛结束后的 world 自杀不算。',
    higherIsBetter: false,
    source: 'docs/stats.md',
  },
  {
    key: 'assists',
    name: '助攻 A',
    category: 'basic',
    definition: '帮队友完成击杀的次数。',
    calculation: '包括闪光助攻；助攻者必须是击杀者的队友。',
    higherIsBetter: true,
    source: 'docs/stats.md',
  },
  {
    key: 'kd',
    name: 'K/D',
    category: 'basic',
    definition: '击杀与死亡之比。',
    calculation: '击杀 ÷ 死亡。',
    higherIsBetter: true,
    references: [
      { range: '≥ 1.2', label: '优秀', tone: 'good' },
      { range: '≈ 1.0', label: '平均', tone: 'neutral' },
      { range: '< 0.8', label: '偏低', tone: 'bad' },
    ],
    source: 'docs/stats.md',
  },
  {
    key: 'adr',
    name: 'ADR 回均伤害',
    category: 'basic',
    definition: '平均每回合对敌人造成的伤害。',
    calculation: '有效伤害 ÷ 参与回合数。有效伤害 = 不超过对方剩余血量的部分；队友伤害不计入。',
    higherIsBetter: true,
    references: [
      { range: '≥ 90', label: '优秀', tone: 'good' },
      { range: '70–85', label: '平均', tone: 'neutral' },
      { range: '< 60', label: '偏低', tone: 'bad' },
    ],
    source: 'docs/stats.md',
  },
  {
    key: 'kast',
    name: 'KAST',
    category: 'basic',
    definition: '对回合有贡献的回合占比。',
    calculation: '有击杀、助攻、存活，或死亡后被队友补枪的回合 ÷ 参与回合数。',
    unit: '%',
    higherIsBetter: true,
    references: [
      { range: '≥ 75%', label: '优秀', tone: 'good' },
      { range: '65–74%', label: '平均', tone: 'neutral' },
      { range: '< 60%', label: '偏低', tone: 'bad' },
    ],
    source: 'docs/stats.md',
  },
  {
    key: 'hsPct',
    name: 'HS% 爆头率',
    category: 'basic',
    definition: '击杀中爆头击杀的比例。',
    calculation: '爆头击杀 ÷ 击杀。',
    unit: '%',
    higherIsBetter: true,
    references: [
      { range: '≥ 55%', label: '很高', tone: 'good' },
      { range: '40–55%', label: '平均', tone: 'neutral' },
    ],
    caveat: '和武器关系很大：步枪、沙鹰偏高，狙击枪、冲锋枪偏低。',
    source: 'docs/stats.md',
  },
  {
    key: 'rating',
    name: 'HLTV 2.0（参照）',
    category: 'basic',
    definition: '社区反推的 HLTV 2.0 Rating，综合击杀、死亡、KAST、ADR。',
    calculation: '0.0073·KAST + 0.3591·KPR − 0.5329·DPR + 0.2372·Impact + 0.0032·ADR + 0.1587，其中 Impact = 2.13·KPR + 0.42·APR − 0.41。',
    higherIsBetter: true,
    references: [
      { range: '≥ 1.15', label: '优秀', tone: 'good' },
      { range: '≈ 1.00', label: '平均', tone: 'neutral' },
      { range: '< 0.85', label: '偏低', tone: 'bad' },
    ],
    caveat: '公式不是官方的，只作参照；本应用的主评价指标是 Swing。',
    source: 'docs/stats.md',
  },
  {
    key: 'multiKills',
    name: '多杀',
    category: 'basic',
    definition: '单回合击杀 2–5 人的回合数。',
    calculation: '按每回合击杀数分别计数（2K / 3K / 4K / 5K）。',
    higherIsBetter: true,
    source: 'docs/stats.md',
  },
  {
    key: 'mvp',
    name: 'MVP（近似）',
    category: 'basic',
    definition: '回合最有价值玩家次数。',
    calculation: '炸弹爆炸算下包者，拆包成功算拆包者，否则算获胜方击杀最多的人（再比伤害）。',
    higherIsBetter: true,
    caveat: '近似值：没有解析 CS2 的 round_mvp 事件，和游戏内可能不同。',
    source: 'docs/stats.md',
  },
  {
    key: 'survival',
    name: '存活率',
    category: 'basic',
    definition: '回合结束时仍然活着的回合占比。',
    calculation: '存活回合 ÷ 参与回合数。',
    unit: '%',
    source: 'docs/stats.md',
  },

  // ── 开局对枪 ──
  {
    key: 'openingDuel',
    name: '开局对枪',
    category: 'opening',
    definition: '每回合第一个敌对击杀的胜负。',
    calculation: '回合内第一次击杀敌人：击杀者算一次成功，被杀者算一次失败，按当时的阵营分 T / CT。成功率 = 成功 ÷（成功 + 失败）。',
    unit: '%',
    higherIsBetter: true,
    references: [
      { range: '≥ 55%', label: '优秀', tone: 'good' },
      { range: '≈ 50%', label: '平均', tone: 'neutral' },
    ],
    source: 'docs/stats.md',
  },

  // ── 补枪 ──
  {
    key: 'tradeKills',
    name: '补枪',
    category: 'trade',
    definition: '队友阵亡后，及时击杀凶手。',
    calculation: '队友阵亡时自己还活着 = 一次机会；5 秒内对凶手造成伤害 = 尝试；5 秒内击杀凶手 = 成功。',
    higherIsBetter: true,
    source: 'docs/stats.md',
  },
  {
    key: 'tradedDeaths',
    name: '被补',
    category: 'trade',
    definition: '自己阵亡后，队友及时击杀了凶手。',
    calculation: '阵亡时还有队友存活 = 一次机会；凶手 5 秒内被队友击杀 = 被补。被补的死亡同样计入 KAST。',
    source: 'docs/stats.md',
  },

  // ── 残局 ──
  {
    key: 'clutch',
    name: '残局 1vN',
    category: 'clutch',
    definition: '只剩自己一人对 N 名敌人时的胜负。',
    calculation: '一方第一次只剩 1 人、另一方还有 N ≥ 1 人时，记为这名玩家的一次 1vN（N 最多计 5，每回合最多一次）。所在方赢下这回合就算残局获胜。',
    higherIsBetter: true,
    source: 'docs/stats.md',
  },

  // ── 道具 ──
  {
    key: 'flashEnemies',
    name: '闪光致盲人数',
    category: 'utility',
    definition: '平均每颗闪光弹致盲的敌人数。',
    calculation: '致盲敌人次数 ÷ 闪光数。只算当时活着的目标，自闪不算，闪到队友另外计数。',
    higherIsBetter: true,
    references: [
      { range: '≥ 1.0', label: '优秀', tone: 'good' },
      { range: '0.5–1.0', label: '平均', tone: 'neutral' },
    ],
    caveat: BLIND_CAVEAT,
    source: 'docs/stats.md',
  },
  {
    key: 'flashDuration',
    name: '闪光致盲时长',
    category: 'utility',
    definition: '平均每颗闪光弹让敌人致盲的总时长。',
    calculation: '致盲敌人总时长 ÷ 闪光数。',
    unit: '秒',
    higherIsBetter: true,
    caveat: BLIND_CAVEAT,
    source: 'docs/stats.md',
  },
  {
    key: 'flashAssists',
    name: '闪光助攻',
    category: 'utility',
    definition: '敌人被你闪到后被队友击杀。',
    calculation: '来自击杀事件的闪光助攻标记，不依赖致盲事件。',
    higherIsBetter: true,
    source: 'docs/stats.md',
  },
  {
    key: 'teamFlashes',
    name: '闪到队友',
    category: 'utility',
    definition: '闪光弹致盲队友的次数。',
    calculation: '致盲事件里目标是队友（不含自己）的次数。',
    higherIsBetter: false,
    caveat: BLIND_CAVEAT,
    source: 'docs/stats.md',
  },
  {
    key: 'utilityDamage',
    name: 'HE / 燃烧伤害',
    category: 'utility',
    definition: '手雷、燃烧弹对敌人造成的伤害。',
    calculation: '武器为 hegrenade、inferno / molotov 的有效伤害。HE 误伤 = HE 对队友造成的伤害。',
    higherIsBetter: true,
    source: 'docs/stats.md',
  },
  {
    key: 'utilityPerRound',
    name: '每回合投掷数',
    category: 'utility',
    definition: '平均每回合扔出的道具数。',
    calculation: '五类道具（闪光、HE、烟雾、燃烧、诱饵）的爆炸 / 起火事件数 ÷ 回合数。',
    source: 'docs/stats.md',
  },
  {
    key: 'unusedUtility',
    name: '死亡时未用道具',
    category: 'utility',
    definition: '阵亡时身上还没扔出去的道具价值。',
    calculation: '用「购买 − 投掷」推算身上的道具，存活则带到下一回合，换边时清空；死亡时剩下的按价格计。',
    unit: '$',
    higherIsBetter: false,
    caveat: '近似值：捡起和丢弃道具无法从事件看出来。',
    source: 'docs/stats.md',
  },

  // ── 经济 ──
  {
    key: 'buyType',
    name: '经济类型',
    category: 'economy',
    definition: '每回合全队的购买力度：手枪局 / ECO / 强起 / 全起。',
    calculation: '半场第一回合是手枪局。其余按全队平均装备价值：低于 $2000 是 ECO；T 方 ≥ $3500、CT 方 ≥ $4000 是全起；中间是强起。装备价值 = freeze end 时的装备 + 之后的购买。',
    source: 'docs/stats.md',
  },

  // ── 枪法 ──
  {
    key: 'ttd',
    name: 'TTD 首伤时间',
    category: 'aim',
    definition: '敌人进入视野到你第一次打中他的时间。',
    calculation: '首次伤害 tick − 可见起点。可见起点 = 伤害前连续能看到对方那一段的开头（中间 ≤ 8 tick 的空档视为连续）；伤害前 8 tick 内看不到对方（穿墙、穿烟）不产生样本。取中位数。',
    unit: 'ms',
    higherIsBetter: false,
    references: [
      { range: '< 450 ms', label: '很快', tone: 'good' },
      { range: '450–650 ms', label: '平均', tone: 'neutral' },
      { range: '> 750 ms', label: '偏慢', tone: 'bad' },
    ],
    caveat: VIS_CAVEAT,
    source: 'docs/aim.md',
  },
  {
    key: 'reaction',
    name: '反应时间',
    category: 'aim',
    definition: '敌人进入视野到你第一次开枪的时间。',
    calculation: '可见起点之后第一次开枪的 tick − 可见起点。取中位数。',
    unit: 'ms',
    higherIsBetter: false,
    caveat: VIS_CAVEAT,
    source: 'docs/aim.md',
  },
  {
    key: 'crosshairPlacement',
    name: '准星位置',
    category: 'aim',
    definition: '敌人刚进入视野时，准星离他头部有多远。',
    calculation: '可见起点那一刻，视线和「眼睛 → 对方头部」方向的夹角，拆成垂直（> 0 表示准星在头上方）和水平分量；夹角 > 90°（敌人在屏幕外）的样本丢弃。取中位数。',
    unit: '°',
    higherIsBetter: false,
    references: [
      { range: '< 5°', label: '优秀', tone: 'good' },
      { range: '5–9°', label: '平均', tone: 'neutral' },
      { range: '> 12°', label: '偏大', tone: 'bad' },
    ],
    caveat: VIS_CAVEAT,
    source: 'docs/aim.md',
  },
  {
    key: 'accuracy',
    name: '命中率',
    category: 'aim',
    definition: '开枪打中敌人的比例。',
    calculation: '开枪的同一 tick 对敌人造成伤害算命中（霰弹枪一枪只算一次）。命中 ÷ 开枪数。',
    unit: '%',
    higherIsBetter: true,
    source: 'docs/aim.md',
  },
  {
    key: 'accuracySpotted',
    name: '可见时命中率',
    category: 'aim',
    definition: '能看到敌人时开枪的命中率，排除了盲射、穿烟压制。',
    calculation: '只统计开枪时能看到至少一个敌人的射击。',
    unit: '%',
    higherIsBetter: true,
    caveat: VIS_CAVEAT,
    source: 'docs/aim.md',
  },
  {
    key: 'headAccuracy',
    name: '爆头命中率',
    category: 'aim',
    definition: '命中的子弹里打中头部的比例。',
    calculation: '打中头的次数 ÷ 命中次数（按子弹，不是按击杀）。',
    unit: '%',
    higherIsBetter: true,
    source: 'docs/aim.md',
  },
  {
    key: 'firstBulletAccuracy',
    name: '首发命中率',
    category: 'aim',
    definition: '能看到敌人时，第一发子弹的命中率。',
    calculation: '首发 = 同一把枪在此前 1.5 个射击间隔内没开过枪。只统计能看到敌人时打出的首发。',
    unit: '%',
    higherIsBetter: true,
    caveat: VIS_CAVEAT,
    source: 'docs/aim.md',
  },
  {
    key: 'counterStrafe',
    name: '急停率',
    category: 'aim',
    definition: '开第一枪时是否已经停稳。',
    calculation: '所有首发中，开枪时水平速度 ≤ 武器最大速度 × 34% 的比例。开镜的狙击枪、AUG、SG 553 用开镜后的最大速度。',
    unit: '%',
    higherIsBetter: true,
    references: [
      { range: '≥ 85%', label: '优秀', tone: 'good' },
      { range: '70–85%', label: '平均', tone: 'neutral' },
      { range: '< 65%', label: '偏低', tone: 'bad' },
    ],
    source: 'docs/aim.md',
  },
  {
    key: 'visibilityQuality',
    name: '可见性质量',
    category: 'aim',
    definition: '这场 demo 的可见性数据是否可靠。',
    calculation: '所有首次伤害里，攻击者在伤害前 8 tick 内能看到受害者的比例。低于 0.7 时不产出依赖可见性的指标。',
    caveat: '官匹和 5E 一般在 0.75–0.98；ESL 职业 demo 只有 0.38–0.58。',
    source: 'docs/aim.md',
  },

  // ── 压枪 ──
  {
    key: 'sprayControl',
    name: '压枪控制',
    category: 'spray',
    definition: '连发时抵消掉的后坐力比例。',
    calculation: '连发 = 同一把枪相邻两枪间隔 ≤ 射速间隔 × 1.5，至少 4 发。在后坐力 ≥ 1° 的那几发上算 clamp(1 − |误差| / |后坐力|, 0, 1)，取平均 × 100。和连发长度、武器无关。',
    higherIsBetter: true,
    references: [
      { range: '≥ 70', label: '优秀', tone: 'good' },
      { range: '50–70', label: '平均', tone: 'neutral' },
      { range: '< 40', label: '偏低', tone: 'bad' },
    ],
    caveat: '误差里包含有意跟枪：跟移动目标会让水平误差变大。',
    source: 'docs/aim.md',
  },
  {
    key: 'sprayError',
    name: '压枪误差',
    category: 'spray',
    definition: '连发中子弹偏离第一发落点的平均角度。',
    calculation: '每发：后坐力 = −2 × Δpunch，准星 = −Δ视角，误差 = 后坐力 + 准星（垂直 > 0 表示偏高、压少了）。单次连发取第 2–n 发 |误差| 的平均。',
    unit: '°',
    higherIsBetter: false,
    source: 'docs/aim.md',
  },
  {
    key: 'breakShot',
    name: '第几发开始崩',
    category: 'spray',
    definition: '连发从第几发起明显失控。',
    calculation: '单次连发：第一发 |误差| > 2° 的子弹（2° 约等于 25 m 外一个身体宽）。按武器汇总：第一个「|误差| 中位数 > 2° 且至少 3 次连发打到这一发」的弹序。',
    higherIsBetter: true,
    source: 'docs/aim.md',
  },

  // ── Swing ──
  {
    key: 'swing',
    name: 'Swing 回合胜率摆动',
    category: 'swing',
    definition: '平均每回合让本方回合胜率提高了多少。',
    calculation: '每个事件（击杀、伤害、下包、拆包、闪光助攻）前后各算一次回合胜率，差值归到相关的人：击杀者拿大头，造成伤害的人、闪光助攻者按比例分；死亡者扣分，被队友补回则少扣。胜率模型考虑双方存活人数、血量、装备、是否下包和剩余时间，并用本地库的回合平滑。',
    unit: '百分点',
    higherIsBetter: true,
    references: [
      { range: '> +2', label: '明显带动', tone: 'good' },
      { range: '−1 ~ +1', label: '持平', tone: 'neutral' },
      { range: '< −2', label: '拖累', tone: 'bad' },
    ],
    caveat: '+3.2 表示平均每回合让本方胜率提高 3.2%；0 表示和平均水平持平。',
    source: 'docs/stats.md',
  },

  // ── 瞄准训练 ──
  {
    key: 'trainingReaction',
    name: '反应时间（训练）',
    category: 'training',
    definition: '屏幕变色到你点击的时间。',
    calculation: '灰屏随机等 1.2–3.5 秒后变色，刺激时间取变色那一帧的时间戳，点击按事件时间算。变色前点击算犯规，这次重来。主成绩取中位数。',
    unit: 'ms',
    higherIsBetter: false,
    caveat: `${TRAINING_CAVEAT}数值不能和其他工具或网站横向比较，也和 demo 里的反应时间不是一个口径。`,
    source: 'docs/training.md',
  },
  {
    key: 'flickHitRate',
    name: '甩枪命中率',
    category: 'training',
    definition: '限时内甩过去点掉的目标比例。',
    calculation: '命中目标数 ÷ 目标数。目标出现在视线 10–38° 外，开枪那一刻视线和目标中心的夹角 ≤ 目标角半径算命中。精度 = 命中 ÷ 开枪数。',
    unit: '%',
    higherIsBetter: true,
    caveat: TRAINING_CAVEAT,
    source: 'docs/training.md',
  },
  {
    key: 'trackingOnTarget',
    name: '跟枪在目标上',
    category: 'training',
    definition: '准星留在移动目标上的时间占比。',
    calculation: '在目标上的时间 ÷ 训练时长。目标像对手 AD 一样变向横移，开始前 1 秒静止准备不计。平均偏离 = 准星到目标中心的平均角度。',
    unit: '%',
    higherIsBetter: true,
    caveat: TRAINING_CAVEAT,
    source: 'docs/training.md',
  },
  {
    key: 'microAccuracy',
    name: '微调精度',
    category: 'training',
    definition: '小目标近距离修正的开枪命中比例。',
    calculation: '命中 ÷ 开枪数。目标角半径 0.3–0.6°，出现在准星 1.2–4.5° 内。命中率 = 命中目标 ÷ 目标数。',
    unit: '%',
    higherIsBetter: true,
    caveat: TRAINING_CAVEAT,
    source: 'docs/training.md',
  },
  {
    key: 'strafeSuccess',
    name: '急停成功率（训练）',
    category: 'training',
    definition: '反向急停后，在速度进入精准阈值时开枪的比例。',
    calculation: '有效开枪里，开枪时速度 ≤ 武器最大速度 × 34%（和 demo 里的急停率同一阈值）的比例。开枪前 0.7 秒内没跑到最大速度 80% 的开枪不计。速度按 CS2 默认的摩擦和加速参数、64 tick 模拟。',
    unit: '%',
    higherIsBetter: true,
    references: [
      { range: '≥ 85%', label: '优秀', tone: 'good' },
      { range: '70–85%', label: '平均', tone: 'neutral' },
      { range: '< 65%', label: '偏低', tone: 'bad' },
    ],
    source: 'docs/training.md',
  },
  {
    key: 'strafeTiming',
    name: '急停时机',
    category: 'training',
    definition: '速度进入阈值到你开枪的时间。',
    calculation: '每次有效开枪：开枪时间 − 速度降到阈值内的时间，取平均。AK-47 全速反向急停约 5 tick（≈ 78 ms）进入阈值，急停键按太久会开始反向加速，窗口大约 9–10 tick。',
    unit: 'ms',
    higherIsBetter: false,
    source: 'docs/training.md',
  },
];

export const METRIC_GLOSSARY: Readonly<Record<string, MetricDefinition>> = Object.freeze(
  Object.fromEntries(list.map((m) => [m.key, m])),
);

/** 别名：方便直接用各页面 DTO 的字段名查 */
const ALIASES: Record<string, string> = {
  k: 'kills',
  d: 'deaths',
  a: 'assists',
  hs: 'hsPct',
  hsp: 'hsPct',
  hltv: 'rating',
  hltv2: 'rating',
  openingKills: 'openingDuel',
  openingDeaths: 'openingDuel',
  entry: 'openingDuel',
  trade: 'tradeKills',
  traded: 'tradedDeaths',
  clutches: 'clutch',
  clutchWins: 'clutch',
  heDamage: 'utilityDamage',
  fireDamage: 'utilityDamage',
  timeToDamage: 'ttd',
  reactionTime: 'reaction',
  crosshair: 'crosshairPlacement',
  cp: 'crosshairPlacement',
  firstBullet: 'firstBulletAccuracy',
  spotted: 'accuracySpotted',
  control: 'sprayControl',
  meanErr: 'sprayError',
  swingAvg: 'swing',
  roundSwing: 'swing',
  kastPct: 'kast',
  counterStrafeRate: 'counterStrafe',
  ttdMedianMs: 'ttd',
  reactionMedianMs: 'reaction',
  crosshairMedianDeg: 'crosshairPlacement',
  breakShotIndex: 'breakShot',
  onTarget: 'trackingOnTarget',
  successRate: 'strafeSuccess',
  avgTiming: 'strafeTiming',
};

export function getMetric(key: string): MetricDefinition | undefined {
  return METRIC_GLOSSARY[key] ?? METRIC_GLOSSARY[ALIASES[key] ?? ''];
}

export function listMetrics(category?: MetricCategory): MetricDefinition[] {
  return category ? list.filter((m) => m.category === category) : [...list];
}
