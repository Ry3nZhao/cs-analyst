<template>
  <div class="econ-chart">
    <SectionHeader title="经济曲线" description="每回合的团队装备价值（冻结结束时装备 + 之后的购买）。实心点 = 该回合获胜" size="lg">
      <div class="legend">
        <span v-for="s in legend" :key="s.team" class="legend-item">
          <svg width="22" height="8"><line x1="1" y1="4" x2="21" y2="4" :class="s.cls" /></svg>
          {{ teamLabel(s.team, mineTeam === s.team) }}
        </span>
      </div>
    </SectionHeader>

    <div ref="box" class="plot" @mouseleave="hover = null">
      <svg v-if="width > 0" :width="width" :height="H" role="img" aria-label="两队每回合装备价值折线图">
        <!-- 网格 + y 轴 -->
        <g class="grid">
          <template v-for="t in yTicks" :key="t">
            <line :x1="M.l" :x2="width - M.r" :y1="y(t)" :y2="y(t)" />
            <text :x="M.l - 8" :y="y(t) + 4" text-anchor="end">{{ fmtMoneyK(t) }}</text>
          </template>
        </g>
        <!-- 换边 -->
        <g class="half">
          <template v-for="h in halves" :key="h">
            <line :x1="h" :x2="h" :y1="M.t" :y2="H - M.b" />
          </template>
        </g>
        <!-- x 轴 -->
        <g class="xaxis">
          <text v-for="r in xLabels" :key="r" :x="x(r)" :y="H - 8" text-anchor="middle">{{ r }}</text>
        </g>
        <!-- 悬停竖线 -->
        <line v-if="hover != null" class="crosshair" :x1="x(hover)" :x2="x(hover)" :y1="M.t" :y2="H - M.b" />
        <!-- 折线 -->
        <g v-for="s in series" :key="s.team">
          <path :d="s.path" :class="['line', s.cls]" />
          <circle
            v-for="p in s.points"
            :key="p.round"
            :cx="x(p.round)"
            :cy="y(p.equipValue)"
            :r="hover === p.round ? 4.5 : 3"
            :class="['pt', s.cls, { won: p.won }]"
          />
        </g>
        <!-- 命中区 -->
        <rect
          v-for="r in rounds"
          :key="r"
          class="hit"
          :x="x(r) - step / 2"
          :y="M.t"
          :width="step"
          :height="H - M.t - M.b"
          @mouseenter="hover = r"
        />
      </svg>

      <div v-if="hover != null && tip" class="tooltip" :style="tipStyle">
        <div class="tip-title">第 {{ hover }} 回合</div>
        <div v-for="row in tip" :key="row.team" class="tip-row">
          <svg width="14" height="8"><line x1="1" y1="4" x2="13" y2="4" :class="row.cls" /></svg>
          <span class="tip-team">{{ teamLabel(row.team, mineTeam === row.team) }}</span>
          <span class="num">{{ fmtMoney(row.equipValue) }}</span>
          <span class="muted">{{ BUY_LABEL[row.buyType] }}</span>
          <span v-if="row.won" class="tip-won">胜</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { MatchDetailDto, TeamId, TeamRoundStatsDto } from '@cs-analyst/shared-types';
import { BUY_LABEL, fmtMoney, fmtMoneyK, teamLabel } from '~/composables/useMatchDetail';

const props = defineProps<{ detail: MatchDetailDto; mineTeam: TeamId | null }>();

const H = 220;
const M = { l: 52, r: 16, t: 12, b: 26 };

const box = ref<HTMLElement | null>(null);
const width = ref(0);
const hover = ref<number | null>(null);
let ro: ResizeObserver | null = null;
onMounted(() => {
  if (!box.value) return;
  width.value = box.value.clientWidth;
  ro = new ResizeObserver(([e]) => { width.value = Math.floor(e!.contentRect.width); });
  ro.observe(box.value);
});
onBeforeUnmount(() => ro?.disconnect());

const rounds = computed(() => props.detail.rounds.map(r => r.number));
const n = computed(() => Math.max(1, rounds.value.length));
const step = computed(() => (width.value - M.l - M.r) / n.value);
const x = (r: number) => M.l + step.value * (r - rounds.value[0]! + 0.5);

