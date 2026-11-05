<template>
  <div class="winprob">
    <SectionHeader title="回合胜率" :description="headerDesc" size="lg">
      <MetricHelp metric="swing" label side="left" />
    </SectionHeader>

    <div v-if="loading" class="state-msg muted">加载中…</div>
    <div v-else-if="!data || data.rounds.length === 0" class="state-msg muted">
      不可用<span v-if="error" class="err">：{{ error }}</span>
    </div>

    <template v-else>
      <div v-if="!mineTeam" class="callout callout-info persp-note">
        未设置主玩家，不知道哪边是「我方」：下面的胜率一律按 <SideBadge :side="2" /> T 方显示。
      </div>

      <!-- 整场概览 + 回合选择：柱高 = 回合开始时我方胜率，下方色块 = 胜方阵营 -->
      <div class="strip-wrap">
        <div class="strip">
          <div class="labels">
            <div class="cell-h">回合</div>
            <div class="bar-cell lbl">开局{{ perspWord }}</div>
            <div class="res-cell lbl">胜方</div>
          </div>
          <button
            v-for="(r, i) in data.rounds"
            :key="r.round"
            class="round-col"
            :class="{ active: selected === r.round, half: i > 0 && sideAOf(data.rounds[i - 1]!.round) !== sideAOf(r.round) }"
            :title="`第 ${r.round} 回合 · 开局${perspWord}胜率 ${pct(mineP(r.round, r.pStart))} · ${r.winnerSide ? (r.winnerSide === 2 ? 'T' : 'CT') + ' 胜' : '无胜方'}`"
            @click="selected = r.round"
          >
            <div class="cell-h num">{{ r.round }}</div>
            <div class="bar-cell">
              <div class="bar-track">
                <div class="bar-mid" />
                <div class="bar-fill" :style="{ height: `${mineP(r.round, r.pStart) * 100}%` }" />
              </div>
            </div>
            <div class="res-cell">
              <span class="res-dot" :class="r.winnerSide === 2 ? 'win-t' : r.winnerSide === 3 ? 'win-ct' : 'win-none'" />
            </div>
          </button>
        </div>
      </div>

      <div v-if="round" class="round-body">
        <div class="rd-head">
          <span class="rd-title">第 {{ round.round }} 回合</span>
          <span v-if="mineTeam" class="rd-item">
            我方本回合为 <SideBadge :side="perspSide(round.round)" />
          </span>
          <span class="rd-item muted num">开局 {{ pct(mineP(round.round, round.pStart)) }}</span>
          <span class="rd-item">
            <SideBadge :side="round.winnerSide" variant="solid" />
            {{ round.winnerSide ? (round.winnerSide === perspSide(round.round) ? `${perspWord}获胜` : `${perspWord}落败`) : '无胜方' }}
          </span>
          <span v-if="plantedAt != null" class="rd-item muted num">{{ fmtClock(plantedAt) }} 下包</span>
        </div>

        <!-- 胜率阶梯曲线 -->
        <div ref="box" class="plot" @mouseleave="hover = null">
          <svg v-if="width > 0" :width="width" :height="H" role="img" :aria-label="`第 ${round.round} 回合${perspWord}胜率曲线`">
            <g class="grid">
              <template v-for="t in Y_TICKS" :key="t">
                <line :x1="M.l" :x2="width - M.r" :y1="y(t)" :y2="y(t)" :class="{ mid: t === 0.5 }" />
                <text :x="M.l - 8" :y="y(t) + 4" text-anchor="end">{{ Math.round(t * 100) }}%</text>
              </template>
            </g>
            <g class="xaxis">
              <text v-for="t in xTicks" :key="t" :x="x(t)" :y="H - 8" text-anchor="middle">{{ fmtClock(t) }}</text>
            </g>
            <!-- 下包竖线 -->
            <g v-if="plantedAt != null" class="plant">
              <line :x1="x(plantedAt)" :x2="x(plantedAt)" :y1="M.t" :y2="H - M.b" />
              <text :x="x(plantedAt) + 4" :y="M.t + 10">下包</text>
            </g>
            <!-- 悬停竖线 -->
            <line v-if="hoverPt" class="crosshair" :x1="x(hoverPt.timeSecs)" :x2="x(hoverPt.timeSecs)" :y1="M.t" :y2="H - M.b" />
            <path :d="curvePath" class="curve" />
            <!-- 事件点 -->
            <circle
              v-for="(e, i) in events"
              :key="i"
              :cx="x(e.timeSecs)"
              :cy="y(e.after)"
              :r="(hover === i ? 1.5 : 0) + (e.kind === 'damage' ? 2 : 3.5)"
              :class="['pt', `k-${e.kind}`, e.actorSide === 2 ? 's-t' : e.actorSide === 3 ? 's-ct' : 's-none']"
            />
            <!-- 命中区 -->
            <circle
              v-for="(e, i) in events"
              :key="`h${i}`"
              class="hit"
              :cx="x(e.timeSecs)"
              :cy="y(e.after)"
              r="9"
              @mouseenter="hover = i"
            />
          </svg>

          <div v-if="hoverPt" class="tooltip" :style="tipStyle">
            <div class="tip-title"><span class="num">{{ fmtClock(hoverPt.timeSecs) }}</span> · {{ hoverPt.label }}</div>
            <div class="tip-row">
              <span class="muted">{{ perspWord }}胜率</span>
              <span class="num">{{ pct(hoverPt.before) }} → {{ pct(hoverPt.after) }}</span>
              <span class="num" :class="toneOf(hoverPt.delta)">{{ fmtPP(hoverPt.delta) }}</span>
            </div>
            <div class="tip-row muted num">
              存活 T {{ hoverPt.tAlive }} : CT {{ hoverPt.ctAlive }}<span v-if="hoverPt.bombPlanted"> · 已下包</span>
            </div>
          </div>
        </div>

        <div class="lists">
          <!-- 本回合摆动最大的事件 -->
          <div class="list-block">
            <div class="list-title">摆动最大的事件</div>
            <table class="data-table compact">
              <thead>
                <tr>
                  <th class="num col-time">时间</th>
                  <th>事件</th>
                  <th class="num">{{ perspWord }}胜率</th>
                  <th class="num">变化</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(e, i) in topEvents" :key="i">
                  <td class="num muted col-time">{{ fmtClock(e.timeSecs) }}</td>
                  <td>
                    <span class="who">
                      <SideBadge v-if="e.actorSide" :side="e.actorSide" variant="bar" />
                      {{ e.label }}
                    </span>
                  </td>
                  <td class="num muted">{{ pct(e.before) }} → {{ pct(e.after) }}</td>
                  <td class="num" :class="toneOf(e.delta)">{{ fmtPP(e.delta) }}</td>
                </tr>
                <tr v-if="topEvents.length === 0"><td colspan="4" class="empty">本回合没有事件</td></tr>
              </tbody>
            </table>
          </div>

          <!-- 本回合每人 Swing -->
          <div class="list-block">
            <div class="list-title">本回合每人 Swing（百分点）</div>
            <table class="data-table compact">
              <thead>
                <tr>
                  <th>选手</th>
                  <th class="num">Swing</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="p in roundPlayers" :key="p.steamid" :class="{ selected: mine.has(p.steamid) }">
                  <td>
                    <span class="who">
                      <SideBadge :side="p.side" variant="bar" />
                      <NuxtLink :to="`/players/${p.steamid}`" class="plink">{{ nameOf(p.steamid) }}</NuxtLink>
                      <span v-if="mineTeam === p.team" class="muted team-hint">我方</span>
                    </span>
                  </td>
                  <td class="num" :class="toneOf(p.swing)">{{ fmtPP(p.swing) }}</td>
                </tr>
                <tr v-if="roundPlayers.length === 0"><td colspan="2" class="empty">不可用</td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
