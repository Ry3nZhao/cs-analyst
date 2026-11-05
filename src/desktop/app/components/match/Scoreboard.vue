<template>
  <div class="scoreboard">
    <SectionHeader title="计分板" size="lg">
      <div class="seg" role="tablist">
        <button
          v-for="t in TABS"
          :key="t.key"
          class="seg-item"
          :class="{ active: tab === t.key }"
          role="tab"
          :aria-selected="tab === t.key"
          @click="tab = t.key"
        >{{ t.label }}</button>
      </div>
    </SectionHeader>

    <div v-if="tab === 'utility' && !detail.blindDataAvailable" class="callout callout-info blind-note">
      这个 demo 没有致盲事件（常见于 ESL 服务器），致盲相关数据不可用。闪光助攻来自击杀事件，不受影响。
    </div>

    <div class="team-tables">
      <div v-for="t in detail.teams" :key="t.team" class="table-wrap team-table" :class="{ won: t.won === true }">
        <table class="data-table compact">
          <thead>
            <tr>
              <th class="col-player">
                <div class="team-head">
                  <span class="team-title">{{ teamLabel(t.team, mineTeam === t.team) }}</span>
                  <span class="team-score num" :class="{ win: t.won === true }">{{ t.score }}</span>
                  <SideBadge :side="t.startSide" />
                  <span class="muted start-hint">开局</span>
                </div>
              </th>
              <th
                v-for="c in columns"
                :key="c.key"
                class="num sortable"
                :class="{ sorted: sortKey === c.key }"
                :title="c.title ?? c.label"
                @click="toggleSort(c.key)"
              >
                {{ c.label }}<span class="sort-mark">{{ sortKey === c.key ? (sortDesc ? '↓' : '↑') : '' }}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="p in sortedPlayers(t.team)" :key="p.steamid" :class="{ selected: mine.has(p.steamid) }">
              <td class="col-player strong">
                <NuxtLink :to="`/players/${p.steamid}`" class="player-link" :title="p.steamid">{{ p.name || p.steamid }}</NuxtLink>
                <span v-if="mine.has(p.steamid)" class="tag me-tag">我</span>
              </td>
              <td
                v-for="c in columns"
                :key="c.key"
                class="num"
                :class="cellClass(c, p)"
                :title="c.hint?.(p)"
              >{{ display(c, p) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { MatchDetailDto, PlayerMatchStatsDto, SteamId, TeamId } from '@cs-analyst/shared-types';
import { fmtMoney, teamLabel } from '~/composables/useMatchDetail';

const props = defineProps<{
  detail: MatchDetailDto;
  mine: Set<SteamId>;
  mineTeam: TeamId | null;
}>();

type TabKey = 'basic' | 'duels' | 'utility' | 'economy';
const TABS: { key: TabKey; label: string }[] = [
  { key: 'basic', label: '基础' },
  { key: 'duels', label: '对枪 / 补枪 / 残局' },
  { key: 'utility', label: '道具' },
  { key: 'economy', label: '经济' },
];

/** 一列：value 返回 null 表示没有可比的数（显示破折号），unavailable 返回 true 显示「不可用」。 */
interface Col {
  key: string;
  label: string;
  title?: string;
  value: (p: PlayerMatchStatsDto) => number | null;
  text?: (p: PlayerMatchStatsDto) => string;
  digits?: number;
  unavailable?: (p: PlayerMatchStatsDto) => boolean;
  /** 单元格悬停提示。 */
  hint?: (p: PlayerMatchStatsDto) => string;
  /** 正负着色（例如 +/-）。 */
  signed?: boolean;
}

const tab = ref<TabKey>('basic');
const sortKey = ref<string>('rating');
const sortDesc = ref(true);

const DEFAULT_SORT: Record<TabKey, string> = { basic: 'rating', duels: 'openK', utility: 'utilDmg', economy: 'equip' };
watch(tab, (t) => { sortKey.value = DEFAULT_SORT[t]; sortDesc.value = true; });

const sum = (a: number[] | undefined, from: number) => (a ?? []).slice(from).reduce((s, v) => s + v, 0);
const pct = (n: number, d: number) => (d > 0 ? (n / d) * 100 : null);
const frac = (n: number, d: number) => `${n}/${d}`;

// 经济页需要的逐回合汇总
const econ = computed(() => {
  const buy = new Map<string, string>();
  for (const e of props.detail.economy) buy.set(`${e.round}:${e.team}`, e.buyType);
  const m = new Map<SteamId, { rounds: number; equip: number; balance: number; damage: number; fullK: number; fullR: number; lowK: number; lowR: number }>();
  for (const r of props.detail.playerRounds) {
    const s = m.get(r.steamid) ?? { rounds: 0, equip: 0, balance: 0, damage: 0, fullK: 0, fullR: 0, lowK: 0, lowR: 0 };
    s.rounds += 1;
    s.equip += r.equipValue;
    s.balance += r.balance;
    s.damage += r.damage;
    const b = buy.get(`${r.round}:${r.team}`);
    if (b === 'full') { s.fullK += r.kills; s.fullR += 1; }
    else if (b === 'eco' || b === 'force') { s.lowK += r.kills; s.lowR += 1; }
    m.set(r.steamid, s);
  }
  return m;
});

const BASIC: Col[] = [
  { key: 'k', label: 'K', title: '击杀', value: p => p.kills },
  { key: 'd', label: 'D', title: '死亡', value: p => p.deaths },
  { key: 'a', label: 'A', title: '助攻', value: p => p.assists },
  { key: 'diff', label: '+/-', title: '击杀 − 死亡', value: p => p.kills - p.deaths, signed: true, text: p => { const v = p.kills - p.deaths; return v > 0 ? `+${v}` : String(v); } },
  { key: 'adr', label: 'ADR', title: '场均每回合伤害', value: p => p.derived.adr, digits: 1 },
  { key: 'kast', label: 'KAST', title: '有击杀 / 助攻 / 存活 / 被补枪的回合比例', value: p => p.derived.kast, digits: 0, text: p => `${p.derived.kast.toFixed(0)}%` },
  { key: 'hs', label: 'HS%', title: '爆头击杀比例', value: p => p.derived.hsPct, text: p => `${p.derived.hsPct.toFixed(0)}%` },
  { key: 'rating', label: 'Rating', title: 'HLTV Rating 2.0（公开近似公式，仅供参考）', value: p => p.derived.rating, digits: 2 },
  { key: 'k2', label: '2K', value: p => p.multiKills[2] ?? 0 },
  { key: 'k3', label: '3K', value: p => p.multiKills[3] ?? 0 },
  { key: 'k4', label: '4K', value: p => p.multiKills[4] ?? 0 },
  { key: 'k5', label: '5K', value: p => p.multiKills[5] ?? 0 },
  { key: 'mvp', label: 'MVP', title: '按引擎规则近似', value: p => p.mvps },
];

const DUELS: Col[] = [
  { key: 'openK', label: '首杀', title: '开局对枪获胜（T + CT）', value: p => p.openingKillsT + p.openingKillsCt },
  { key: 'openD', label: '首死', title: '开局对枪阵亡（T + CT）', value: p => p.openingDeathsT + p.openingDeathsCt },
  { key: 'openPct', label: '首杀率', title: '开局对枪成功率', value: p => (p.derived.openingAttempts > 0 ? p.derived.openingSuccessPct : null), text: p => `${p.derived.openingSuccessPct.toFixed(0)}%` },
  { key: 'openT', label: 'T 首杀', title: 'T 方开局 击杀/尝试', value: p => p.openingKillsT, text: p => frac(p.openingKillsT, p.openingKillsT + p.openingDeathsT) },
  { key: 'openCt', label: 'CT 首杀', title: 'CT 方开局 击杀/尝试', value: p => p.openingKillsCt, text: p => frac(p.openingKillsCt, p.openingKillsCt + p.openingDeathsCt) },
  { key: 'trade', label: '补枪', title: '补枪成功 / 补枪机会（5 秒内）', value: p => p.tradeKills, text: p => frac(p.tradeKills, p.tradeKillOpportunities) },
  { key: 'tradePct', label: '补枪率', value: p => pct(p.tradeKills, p.tradeKillOpportunities), text: p => `${p.derived.tradeKillPct.toFixed(0)}%` },
  { key: 'traded', label: '被补', title: '阵亡后 5 秒内被队友补回 / 有队友存活的阵亡', value: p => p.tradedDeaths, text: p => frac(p.tradedDeaths, p.tradedDeathOpportunities) },
  { key: 'tradedPct', label: '被补率', value: p => pct(p.tradedDeaths, p.tradedDeathOpportunities), text: p => `${p.derived.tradedDeathPct.toFixed(0)}%` },
  { key: 'clutch', label: '残局', title: '1vN 胜 / 尝试（全部 N）', value: p => sum(p.clutchWins, 1), text: p => frac(sum(p.clutchWins, 1), sum(p.clutchAttempts, 1)) },
  { key: 'c1', label: '1v1', value: p => p.clutchWins[1] ?? 0, text: p => frac(p.clutchWins[1] ?? 0, p.clutchAttempts[1] ?? 0) },
  { key: 'c2', label: '1v2', value: p => p.clutchWins[2] ?? 0, text: p => frac(p.clutchWins[2] ?? 0, p.clutchAttempts[2] ?? 0) },
  { key: 'c3', label: '1v3+', value: p => sum(p.clutchWins, 3), text: p => frac(sum(p.clutchWins, 3), sum(p.clutchAttempts, 3)) },
  { key: 'clutchK', label: '残局击杀', value: p => p.clutchKills },
];

const noBlind = (p: PlayerMatchStatsDto) => p.enemiesFlashed == null;
const UTILITY: Col[] = [
  { key: 'flash', label: '闪光', title: '投掷闪光弹数', value: p => p.flashesThrown },
  { key: 'he', label: 'HE', value: p => p.heThrown },
  { key: 'smoke', label: '烟雾', value: p => p.smokesThrown },
  { key: 'molo', label: '燃烧', value: p => p.molotovsThrown },
  { key: 'enemyPerFlash', label: '致盲/闪', title: '每颗闪光平均致盲敌人数', value: p => p.derived.enemiesFlashedPerFlash, digits: 2, unavailable: noBlind },
  { key: 'blindSecs', label: '时长/闪', title: '每颗闪光平均致盲敌人总秒数', value: p => p.derived.enemyBlindSecondsPerFlash, digits: 2, unavailable: noBlind },
  { key: 'teamFlash', label: '闪队友/闪', title: '每颗闪光平均闪到的队友数', value: p => p.derived.teammatesFlashedPerFlash, digits: 2, unavailable: noBlind },
  { key: 'flashA', label: '闪光助攻', value: p => p.flashAssists },
  { key: 'heDmg', label: 'HE 伤害', value: p => p.heDamage },
  { key: 'fireDmg', label: '燃烧伤害', value: p => p.fireDamage },
  { key: 'utilDmg', label: '道具伤害', title: 'HE + 燃烧对敌人的伤害', value: p => p.derived.utilityDamage },
  { key: 'utilPr', label: '投掷/回合', value: p => p.derived.utilityThrownPerRound, digits: 2 },
  { key: 'unused', label: '死亡浪费', title: '死亡时手里没用掉的道具价值（累计）', value: p => p.unusedUtilityValue, text: p => fmtMoney(p.unusedUtilityValue) },
];

const avg = (n: number, d: number) => (d > 0 ? n / d : null);
const e = (p: PlayerMatchStatsDto) => econ.value.get(p.steamid);
const ECONOMY: Col[] = [
  { key: 'equip', label: '平均装备', title: '冻结结束时的装备价值 + 之后的购买', value: p => avg(e(p)?.equip ?? 0, e(p)?.rounds ?? 0), text: p => fmtMoney(avg(e(p)?.equip ?? 0, e(p)?.rounds ?? 0)) },
  { key: 'bal', label: '平均余额', title: '冻结结束时的余额', value: p => avg(e(p)?.balance ?? 0, e(p)?.rounds ?? 0), text: p => fmtMoney(avg(e(p)?.balance ?? 0, e(p)?.rounds ?? 0)) },
  { key: 'dmgPerK', label: '伤害/千元', title: '总伤害 ÷ 总装备价值（千元）', value: p => { const s = e(p); return s && s.equip > 0 ? s.damage / (s.equip / 1000) : null; }, digits: 1 },
  { key: 'fullK', label: '全起 KPR', title: '本队全起回合的每回合击杀（悬停看击杀 / 回合）', value: p => avg(e(p)?.fullK ?? 0, e(p)?.fullR ?? 0), digits: 2, hint: p => `${e(p)?.fullK ?? 0} 杀 / ${e(p)?.fullR ?? 0} 回合` },
  { key: 'lowK', label: 'ECO/强起 KPR', title: '本队 ECO 或强起回合的每回合击杀（悬停看击杀 / 回合）', value: p => avg(e(p)?.lowK ?? 0, e(p)?.lowR ?? 0), digits: 2, hint: p => `${e(p)?.lowK ?? 0} 杀 / ${e(p)?.lowR ?? 0} 回合` },
  { key: 'unusedE', label: '死亡浪费', title: '死亡时手里没用掉的道具价值（累计）', value: p => p.unusedUtilityValue, text: p => fmtMoney(p.unusedUtilityValue) },
];

const columns = computed(() => ({ basic: BASIC, duels: DUELS, utility: UTILITY, economy: ECONOMY })[tab.value]);

function toggleSort(key: string) {
  if (sortKey.value === key) sortDesc.value = !sortDesc.value;
  else { sortKey.value = key; sortDesc.value = true; }
}

function sortedPlayers(team: TeamId) {
  const col = columns.value.find(c => c.key === sortKey.value);
  const list = props.detail.scoreboard.filter(p => p.team === team);
  if (!col) return list;
  const dir = sortDesc.value ? -1 : 1;
  return [...list].sort((a, b) => {
    const va = col.unavailable?.(a) ? null : col.value(a);
    const vb = col.unavailable?.(b) ? null : col.value(b);
    if (va == null && vb == null) return 0;
    if (va == null) return 1; // 缺失值始终排在最后
    if (vb == null) return -1;
    return (va - vb) * dir;
  });
}

function display(c: Col, p: PlayerMatchStatsDto) {
  if (c.unavailable?.(p)) return '不可用';
  const v = c.value(p);
  if (c.text) return v == null ? '—' : c.text(p);
  if (v == null) return '—';
  return c.digits != null ? v.toFixed(c.digits) : String(v);
}

function cellClass(c: Col, p: PlayerMatchStatsDto) {
  if (c.unavailable?.(p)) return 'dim unavail';
  const v = c.value(p);
  if (v == null) return 'dim';
  if (c.signed) return v > 0 ? 'good' : v < 0 ? 'bad' : 'muted';
  if (c.key === sortKey.value) return 'strong';
  // 0 或 0/0 这类「没发生」的格子降一级，让有数的格子跳出来
  return v === 0 ? 'dim' : '';
}
</script>

<style scoped>
.scoreboard { display: flex; flex-direction: column; }
.blind-note { margin-bottom: var(--space-3); }

.seg { display: inline-flex; padding: 2px; gap: 2px; background: var(--bg-sunken); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); }
.seg-item {
  height: 24px; padding: 0 var(--space-3);
  border: 0; border-radius: var(--radius-sm); background: transparent;
  font-size: var(--text-sm); color: var(--text-3); cursor: pointer;
}
.seg-item:hover { color: var(--text); }
.seg-item.active { background: var(--surface-raised); color: var(--text); box-shadow: inset 0 -2px 0 var(--accent); }

.team-tables { display: flex; flex-direction: column; gap: var(--space-3); }
.team-table { overflow-x: auto; }
.col-player { width: 220px; min-width: 180px; text-align: left; }
.team-head { display: flex; align-items: center; gap: var(--space-2); text-transform: none; letter-spacing: 0; }
.team-title { color: var(--text); font-weight: 600; font-size: var(--text-sm); }
.team-score { font-size: var(--text-md); font-weight: 600; color: var(--text-3); }
.team-score.win { color: var(--accent); }
.start-hint { font-size: var(--text-xs); font-weight: 400; }

th.sortable { cursor: pointer; user-select: none; white-space: nowrap; }
th.sortable:hover { color: var(--text); }
th.sorted { color: var(--text); }
.sort-mark { display: inline-block; width: 10px; text-align: left; }

.player-link { color: inherit; text-decoration: none; }
.player-link:hover { text-decoration: underline; text-underline-offset: 2px; }
.me-tag { margin-left: var(--space-2); height: 16px; padding: 0 5px; font-size: 10px; color: var(--accent); border-color: var(--accent-line); }
td.unavail { font-family: var(--font-sans); font-size: var(--text-xs); }
td.strong { color: var(--text); }
td.dim { color: var(--text-disabled); }
td.muted { color: var(--text-3); }
td.good { color: var(--value-good); }
td.bad { color: var(--value-bad); }
td { font-family: var(--font-mono); }
td.col-player { font-family: var(--font-sans); }
</style>