const maxV = computed(() => {
  const m = Math.max(1000, ...props.detail.economy.map(e => e.equipValue));
  const unit = m > 20000 ? 10000 : 5000;
  return Math.ceil(m / unit) * unit;
});
const y = (v: number) => H - M.b - (v / maxV.value) * (H - M.t - M.b);
const yTicks = computed(() => {
  const c = maxV.value > 20000 ? 10000 : 5000;
  const out: number[] = [];
  for (let v = 0; v <= maxV.value; v += c) out.push(v);
  return out;
});
const xLabels = computed(() => rounds.value.filter(r => r === 1 || r % 3 === 0 || n.value <= 16));

const halves = computed(() => {
  const out: number[] = [];
  const rs = props.detail.rounds;
  for (let i = 1; i < rs.length; i++) if (rs[i]!.sideA !== rs[i - 1]!.sideA) out.push((x(rs[i - 1]!.number) + x(rs[i]!.number)) / 2);
  return out;
});

// 「我方」（或 A 队）用实线亮色，另一队用虚线灰色；不靠颜色区分队伍。
const primary = computed<TeamId>(() => props.mineTeam ?? 'A');
// 次要队伍先画，主队伍压在上面
const series = computed(() => ((primary.value === 'A' ? ['B', 'A'] : ['A', 'B']) as TeamId[])
  .map((team) => {
    const points = props.detail.economy.filter(e => e.team === team).sort((a, b) => a.round - b.round);
    const path = points.map((p, i) => `${i ? 'L' : 'M'}${x(p.round).toFixed(1)},${y(p.equipValue).toFixed(1)}`).join('');
    return { team, points, path, cls: team === primary.value ? 's-primary' : 's-secondary' };
  }));
const legend = computed(() => [...series.value].reverse());

const tip = computed(() => {
  if (hover.value == null) return null;
  return legend.value
    .map(s => ({ ...(s.points.find(p => p.round === hover.value) as TeamRoundStatsDto | undefined), cls: s.cls, team: s.team }))
    .filter((r): r is TeamRoundStatsDto & { cls: string } => r.equipValue != null);
});
const tipStyle = computed(() => {
  if (hover.value == null) return {};
  const px = x(hover.value);
  const right = px > width.value * 0.6;
  return right ? { right: `${width.value - px + 12}px`, top: '8px' } : { left: `${px + 12}px`, top: '8px' };
});
</script>

<style scoped>
.legend { display: flex; gap: var(--space-4); font-size: var(--text-xs); color: var(--text-2); }
.legend-item { display: inline-flex; align-items: center; gap: 6px; }

.plot { position: relative; width: 100%; }
svg { display: block; overflow: visible; }

.grid line { stroke: var(--border-subtle); stroke-width: 1; }
.grid text, .xaxis text { fill: var(--text-3); font-size: 10px; font-family: var(--font-mono); }
.half line { stroke: var(--border-strong); stroke-dasharray: 3 3; }
.crosshair { stroke: var(--border-strong); stroke-width: 1; }

.line { fill: none; stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
.s-primary { stroke: var(--text); }
.s-secondary { stroke: var(--text-3); stroke-dasharray: 5 3; }
line.s-primary, line.s-secondary { stroke-width: 2; }
.pt { stroke-width: 1.5; fill: var(--surface); stroke-dasharray: none; }
.pt.s-primary.won { fill: var(--text); }
.pt.s-secondary.won { fill: var(--text-3); }
.hit { fill: transparent; cursor: crosshair; }

.tooltip {
  position: absolute; z-index: 2; pointer-events: none; min-width: 200px;
  padding: var(--space-2) var(--space-3);
  background: var(--surface-raised); border: 1px solid var(--border); border-radius: var(--radius-md);
  box-shadow: var(--shadow-pop); font-size: var(--text-xs);
}
.tip-title { color: var(--text); font-weight: 600; margin-bottom: 4px; }
.tip-row { display: flex; align-items: center; gap: var(--space-2); height: 20px; color: var(--text-2); }
.tip-team { min-width: 36px; }
.tip-row .num { margin-left: auto; color: var(--text); }
.tip-won { color: var(--accent); }
</style>