// 回合胜率曲线（W2-B 胜率模型）：口径见 docs/stats.md「Swing」。
// p 一律是 T 方胜率；这里按「我方」视角换算（不知道我方时用 T 方）。
import type { MatchDetailDto, MatchWinProbabilityDto, Side, SteamId, TeamId, WinProbEventKind, WinProbPointDto } from '@cs-analyst/shared-types';
import { fetchWinProbability, fmtClock, sideOf, useMySteamIds } from '~/composables/useMatchDetail';

const props = defineProps<{ detail: MatchDetailDto; mineTeam: TeamId | null }>();
const mine = useMySteamIds();

const data = ref<MatchWinProbabilityDto | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const selected = ref<number | null>(null);

async function load() {
  loading.value = true;
  error.value = null;
  try {
    data.value = await fetchWinProbability(props.detail.matchId);
  } catch (e) {
    data.value = null;
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
  // 默认第 1 回合（或选中的回合已不存在时回到第一个）
  const rs = data.value?.rounds ?? [];
  if (!rs.some(r => r.round === selected.value)) selected.value = rs[0]?.round ?? null;
}
watch(() => props.detail.matchId, load, { immediate: true });

// ── 视角 ──
const perspWord = computed(() => (props.mineTeam ? '我方' : 'T 方'));
const headerDesc = computed(() => {
  const m = data.value?.model;
  const base = '每个事件前后的回合胜率，Swing 是这部分变化分到每个人头上的值。点击回合切换';
  if (m && !m.usesLocalData) return `${base}。本地库只有 ${m.localRounds} 个回合，暂用内置先验模型`;
  return base;
});
const roundSideA = computed(() => new Map(props.detail.rounds.map(r => [r.number, r.sideA])));
function sideAOf(round: number): Side | undefined { return roundSideA.value.get(round); }
/** 本回合「我方」所在阵营；不知道我方时为 T。 */
function perspSide(round: number): Side {
  const sa = sideAOf(round);
  if (!props.mineTeam || sa == null) return 2;
  return sideOf(props.mineTeam, sa);
}
/** T 方胜率 → 视角方胜率。 */
function mineP(round: number, pT: number) { return perspSide(round) === 2 ? pT : 1 - pT; }

// ── 格式化 ──
const pct = (p: number) => `${Math.round(p * 100)}%`;
function fmtPP(v: number) {
  const s = Math.abs(v).toFixed(1);
  if (s === '0.0') return '0.0';
  return v > 0 ? `+${s}` : `−${s}`;
}
function toneOf(v: number) {
  if (Math.abs(v) < 0.05) return 'muted';
  return v > 0 ? 'good' : 'bad';
}
const names = computed(() => new Map(props.detail.scoreboard.map(p => [p.steamid, p.name || p.steamid])));
function nameOf(id: SteamId | null) { return id ? (names.value.get(id) ?? id) : '—'; }

// ── 选中回合 ──
const round = computed(() => data.value?.rounds.find(r => r.round === selected.value) ?? null);
/** 本回合每人的阵营（来自 playerRounds）。 */
const sideOfPlayer = computed(() => {
  const m = new Map<SteamId, Side>();
  for (const p of data.value?.playerRounds ?? []) if (p.round === selected.value) m.set(p.steamid, p.side);
  return m;
});

function eventLabel(e: WinProbPointDto): string {
  const a = nameOf(e.player);
  const v = nameOf(e.victim);
  const k: WinProbEventKind = e.kind;
  switch (k) {
    case 'start': return '回合开始';
    case 'damage': return `${a} 伤害 ${v}`;
    case 'kill': return `${a} 击杀 ${v}`;
    case 'team_kill': return `${a} 误杀队友 ${v}`;
    case 'suicide': return `${v} 自杀`;
    case 'world': return `${v} 死亡（环境）`;
    case 'planted': return `${a} 下包`;
    case 'defused': return `${a} 拆除炸弹`;
    case 'exploded': return '炸弹爆炸';
    case 'end': return '回合结束';
    default: return String(k);
  }
}

/** 视角换算后的事件点。delta 单位为百分点。 */
const events = computed(() => {
  const r = round.value;
  if (!r) return [];
  return r.points.map((pt) => {
    const before = mineP(r.round, pt.pBefore);
    const after = mineP(r.round, pt.pAfter);
    const actor = pt.player ?? (pt.kind === 'suicide' || pt.kind === 'world' ? pt.victim : null);
    return {
      ...pt,
      before,
      after,
      delta: (after - before) * 100,
      label: eventLabel(pt),
      actorSide: actor ? (sideOfPlayer.value.get(actor) ?? null) : null,
    };
  });
});

const plantedAt = computed(() => round.value?.points.find(p => p.kind === 'planted')?.timeSecs ?? null);

const topEvents = computed(() => events.value
  .filter(e => e.kind !== 'start' && e.kind !== 'end')
  .slice()
  .sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta))
  .slice(0, 5));

