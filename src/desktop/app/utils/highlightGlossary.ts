// W3-A：集锦类型的名称和说明。口径和阈值与 crates/highlights/src/rules.rs、docs/highlights.md 一致，改一处就要改另外两处。
// 也登记进 metricGlossary（key = `highlight.<kind>`），所以可以直接 <MetricHelp :metric="`highlight.${kind}`" />。
import type { HighlightCategory, HighlightKind } from '@cs-analyst/shared-types';
import type { MetricDefinition } from './metricGlossary';

export interface HighlightKindMeta {
  kind: HighlightKind;
  category: HighlightCategory;
  /** 短名，用于标签和筛选按钮 */
  label: string;
  definition: string;
  calculation: string;
  caveat?: string;
}

const AIM_CAVEAT = '需要枪法分析的可见性数据；ESL 职业 demo 这类可见性不可靠的对局没有这一类。';
const SWING_CAVEAT = '用的是这一下让本方回合胜率变了多少（胜率曲线上的跳变），不是个人分到的 Swing。';

export const HIGHLIGHT_KINDS: HighlightKindMeta[] = [
  // ── 高光 ──
  {
    kind: 'multi_kill',
    category: 'highlight',
    label: '多杀',
    definition: '一回合内击杀 3 人及以上（3K / 4K / ACE）。',
    calculation: '同一回合击杀敌人的次数 ≥ 3，口径与计分板的多杀一致。分数：3K 50、4K 75、ACE 100，每个爆头 +2。',
  },
  {
    kind: 'quick_kills',
    category: 'highlight',
    label: '快速连杀',
    definition: '很短时间内连续击杀。',
    calculation: '相邻两次击杀间隔 ≤ 1.2 秒连成一串；两杀要在 0.8 秒内完成，三杀以上不限总时长。',
  },
  {
    kind: 'clutch_win',
    category: 'highlight',
    label: '残局获胜',
    definition: '本方只剩自己一人时赢下回合（1vN）。',
    calculation: '与残局统计口径一致：本方第一次只剩 1 人、对方还有 N 人，最终本方赢下这回合。从最后一名队友阵亡开始。分数随 N 增加：1v1 35 到 1v5 100。',
  },
  {
    kind: 'flick_headshot',
    category: 'highlight',
    label: '甩枪爆头',
    definition: '准星离得很远，瞬间甩过去一枪爆头。',
    calculation: '对方进入视野时准星离头部 ≥ 15°，500 ms 内造成第一次伤害，并在 0.5 秒内爆头击杀。',
    caveat: AIM_CAVEAT,
  },
  {
    kind: 'wallbang',
    category: 'highlight',
    label: '穿墙',
    definition: '子弹穿过墙体或物体击杀敌人。',
    calculation: '击杀事件的穿透标记（penetrated），只算枪械。',
  },
  {
    kind: 'smoke_kill',
    category: 'highlight',
    label: '穿烟',
    definition: '隔着烟雾击杀敌人。',
    calculation: '击杀事件的穿烟标记（thrusmoke），只算枪械。',
  },
  {
    kind: 'blind_kill',
    category: 'highlight',
    label: '盲杀',
    definition: '自己被闪白时完成击杀。',
    calculation: '击杀事件的「攻击者被致盲」标记，不含手雷类击杀。',
  },
  {
    kind: 'airborne_kill',
    category: 'highlight',
    label: '跳杀',
    definition: '人在空中时完成击杀。',
    calculation: '击杀事件的「攻击者在空中」标记，狙击枪跳狙额外加分。',
  },
  {
    kind: 'noscope_kill',
    category: 'highlight',
    label: '无镜狙',
    definition: '狙击枪不开镜击杀。',
    calculation: '击杀事件的 noscope 标记，只算 AWP、SSG 08、SCAR-20、G3SG1；距离越远分越高。',
  },
  {
    kind: 'swing_kill',
    category: 'highlight',
    label: '关键击杀',
    definition: '一次击杀让本方回合胜率大幅提高。',
    calculation: '胜率曲线上这次击杀让本方胜率提高 ≥ 25 个百分点，例如残局里的击杀、下包后的关键一枪。',
    caveat: SWING_CAVEAT,
  },
  {
    kind: 'key_defuse',
    category: 'highlight',
    label: '关键拆包',
    definition: '拆包时 T 还有人活着，或 C4 只剩 1 秒以内。',
    calculation: '拆包成功时 T 方仍有存活者（拆包者下包后有过击杀），或者拆完时 C4 剩余时间 ≤ 1 秒。',
  },
  {
    kind: 'ninja_defuse',
    category: 'highlight',
    label: '忍者拆包',
    definition: 'T 还活着的情况下偷偷把包拆了。',
    calculation: '拆包成功时 T 方仍有存活者，且拆包者从下包到拆完没有击杀任何人。',
  },
  // ── 失误 ──
  {
    kind: 'untraded_opening_death',
    category: 'mistake',
    label: '首死没被补',
    definition: '本回合第一个阵亡，而且队友没能在 5 秒内补掉凶手。',
    calculation: '开局对枪失败（本回合第一个敌对击杀的受害者），且 5 秒补枪窗口内凶手没有被击杀。这一回合输了额外加分。',
  },
  {
    kind: 'unused_utility_death',
    category: 'mistake',
    label: '死时道具没用',
    definition: '死的时候身上还有道具，这一回合一颗都没扔。',
    calculation: '本回合买了道具、一颗没扔就死了，道具价值 ≥ $200（取推算的身上未用道具和本回合购买额里较小的一个）。',
    caveat: '只看本回合买的道具；捡起和丢弃的道具看不到。',
  },
  {
    kind: 'team_flash',
    category: 'mistake',
    label: '闪到队友',
    definition: '一颗闪光把队友闪白了，而且闪到的敌人不如队友多。',
    calculation: '同一颗闪光（同一投掷者、致盲事件相差 ≤ 2 tick）让至少一名队友致盲 ≥ 1.5 秒，且致盲敌人的总时长不超过队友。',
    caveat: 'ESL 等职业服务器的 demo 没有致盲事件，这类对局没有这一类。',
  },
  {
    kind: 'team_damage',
    category: 'mistake',
    label: '队友伤害',
    definition: '打伤或误杀了队友。',
    calculation: '一回合内对队友造成的伤害合计 ≥ 25（含燃烧弹、手雷），或者误杀队友。',
  },
  {
    kind: 'lost_hp_duel',
    category: 'mistake',
    label: '血优输对枪',
    definition: '一对一对枪时自己血更多，却被对方打死。',
    calculation: '击杀前 4 秒内只有你们两人互相造成伤害（没有第三人参与、你没有被闪），交火开始时你比对方多 ≥ 40 血，你也开了枪，最后被对方击杀。',
  },
  {
    kind: 'whiffed_spray',
    category: 'mistake',
    label: '近距离扫空',
    definition: '近距离连发一枪没中，随后被打死。',
    calculation: '步枪 / 冲锋枪 / 机枪连发 ≥ 5 发、没有对敌人造成任何伤害，连发结束后 2 秒内被 12 米以内的敌人击杀。',
  },
  {
    kind: 'swing_loss',
    category: 'mistake',
    label: '大幅负 Swing',
    definition: '自己的一次死亡（或误杀、自杀）让本方回合胜率大幅下降。',
    calculation: '胜率曲线上这个事件让本方胜率下降 ≥ 25 个百分点，例如人数优势下送掉、残局最后一人阵亡。',
    caveat: SWING_CAVEAT,
  },
];