const roundPlayers = computed(() => (data.value?.playerRounds ?? [])
  .filter(p => p.round === selected.value)
  .slice()
  .sort((a, b) => {
    // 我方（或 T 方）在前，组内按 Swing 从高到低
    const ga = props.mineTeam ? (a.team === props.mineTeam ? 0 : 1) : (a.side === 2 ? 0 : 1);
    const gb = props.mineTeam ? (b.team === props.mineTeam ? 0 : 1) : (b.side === 2 ? 0 : 1);
    return ga - gb || b.swing - a.swing;
  }));

// ── 曲线 ──
const H = 220;
const M = { l: 44, r: 16, t: 12, b: 26 };
const Y_TICKS = [0, 0.25, 0.5, 0.75, 1];

const box = ref<HTMLElement | null>(null);
const width = ref(0);
const hover = ref<number | null>(null);
let ro: ResizeObserver | null = null;
// 曲线容器在数据加载后才出现，所以等 box 出现再挂 ResizeObserver
watch(box, (el) => {
  ro?.disconnect();
  ro = null;
  if (!el) return;
  width.value = el.clientWidth;
  ro = new ResizeObserver(([e]) => { width.value = Math.floor(e!.contentRect.width); });
  ro.observe(el);
});
onBeforeUnmount(() => ro?.disconnect());
watch(selected, () => { hover.value = null; });

const maxT = computed(() => Math.max(10, ...events.value.map(e => e.timeSecs)));
const x = (t: number) => M.l + (Math.max(0, t) / maxT.value) * (width.value - M.l - M.r);
const y = (p: number) => M.t + (1 - p) * (H - M.t - M.b);
const xTicks = computed(() => {
  const stepS = maxT.value > 90 ? 20 : maxT.value > 40 ? 10 : 5;
  const out: number[] = [];
  for (let t = 0; t <= maxT.value; t += stepS) out.push(t);
  return out;
});

/** 两个事件之间胜率随时间平滑漂移（上一点 pAfter → 本点 pBefore），事件处竖直跳到 pAfter。 */
const curvePath = computed(() => {
  const es = events.value;
  if (es.length === 0) return '';
  let d = `M${x(es[0]!.timeSecs).toFixed(1)},${y(es[0]!.after).toFixed(1)}`;
  for (let i = 1; i < es.length; i++) {
    const e = es[i]!;
    const px = x(e.timeSecs).toFixed(1);
    d += `L${px},${y(e.before).toFixed(1)}L${px},${y(e.after).toFixed(1)}`;
  }
  return d;
});

const hoverPt = computed(() => (hover.value != null ? events.value[hover.value] ?? null : null));
const tipStyle = computed(() => {
  const e = hoverPt.value;
  if (!e) return {};
  const px = x(e.timeSecs);
  const right = px > width.value * 0.6;
  return right ? { right: `${width.value - px + 12}px`, top: '8px' } : { left: `${px + 12}px`, top: '8px' };
});
</script>

<style scoped>
.winprob { display: flex; flex-direction: column; gap: var(--space-3); }
.state-msg { padding: var(--space-6); text-align: center; }
.err { font-size: var(--text-xs); }
.persp-note { display: flex; align-items: center; gap: var(--space-1); }

/* 回合条 */
.strip-wrap { overflow-x: auto; padding-bottom: var(--space-1); }
.strip { display: flex; align-items: stretch; width: max-content; }
.labels, .round-col { display: flex; flex-direction: column; }
.labels { position: sticky; left: 0; z-index: 1; width: 64px; background: var(--surface); }
.lbl { justify-content: flex-start !important; font-size: var(--text-xs); color: var(--text-3); white-space: nowrap; }
.cell-h, .bar-cell, .res-cell { display: flex; align-items: center; justify-content: center; }
.cell-h { height: 20px; font-size: var(--text-xs); color: var(--text-3); }
.bar-cell { height: 32px; }
.res-cell { height: 14px; }