export const HIGHLIGHT_KIND_MAP: Readonly<Record<string, HighlightKindMeta>> = Object.freeze(
  Object.fromEntries(HIGHLIGHT_KINDS.map((k) => [k.kind, k])),
);

export function highlightLabel(kind: string): string {
  return HIGHLIGHT_KIND_MAP[kind]?.label ?? kind;
}

/** metricGlossary 里的条目：key = `highlight.<kind>`，另有 `highlight.score` */
export const HIGHLIGHT_METRICS: MetricDefinition[] = [
  ...HIGHLIGHT_KINDS.map(
    (k): MetricDefinition => ({
      key: `highlight.${k.kind}`,
      name: `${k.category === 'highlight' ? '高光' : '失误'} · ${k.label}`,
      category: 'highlight',
      definition: k.definition,
      calculation: k.calculation,
      caveat: k.caveat,
      source: 'docs/highlights.md',
    }),
  ),
  {
    key: 'highlight.score',
    name: '集锦分数',
    category: 'highlight',
    definition: '这一刻有多精彩（高光）或多严重（失误），用来排序。',
    calculation: '每条规则给出 0–100 的原始分（例如 ACE 100、3K 50；Swing 类按胜率变化的百分点换算），再乘以设置里这条规则的权重（默认 0.7–1.0，可调 0–3）。',
    higherIsBetter: true,
    source: 'docs/highlights.md',
  },
];