.round-col {
  width: 34px; padding: 2px 0;
  border: 1px solid transparent; border-radius: var(--radius-sm);
  background: transparent; color: inherit; cursor: pointer; font: inherit;
}
.round-col:hover { background: var(--surface-hover); }
.round-col:hover .cell-h { color: var(--text); }
.round-col.active { background: var(--surface-active); border-color: var(--accent-line); }
.round-col.active .cell-h { color: var(--text); }
.round-col.half { margin-left: 10px; position: relative; }
.round-col.half::before {
  content: ''; position: absolute; left: -6px; top: 4px; bottom: 4px;
  border-left: 1px dashed var(--border-strong);
}
.bar-track { position: relative; width: 10px; height: 28px; background: var(--bg-sunken); border-radius: 2px; overflow: hidden; }
.bar-fill { position: absolute; left: 0; right: 0; bottom: 0; background: var(--text-3); }
.round-col.active .bar-fill { background: var(--text); }
.bar-mid { position: absolute; left: 0; right: 0; top: 50%; border-top: 1px dashed var(--border-strong); z-index: 1; }
.res-dot { width: 8px; height: 4px; border-radius: 1px; }
.win-t { background: var(--t-side); }
.win-ct { background: var(--ct-side); }
.win-none { background: var(--border); }

/* 选中回合 */
.round-body { display: flex; flex-direction: column; gap: var(--space-3); padding-top: var(--space-3); border-top: 1px solid var(--border-subtle); }
.rd-head { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4); font-size: var(--text-sm); color: var(--text-2); }
.rd-title { font-weight: 600; color: var(--text); font-size: var(--text-md); }
.rd-item { display: inline-flex; align-items: center; gap: var(--space-2); }

.plot { position: relative; width: 100%; }
svg { display: block; overflow: visible; }
.grid line { stroke: var(--border-subtle); stroke-width: 1; }
.grid line.mid { stroke: var(--border-strong); stroke-dasharray: 4 3; }
.grid text, .xaxis text { fill: var(--text-3); font-size: 10px; font-family: var(--font-mono); }
.plant line { stroke: var(--t-side-line); stroke-width: 1; stroke-dasharray: 3 3; }
.plant text { fill: var(--t-side); font-size: 10px; }
.crosshair { stroke: var(--border-strong); stroke-width: 1; }
.curve { fill: none; stroke: var(--text); stroke-width: 1.75; stroke-linejoin: round; }

.pt { stroke: var(--surface); stroke-width: 1; }
.pt.s-t { fill: var(--t-side); }
.pt.s-ct { fill: var(--ct-side); }
.pt.s-none { fill: var(--text-3); }
.pt.k-damage { opacity: 0.7; stroke: none; }
.pt.k-start, .pt.k-end { fill: var(--surface); stroke: var(--text-2); stroke-width: 1.5; }
.pt.k-planted, .pt.k-defused, .pt.k-exploded { stroke: var(--text); stroke-width: 1.5; }
.hit { fill: transparent; cursor: crosshair; }

.tooltip {
  position: absolute; z-index: 2; pointer-events: none; min-width: 220px;
  padding: var(--space-2) var(--space-3);
  background: var(--surface-raised); border: 1px solid var(--border); border-radius: var(--radius-md);
  box-shadow: var(--shadow-pop); font-size: var(--text-xs);
}
.tip-title { color: var(--text); font-weight: 600; margin-bottom: 4px; }
.tip-row { display: flex; align-items: center; gap: var(--space-2); height: 20px; color: var(--text-2); }
.tip-row .num:last-child { margin-left: auto; }

.lists { display: grid; grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); gap: var(--space-4); }
@media (max-width: 900px) { .lists { grid-template-columns: minmax(0, 1fr); } }
.list-block { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
.list-title { font-size: var(--text-xs); color: var(--text-3); }
.col-time { width: 56px; }
.who { display: inline-flex; align-items: center; gap: var(--space-2); }
.plink { color: var(--text); text-decoration: none; }
.plink:hover { text-decoration: underline; text-underline-offset: 2px; }
.team-hint { font-size: var(--text-xs); }
td.good, span.good { color: var(--value-good); }
td.bad, span.bad { color: var(--value-bad); }
td.muted { color: var(--text-3); }
</style>
