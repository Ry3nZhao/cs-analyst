<template>
  <div class="replay-page">
    <!-- 顶栏 -->
    <header class="rp-head">
      <NuxtLink :to="`/matches/${matchId}`" class="btn btn-sm btn-ghost">← 比赛详情</NuxtLink>
      <div class="rp-title">
        <span class="rp-map">{{ mapLabel }}</span>
        <span v-if="data?.header.playedAt" class="muted num">{{ fmtDate(data.header.playedAt) }}</span>
      </div>
      <div class="seg" role="tablist">
        <button type="button" role="tab" :class="{ active: view === 'replay' }" @click="setView('replay')">2D 回放</button>
        <button type="button" role="tab" :class="{ active: view === 'heat' }" @click="setView('heat')">热力图</button>
      </div>
      <div class="spacer" />
      <span v-if="radarStatus" class="rp-radar-status" :title="radar.info?.note ?? undefined">{{ radarStatus }}</span>
      <button type="button" class="btn btn-sm" @click="settingsOpen = true">底图设置</button>
    </header>

    <div v-if="loading" class="rp-state muted">正在加载回放…</div>
    <div v-else-if="error" class="callout callout-danger">{{ error }}</div>

    <template v-else-if="data">
      <!-- ── 回放 ── -->
      <div v-show="view === 'replay'" class="rp-body">
        <div ref="stageEl" class="rp-stage">
          <canvas
            ref="canvasEl"
            class="rp-canvas"
            @wheel.prevent="onWheel"
            @pointerdown="onPointerDown"
            @pointermove="onPointerMove"
            @pointerup="onPointerUp"
            @pointerleave="hoverSlot = null"
            @dblclick="resetView"
          />
          <div class="rp-hud">
            <div class="hud-score num">
              <span class="t">{{ hud.scoreT }}</span>
              <span class="dim">:</span>
              <span class="ct">{{ hud.scoreCt }}</span>
            </div>
            <div class="hud-round">第 {{ hud.round }} 回合</div>
            <div class="hud-clock num" :class="{ bomb: hud.bomb }">{{ hud.clock }}</div>
          </div>
          <div class="rp-layers">
            <template v-if="hasLower">
              <button type="button" class="chip" :class="{ on: level === 'default' }" @click="level = 'default'">上层</button>
              <button type="button" class="chip" :class="{ on: level === 'lower' }" @click="level = 'lower'">下层</button>
              <span class="sep" />
            </template>
            <button type="button" class="chip" :class="{ on: showNades }" title="本回合全部道具落点" @click="showNades = !showNades">道具落点</button>
            <button type="button" class="chip" :class="{ on: showNames }" @click="showNames = !showNames">名字</button>
          </div>
          <div v-if="projection?.fitted" class="rp-fit-note">无底图 · 按坐标自动拟合</div>
        </div>

        <!-- 侧栏 -->
        <aside class="rp-side">
          <section v-for="g in playerGroups" :key="g.side" class="side-group">
            <div class="side-head">
              <SideBadge :side="g.side" />
              <span class="muted">{{ g.alive }} / {{ g.players.length }} 存活</span>
            </div>
            <div
              v-for="p in g.players"
              :key="p.slot"
              class="prow"
              :class="{ dead: !p.alive, sel: selectedSlot === p.slot }"
              @click="selectedSlot = selectedSlot === p.slot ? null : p.slot"
              @mouseenter="hoverSlot = p.slot"
              @mouseleave="hoverSlot = null"
            >
              <span class="pname truncate">{{ p.name }}</span>
              <span class="pflags">
                <span v-if="p.flashed" class="pflag" title="被闪">闪</span>
                <span v-if="p.scoped" class="pflag" title="开镜">镜</span>
                <span v-if="p.kills" class="pk num" title="本回合击杀">{{ p.kills }}K</span>
              </span>
              <span class="php num">{{ p.alive ? p.hp : '' }}</span>
              <span class="hpbar"><span class="hpfill" :class="g.side === 2 ? 't' : 'ct'" :style="{ width: `${p.alive ? p.hp : 0}%` }" /></span>
            </div>
          </section>

          <section class="side-group feed">
            <div class="side-head"><span class="side-title">击杀流水</span><span class="muted num">{{ roundKills.length }}</span></div>
            <div v-if="!roundKills.length" class="muted feed-empty">本回合没有击杀</div>
            <button
              v-for="(k, i) in roundKills"
              :key="i"
              type="button"
              class="krow"
              :class="{ future: k.tick > curTick }"
              @click="seek(k.tick - 3 * tickRate)"
            >
              <span class="ktime num">{{ k.clock }}</span>
              <span class="kname truncate" :class="sideCls(k.attackerTeam)">{{ k.attackerName }}</span>
              <span class="kweap">{{ k.weapon }}<span v-if="k.headshot" class="hs" title="爆头">HS</span></span>
              <span class="kname truncate" :class="sideCls(k.victimTeam)">{{ k.victimName }}</span>
            </button>
          </section>
        </aside>

        <!-- 播放控制 -->
        <div class="rp-transport">
          <div class="rounds-strip">
            <button
              v-for="r in data.header.rounds"
              :key="r.number"
              type="button"
              class="rbtn num"
              :class="[{ cur: r.number === hud.round }, r.winner === 2 ? 'w-t' : r.winner === 3 ? 'w-ct' : '']"
              :title="`第 ${r.number} 回合${r.reason ? ' · ' + (ROUND_REASON[r.reason] ?? r.reason) : ''}`"
              @click="jumpRound(r.number)"
            >{{ r.number }}</button>
          </div>
          <div class="controls">
            <button type="button" class="btn btn-sm btn-icon" title="上一回合 ( [ )" @click="stepRound(-1)">⏮</button>
            <button type="button" class="btn-primary btn btn-sm play" :title="playing ? '暂停（空格）' : '播放（空格）'" @click="togglePlay">
              {{ playing ? '暂停' : '播放' }}
            </button>
            <button type="button" class="btn btn-sm btn-icon" title="下一回合 ( ] )" @click="stepRound(1)">⏭</button>
            <div
              ref="timelineEl"
              class="timeline"
              @pointerdown="onTimelineDown"
              @pointermove="onTimelineMove"
              @pointerup="onTimelineUp"
            >
              <div class="tl-track">
                <div class="tl-freeze" :style="{ width: `${tl.freezePct}%` }" />
                <div class="tl-fill" :style="{ width: `${tl.pct}%` }" />
                <div v-for="(m, i) in tl.marks" :key="i" class="tl-mark" :class="m.cls" :style="{ left: `${m.pct}%` }" :title="m.title" />
                <div class="tl-thumb" :style="{ left: `${tl.pct}%` }" />
              </div>
            </div>
            <span class="tl-time num">{{ tl.label }}</span>
            <div class="seg speed">
              <button v-for="s in SPEEDS" :key="s" type="button" :class="{ active: speed === s }" @click="speed = s">{{ s }}×</button>
            </div>
          </div>
          <div class="hints">空格 播放/暂停 · ← → 5 秒（Shift 1 秒）· , . 单帧 · [ ] 回合 · ↑ ↓ 倍速 · 滚轮缩放 · 拖动平移 · 双击复位</div>
        </div>
      </div>

      <!-- ── 热力图 ── -->
      <ReplayHeatmap
        v-if="view === 'heat'"
        :header="data.header"
        :radar="radar"
        :projection="projection"
      />
    </template>

    <ReplayRadarSettings v-model:open="settingsOpen" :map-name="data?.header.mapName ?? null" :info="radar.info" @saved="reloadRadar" />
  </div>
</template>

<script setup lang="ts">
import { REPLAY_FLAG } from '@cs-analyst/shared-types';
import {
  cssVar, fitProjection, fmtClock, frameAt, isLower, loadRadar, loadReplay, projectionFromCalibration, ROUND_REASON,
  weaponLabel, worldToRadar, type Projection, type Radar, type ReplayData,
} from '~/lib/replay';

const SPEEDS = [0.5, 1, 2, 4, 8] as const;
const SMOKE_SECS = 18;
const INFERNO_SECS = 7;
const KILL_LINE_SECS = 3;
const SMOKE_RADIUS = 144; // world units
const INFERNO_RADIUS = 120;

const route = useRoute();
const router = useRouter();
const matchId = computed(() => String(route.params.id));
const view = computed(() => (route.query.view === 'heat' ? 'heat' : 'replay'));
function setView(v: 'replay' | 'heat') {
  router.replace({ query: { ...route.query, view: v === 'heat' ? 'heat' : undefined } });
}

const loading = ref(true);
const error = ref<string | null>(null);
const data = shallowRef<ReplayData | null>(null);
const radar = shallowRef<Radar>({ info: null, images: {} });
const settingsOpen = ref(false);
const level = ref<'default' | 'lower'>('default');
const showNades = ref(false);
const showNames = ref(true);
const selectedSlot = ref<number | null>(null);
const hoverSlot = ref<number | null>(null);

const tickRate = computed(() => data.value?.header.tickRate ?? 64);
const mapLabel = computed(() => data.value?.header.mapName ?? '未知地图');
const hasLower = computed(() => projection.value?.calibration.lowerMaxZ != null && !!radar.value.images.lower);

const radarStatus = computed(() => {
  const i = radar.value.info;
  if (!data.value) return '';
  if (radar.value.images.default) return i?.levels[0]?.source === 'user' ? '底图：自定义目录' : '底图：本机 CS2';
  return '无底图';
});

function fmtDate(s: string) {
  return s.replace('T', ' ').slice(0, 16);
}
function sideCls(t: number | null | undefined) {
  return t === 2 ? 't' : t === 3 ? 'ct' : '';
}

// ---------------------------------------------------------------------------------------------
// 加载
// ---------------------------------------------------------------------------------------------

const projection = shallowRef<Projection | null>(null);

function buildProjection() {
  const d = data.value;
  if (!d) return;
  const cal = radar.value.info?.calibration;
  if (cal) {
    projection.value = projectionFromCalibration(cal);
  } else {
    projection.value = fitProjection(d.x, d.y, (i) => (d.flags[i]! & REPLAY_FLAG.alive) !== 0);
  }
}

async function reloadRadar() {
  radar.value = await loadRadar(data.value?.header.mapName);
  buildProjection();
  draw();
}

onMounted(async () => {
  try {
    data.value = await loadReplay(matchId.value);
    radar.value = await loadRadar(data.value.header.mapName);
    buildProjection();
    const first = data.value.header.rounds[0];
    curTick.value = first ? (first.freezeEndTick ?? first.startTick) : data.value.ticks[0] ?? 0;
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
  await nextTick();
  setupCanvas();
  window.addEventListener('keydown', onKey);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  cancelAnimationFrame(raf);
  ro?.disconnect();
});

// ---------------------------------------------------------------------------------------------
// 播放
// ---------------------------------------------------------------------------------------------

const curTick = ref(0);
const playing = ref(false);
const speed = ref<number>(1);
let raf = 0;
let lastTs = 0;

const firstTick = computed(() => data.value?.ticks[0] ?? 0);
const lastTick = computed(() => {
  const t = data.value?.ticks;
  return t && t.length ? t[t.length - 1]! : 0;
});

function loop(ts: number) {
  if (!playing.value) return;
  const dt = lastTs ? Math.min(0.1, (ts - lastTs) / 1000) : 0;
  lastTs = ts;
  curTick.value = Math.min(lastTick.value, curTick.value + dt * tickRate.value * speed.value);
  if (curTick.value >= lastTick.value) playing.value = false;
  draw();
  if (playing.value) raf = requestAnimationFrame(loop);
}

function togglePlay() {
  if (!data.value) return;
  playing.value = !playing.value;
  if (playing.value) {
    if (curTick.value >= lastTick.value) curTick.value = firstTick.value;
    lastTs = 0;
    raf = requestAnimationFrame(loop);
  }
}

function seek(t: number) {
  curTick.value = Math.max(firstTick.value, Math.min(lastTick.value, t));
  draw();
}

const currentRound = computed(() => {
  const rs = data.value?.header.rounds ?? [];
  let r = rs[0];
  for (const x of rs) if (x.startTick <= curTick.value) r = x;
  return r;
});

function jumpRound(n: number) {
  const r = data.value?.header.rounds.find((x) => x.number === n);
  if (r) seek(r.freezeEndTick ?? r.startTick);
}
function stepRound(d: number) {
  const n = (currentRound.value?.number ?? 1) + d;
  const rs = data.value?.header.rounds ?? [];
  if (n >= 1 && n <= (rs[rs.length - 1]?.number ?? 0)) jumpRound(n);
}

function onKey(e: KeyboardEvent) {
  if (view.value !== 'replay' || settingsOpen.value) return;
  const tag = (e.target as HTMLElement | null)?.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
  const step = (d: number) => seek(curTick.value + d * tickRate.value);
  const frame = (data.value?.header.tickStep ?? 4);
  switch (e.key) {
    case ' ': togglePlay(); break;
    case 'ArrowLeft': step(e.shiftKey ? -1 : -5); break;
    case 'ArrowRight': step(e.shiftKey ? 1 : 5); break;
    case ',': seek(curTick.value - frame); break;
    case '.': seek(curTick.value + frame); break;
    case '[': case 'PageUp': stepRound(-1); break;
    case ']': case 'PageDown': stepRound(1); break;
    case 'ArrowUp': speed.value = SPEEDS[Math.min(SPEEDS.length - 1, SPEEDS.indexOf(speed.value as never) + 1)]!; break;
    case 'ArrowDown': speed.value = SPEEDS[Math.max(0, SPEEDS.indexOf(speed.value as never) - 1)]!; break;
    case 'Home': jumpRound(currentRound.value?.number ?? 1); break;
    default: return;
  }
  e.preventDefault();
}

// ---------------------------------------------------------------------------------------------
// HUD、侧栏（与 curTick 联动的派生数据）
// ---------------------------------------------------------------------------------------------

const nameOf = computed(() => {
  const m = new Map<string, string>();
  for (const p of data.value?.header.players ?? []) m.set(p.steamid, p.name);
  return m;
});

function roundClock(r: { freezeEndTick: number | null; startTick: number }, tick: number) {
  const fe = r.freezeEndTick ?? r.startTick;
  return fmtClock(Math.max(0, (tick - fe) / tickRate.value));
}

const roundKills = computed(() => {
  const r = currentRound.value;
  if (!r || !data.value) return [];
  return data.value.header.kills
    .filter((k) => k.round === r.number)
    .map((k) => ({
      ...k,
      clock: roundClock(r, k.tick),
      attackerName: k.attacker && k.attacker !== k.victim ? nameOf.value.get(k.attacker) ?? '?' : '',
      victimName: k.victim ? nameOf.value.get(k.victim) ?? '?' : '?',
      weapon: k.attacker && k.attacker === k.victim && k.weapon === 'world' ? '自杀' : weaponLabel(k.weapon),
    }));
});

const planted = computed(() => {
  const r = currentRound.value;
  if (!r || !data.value) return null;
  const evs = data.value.header.bomb.filter((b) => b.round === r.number && b.tick <= curTick.value);
  const p = evs.filter((b) => b.kind === 'planted').pop();
  if (!p) return null;
  const end = evs.find((b) => (b.kind === 'defused' || b.kind === 'exploded') && b.tick >= p.tick);
  return { ...p, end: end?.kind ?? null };
});

const hud = computed(() => {
  const r = currentRound.value;
  const rs = data.value?.header.rounds ?? [];
  const prev = r ? rs.find((x) => x.number === r.number - 1) : undefined;
  const scoreT = curTick.value > (r?.endTick ?? Infinity) ? r?.scoreT : prev?.scoreT;
  const scoreCt = curTick.value > (r?.endTick ?? Infinity) ? r?.scoreCt : prev?.scoreCt;
  let clock = '';
  let bomb = false;
  if (r) {
    const fe = r.freezeEndTick ?? r.startTick;
    if (curTick.value < fe) clock = `冻结 ${fmtClock((fe - curTick.value) / tickRate.value)}`;
    else if (planted.value && !planted.value.end) {
      bomb = true;
      clock = `C4 ${fmtClock(40 - (curTick.value - planted.value.tick) / tickRate.value)}`;
    } else if (curTick.value > r.endTick) clock = '回合结束';
    else clock = fmtClock(115 - (curTick.value - fe) / tickRate.value);
  }
  return { round: r?.number ?? 0, scoreT: scoreT ?? 0, scoreCt: scoreCt ?? 0, clock, bomb };
});

/** 侧栏选手状态，每 100ms 刷新一次，避免每帧触发 Vue 更新。 */
const snapshotTick = ref(0);
let lastSnap = 0;
const playerGroups = computed(() => {
  const d = data.value;
  void snapshotTick.value;
  if (!d) return [];
  const f = frameAt(d.ticks, curTick.value);
  const np = d.header.nPlayers;
  const r = currentRound.value;
  const kills = new Map<string, number>();
  for (const k of d.header.kills) {
    if (r && k.round === r.number && k.tick <= curTick.value && k.attacker && k.attacker !== k.victim) {
      kills.set(k.attacker, (kills.get(k.attacker) ?? 0) + 1);
    }
  }
  const rows = d.header.players.map((p) => {
    const i = f * np + p.slot;
    const fl = d.flags[i]!;
    return {
      slot: p.slot,
      name: p.name,
      side: fl & REPLAY_FLAG.ct ? 3 : 2,
      alive: (fl & REPLAY_FLAG.alive) !== 0,
      hp: d.health[i]!,
      flashed: (fl & REPLAY_FLAG.flashed) !== 0 && (fl & REPLAY_FLAG.alive) !== 0,
      scoped: (fl & REPLAY_FLAG.scoped) !== 0 && (fl & REPLAY_FLAG.alive) !== 0,
      kills: kills.get(p.steamid) ?? 0,
    };
  });
  return ([2, 3] as const).map((side) => {
    const players = rows.filter((p) => p.side === side).sort((a, b) => a.name.localeCompare(b.name));
    return { side, players, alive: players.filter((p) => p.alive).length };
  });
});

// 时间轴：当前回合范围
const tl = computed(() => {
  const d = data.value;
  const r = currentRound.value;
  if (!d || !r) return { pct: 0, freezePct: 0, marks: [], label: '', start: 0, end: 1 };
  const rs = d.header.rounds;
  const next = rs.find((x) => x.number === r.number + 1);
  const start = r.startTick;
  const end = Math.max(start + 1, next ? next.startTick : Math.min(lastTick.value, r.endTick + 5 * tickRate.value));
  const pct = (t: number) => Math.max(0, Math.min(100, ((t - start) / (end - start)) * 100));
  const marks = [
    ...d.header.kills.filter((k) => k.round === r.number).map((k) => ({
      pct: pct(k.tick),
      cls: `kill ${sideCls(k.attackerTeam)}`,
      title: `${nameOf.value.get(k.attacker ?? '') ?? '?'} → ${nameOf.value.get(k.victim ?? '') ?? '?'}`,
    })),
    ...d.header.bomb.filter((b) => b.round === r.number && b.kind !== 'plant_begin' && b.kind !== 'defuse_begin').map((b) => ({
      pct: pct(b.tick),
      cls: `bomb ${b.kind}`,
      title: ({ planted: '下包', defused: '拆除', exploded: '爆炸' } as Record<string, string>)[b.kind] ?? b.kind,
    })),
    { pct: pct(r.endTick), cls: 'end', title: '回合结束' },
  ];
  return {
    pct: pct(curTick.value),
    freezePct: pct(r.freezeEndTick ?? start),
    marks,
    label: `${fmtClock((curTick.value - start) / tickRate.value)} / ${fmtClock((end - start) / tickRate.value)}`,
    start,
    end,
  };
});

const timelineEl = ref<HTMLDivElement | null>(null);
let tlDragging = false;
function tlSeek(e: PointerEvent) {
  const el = timelineEl.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const f = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  seek(tl.value.start + f * (tl.value.end - tl.value.start));
}
function onTimelineDown(e: PointerEvent) {
  tlDragging = true;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  tlSeek(e);
}
function onTimelineMove(e: PointerEvent) {
  if (tlDragging) tlSeek(e);
}
function onTimelineUp() {
  tlDragging = false;
}

// ---------------------------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------------------------

const stageEl = ref<HTMLDivElement | null>(null);
const canvasEl = ref<HTMLCanvasElement | null>(null);
let ro: ResizeObserver | null = null;
const cam = { x: 512, y: 512, zoom: 1 };
let W = 0;
let H = 0;
let dpr = 1;

function setupCanvas() {
  const stage = stageEl.value;
  if (!stage) return;
  ro = new ResizeObserver(() => {
    const c = canvasEl.value;
    if (!c) return;
    const rect = stage.getBoundingClientRect();
    dpr = window.devicePixelRatio || 1;
    W = rect.width;
    H = rect.height;
    c.width = Math.round(W * dpr);
    c.height = Math.round(H * dpr);
    c.style.width = `${W}px`;
    c.style.height = `${H}px`;
    draw();
  });
  ro.observe(stage);
}

function baseScale() {
  return (Math.min(W, H) / 1024) * cam.zoom;
}
function toCanvas(rx: number, ry: number): [number, number] {
  const s = baseScale();
  return [W / 2 + (rx - cam.x) * s, H / 2 + (ry - cam.y) * s];
}
function worldToCanvas(x: number, y: number): [number, number] {
  const p = projection.value!;
  const [rx, ry] = worldToRadar(p, x, y);
  return toCanvas(rx, ry);
}

function resetView() {
  cam.x = 512;
  cam.y = 512;
  cam.zoom = 1;
  draw();
}

function onWheel(e: WheelEvent) {
  const rect = canvasEl.value!.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;
  const s0 = baseScale();
  const rx = cam.x + (mx - W / 2) / s0;
  const ry = cam.y + (my - H / 2) / s0;
  cam.zoom = Math.max(0.6, Math.min(8, cam.zoom * Math.exp(-e.deltaY * 0.0015)));
  const s1 = baseScale();
  cam.x = rx - (mx - W / 2) / s1;
  cam.y = ry - (my - H / 2) / s1;
  draw();
}

let drag: { x: number; y: number; cx: number; cy: number; moved: boolean } | null = null;
function onPointerDown(e: PointerEvent) {
  drag = { x: e.clientX, y: e.clientY, cx: cam.x, cy: cam.y, moved: false };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}
function onPointerMove(e: PointerEvent) {
  if (drag) {
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true;
    if (drag.moved) {
      const s = baseScale();
      cam.x = drag.cx - dx / s;
      cam.y = drag.cy - dy / s;
      draw();
    }
    return;
  }
  const rect = canvasEl.value!.getBoundingClientRect();
  hoverSlot.value = pickSlot(e.clientX - rect.left, e.clientY - rect.top);
}
function onPointerUp(e: PointerEvent) {
  if (drag && !drag.moved) {
    const rect = canvasEl.value!.getBoundingClientRect();
    const s = pickSlot(e.clientX - rect.left, e.clientY - rect.top);
    selectedSlot.value = s === selectedSlot.value ? null : s;
  }
  drag = null;
}

let lastPositions: { slot: number; x: number; y: number }[] = [];
function pickSlot(mx: number, my: number): number | null {
  let best: number | null = null;
  let bd = 12 * 12;
  for (const p of lastPositions) {
    const d = (p.x - mx) ** 2 + (p.y - my) ** 2;
    if (d < bd) {
      bd = d;
      best = p.slot;
    }
  }
  return best;
}

watch([level, showNades, showNames, selectedSlot, hoverSlot, speed], () => draw());

function lerpAngle(a: number, b: number, t: number) {
  const d = ((b - a + 540) % 360) - 180;
  return a + d * t;
}

function draw() {
  const c = canvasEl.value;
  const d = data.value;
  const p = projection.value;
  if (!c || !d || !p || W === 0) return;
  const ctx = c.getContext('2d')!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const colT = cssVar('--t-side');
  const colCT = cssVar('--ct-side');
  const colText = cssVar('--text');
  const colText2 = cssVar('--text-2');
  const colText3 = cssVar('--text-3');
  const colDim = cssVar('--text-disabled');
  const colDanger = cssVar('--danger');
  const colSuccess = cssVar('--success');
  const colBorder = cssVar('--border-subtle');
  ctx.fillStyle = cssVar('--bg');
  ctx.fillRect(0, 0, W, H);

  // 底图（灰阶）或网格
  const img = level.value === 'lower' ? radar.value.images.lower : radar.value.images.default;
  const s = baseScale();
  const [ox, oy] = toCanvas(0, 0);
  if (img && !p.fitted) {
    ctx.save();
    ctx.filter = 'grayscale(1) brightness(0.9) contrast(1.05)';
    ctx.globalAlpha = 0.55;
    ctx.drawImage(img, ox, oy, 1024 * s, 1024 * s);
    ctx.restore();
  } else {
    ctx.strokeStyle = colBorder;
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let g = 0; g <= 1024; g += 64) {
      const [x1, y1] = toCanvas(g, 0);
      const [x2, y2] = toCanvas(g, 1024);
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      const [x3, y3] = toCanvas(0, g);
      const [x4, y4] = toCanvas(1024, g);
      ctx.moveTo(x3, y3);
      ctx.lineTo(x4, y4);
    }
    ctx.stroke();
  }

  const t = curTick.value;
  const r = currentRound.value;
  const tr = tickRate.value;
  const levelAlpha = (z: number) => (p.calibration.lowerMaxZ == null ? 1 : (isLower(p, z) === (level.value === 'lower') ? 1 : 0.3));
  const unit = 1 / p.calibration.scale * s; // canvas px per world unit

  // 道具：本回合全部落点（可选）+ 正在生效的效果
  if (r) {
    for (const g of d.header.grenades) {
      if (g.round !== r.number || !g.pos) continue;
      const [gx, gy] = worldToCanvas(g.pos[0], g.pos[1]);
      const age = (t - g.tick) / tr;
      ctx.globalAlpha = levelAlpha(g.pos[2]);
      if (age >= 0 && g.kind === 'smokegrenade' && age < SMOKE_SECS) {
        const fade = Math.min(1, (SMOKE_SECS - age) / 2, age / 0.5 + 0.2);
        ctx.fillStyle = cssVar('--gray-200');
        ctx.globalAlpha *= 0.42 * fade;
        ctx.beginPath();
        ctx.arc(gx, gy, SMOKE_RADIUS * unit, 0, Math.PI * 2);
        ctx.fill();
      } else if (age >= 0 && g.kind === 'inferno' && age < INFERNO_SECS) {
        ctx.fillStyle = colDanger;
        ctx.globalAlpha *= 0.28;
        ctx.beginPath();
        ctx.arc(gx, gy, INFERNO_RADIUS * unit, 0, Math.PI * 2);
        ctx.fill();
      } else if (age >= 0 && age < 0.6 && (g.kind === 'flashbang' || g.kind === 'hegrenade')) {
        const k = age / 0.6;
        ctx.strokeStyle = g.kind === 'flashbang' ? colText : colDanger;
        ctx.globalAlpha *= 1 - k;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(gx, gy, 6 + k * (g.kind === 'flashbang' ? 26 : 20), 0, Math.PI * 2);
        ctx.stroke();
      } else if (showNades.value) {
        ctx.globalAlpha *= age >= 0 ? 0.9 : 0.35;
        drawNadeIcon(ctx, g.kind, gx, gy, colText2, colDanger);
      }
      ctx.globalAlpha = 1;
    }
  }

  // C4
  if (planted.value?.pos) {
    const [bx, by] = worldToCanvas(planted.value.pos[0], planted.value.pos[1]);
    const blink = planted.value.end ? 1 : 0.55 + 0.45 * Math.abs(Math.sin(((t - planted.value.tick) / tr) * Math.PI));
    ctx.globalAlpha = blink;
    ctx.fillStyle = planted.value.end === 'defused' ? colSuccess : colDanger;
    ctx.fillRect(bx - 5, by - 5, 10, 10);
    ctx.globalAlpha = 1;
  }

  // 击杀连线
  if (r) {
    for (const k of d.header.kills) {
      if (k.round !== r.number || k.tick > t || !k.attackerPos || !k.victimPos) continue;
      const age = (t - k.tick) / tr;
      if (age > KILL_LINE_SECS || k.attacker === k.victim) continue;
      const [ax, ay] = worldToCanvas(k.attackerPos[0], k.attackerPos[1]);
      const [vx, vy] = worldToCanvas(k.victimPos[0], k.victimPos[1]);
      ctx.globalAlpha = 1 - age / KILL_LINE_SECS;
      ctx.strokeStyle = k.attackerTeam === 3 ? colCT : colT;
      ctx.lineWidth = 1.5;
      ctx.setLineDash([5, 4]);
      ctx.beginPath();
      ctx.moveTo(ax, ay);
      ctx.lineTo(vx, vy);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }
  }

  // 死亡：灰色叉（本回合死者）
  const deadNow = new Set<string>();
  if (r) {
    for (const k of d.header.kills) {
      if (k.round !== r.number || k.tick > t || !k.victim || !k.victimPos) continue;
      deadNow.add(k.victim);
      const [x, y] = worldToCanvas(k.victimPos[0], k.victimPos[1]);
      ctx.globalAlpha = levelAlpha(k.victimPos[2]);
      ctx.strokeStyle = colDim;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(x - 5, y - 5);
      ctx.lineTo(x + 5, y + 5);
      ctx.moveTo(x + 5, y - 5);
      ctx.lineTo(x - 5, y + 5);
      ctx.stroke();
      if (showNames.value) {
        ctx.fillStyle = colDim;
        ctx.font = '10px Inter, "Segoe UI", sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(nameOf.value.get(k.victim) ?? '', x, y + 16);
      }
      ctx.globalAlpha = 1;
    }
  }

  // 选手
  const f = frameAt(d.ticks, t);
  const f2 = Math.min(d.ticks.length - 1, f + 1);
  const span = d.ticks[f2]! - d.ticks[f]!;
  const a = span > 0 ? Math.max(0, Math.min(1, (t - d.ticks[f]!) / span)) : 0;
  const np = d.header.nPlayers;
  const positions: typeof lastPositions = [];
  const order = [...d.header.players].sort((x, y) => (x.slot === selectedSlot.value ? 1 : 0) - (y.slot === selectedSlot.value ? 1 : 0));
  for (const pl of order) {
    const i = f * np + pl.slot;
    const j = f2 * np + pl.slot;
    const fl = d.flags[i]!;
    if (!(fl & REPLAY_FLAG.alive)) continue;
    const both = (d.flags[j]! & REPLAY_FLAG.alive) !== 0;
    const k = both ? a : 0;
    const wx = d.x[i]! + (d.x[j]! - d.x[i]!) * k;
    const wy = d.y[i]! + (d.y[j]! - d.y[i]!) * k;
    const wz = d.z[i]! + (d.z[j]! - d.z[i]!) * k;
    const yaw = both ? lerpAngle(d.yaw[i]!, d.yaw[j]!, k) : d.yaw[i]!;
    const [x, y] = worldToCanvas(wx, wy);
    positions.push({ slot: pl.slot, x, y });
    const ct = (fl & REPLAY_FLAG.ct) !== 0;
    const col = ct ? colCT : colT;
    const ang = (-yaw * Math.PI) / 180;
    const focus = pl.slot === selectedSlot.value || pl.slot === hoverSlot.value;
    ctx.globalAlpha = levelAlpha(wz);

    // 开镜：视线
    if (fl & REPLAY_FLAG.scoped) {
      ctx.strokeStyle = col;
      ctx.globalAlpha *= 0.55;
      ctx.lineWidth = 1;
      ctx.setLineDash([2, 3]);
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(x + Math.cos(ang) * 90, y + Math.sin(ang) * 90);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = levelAlpha(wz);
    }
    // 朝向：扇形
    ctx.fillStyle = col;
    ctx.globalAlpha *= 0.35;
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.arc(x, y, 22, ang - 0.38, ang + 0.38);
    ctx.closePath();
    ctx.fill();
    ctx.globalAlpha = levelAlpha(wz);
    // 点
    const flashed = (fl & REPLAY_FLAG.flashed) !== 0;
    ctx.beginPath();
    ctx.arc(x, y, 7, 0, Math.PI * 2);
    ctx.fillStyle = flashed ? colText : col;
    ctx.fill();
    ctx.lineWidth = focus ? 2.5 : 1.5;
    ctx.strokeStyle = focus ? colText : flashed ? col : cssVar('--bg');
    ctx.stroke();
    // 朝向刻度
    ctx.strokeStyle = colText;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(x + Math.cos(ang) * 7, y + Math.sin(ang) * 7);
    ctx.lineTo(x + Math.cos(ang) * 12, y + Math.sin(ang) * 12);
    ctx.stroke();
    // 被闪：外圈
    if (flashed) {
      ctx.strokeStyle = colText;
      ctx.lineWidth = 1;
      ctx.setLineDash([2, 2]);
      ctx.beginPath();
      ctx.arc(x, y, 11, 0, Math.PI * 2);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    // 血量弧
    const hp = d.health[i]!;
    if (hp < 100) {
      ctx.strokeStyle = colText3;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(x, y, 9.5, -Math.PI / 2, -Math.PI / 2 + (Math.PI * 2 * hp) / 100);
      ctx.stroke();
    }
    if (showNames.value || focus) {
      ctx.font = `${focus ? '600 ' : ''}11px Inter, "Segoe UI", sans-serif`;
      ctx.textAlign = 'center';
      ctx.fillStyle = focus ? colText : colText2;
      ctx.fillText(pl.name.length > 14 ? `${pl.name.slice(0, 13)}…` : pl.name, x, y - 13);
    }
    ctx.globalAlpha = 1;
  }
  lastPositions = positions;

  const now = performance.now();
  if (now - lastSnap > 100 || !playing.value) {
    lastSnap = now;
    snapshotTick.value++;
  }
}

function drawNadeIcon(ctx: CanvasRenderingContext2D, kind: string, x: number, y: number, col: string, danger: string) {
  ctx.lineWidth = 1.5;
  ctx.strokeStyle = kind === 'inferno' || kind === 'hegrenade' ? danger : col;
  ctx.fillStyle = ctx.strokeStyle;
  ctx.beginPath();
  if (kind === 'smokegrenade') {
    ctx.arc(x, y, 5, 0, Math.PI * 2);
    ctx.stroke();
  } else if (kind === 'flashbang') {
    ctx.moveTo(x, y - 6);
    ctx.lineTo(x + 5, y);
    ctx.lineTo(x, y + 6);
    ctx.lineTo(x - 5, y);
    ctx.closePath();
    ctx.stroke();
  } else if (kind === 'hegrenade') {
    ctx.arc(x, y, 4, 0, Math.PI * 2);
    ctx.fill();
  } else if (kind === 'inferno') {
    ctx.moveTo(x, y - 6);
    ctx.lineTo(x + 5, y + 4);
    ctx.lineTo(x - 5, y + 4);
    ctx.closePath();
    ctx.stroke();
  } else {
    ctx.rect(x - 3, y - 3, 6, 6);
    ctx.stroke();
  }
}

watch(curTick, () => {
  if (!playing.value) draw();
});
watch(projection, () => draw());

</script>

<style scoped>
.replay-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  height: 100%;
  min-height: 560px;
}
.rp-head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
.rp-title { display: flex; align-items: baseline; gap: var(--space-2); }
.rp-map { color: var(--text); font-size: var(--text-lg); font-weight: 600; }
.spacer { flex: 1; }
.rp-radar-status { color: var(--text-3); font-size: var(--text-xs); }
.rp-state { padding: var(--space-8); text-align: center; }

.seg {
  display: inline-flex;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  overflow: hidden;
}
.seg button {
  height: var(--control-h-sm);
  padding: 0 var(--space-3);
  font-size: var(--text-sm);
  color: var(--text-3);
  background: transparent;
  border: 0;
  cursor: pointer;
}
.seg button + button { border-left: 1px solid var(--border); }
.seg button:hover { color: var(--text); background: var(--surface-hover); }
.seg button.active { color: var(--text); background: var(--surface-active); box-shadow: inset 0 -2px 0 var(--accent); }

.rp-body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 300px;
  grid-template-rows: minmax(0, 1fr) auto;
  gap: var(--space-3);
}
.rp-stage {
  position: relative;
  min-height: 0;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.rp-canvas { display: block; cursor: grab; touch-action: none; }
.rp-canvas:active { cursor: grabbing; }
.rp-hud {
  position: absolute;
  top: var(--space-3);
  left: var(--space-3);
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-1) var(--space-3);
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  pointer-events: none;
}
.hud-score { font-size: var(--text-lg); font-weight: 600; display: flex; gap: 4px; }
.hud-score .t { color: var(--t-side); }
.hud-score .ct { color: var(--ct-side); }
.hud-round { color: var(--text-2); }
.hud-clock { color: var(--text); min-width: 64px; }
.hud-clock.bomb { color: var(--danger); }
.rp-layers {
  position: absolute;
  right: var(--space-3);
  top: var(--space-3);
  display: flex;
  gap: var(--space-1);
  align-items: center;
}
.rp-layers .sep { width: 1px; height: 16px; background: var(--border); margin: 0 var(--space-1); }
.chip {
  height: 22px;
  padding: 0 var(--space-2);
  font-size: var(--text-xs);
  color: var(--text-3);
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  cursor: pointer;
}
.chip:hover { color: var(--text); }
.chip.on { color: var(--text); border-color: var(--accent-line); background: var(--accent-soft); }
.rp-fit-note {
  position: absolute;
  left: var(--space-3);
  bottom: var(--space-3);
  font-size: var(--text-xs);
  color: var(--text-3);
}

.rp-side {
  grid-row: 1 / span 2;
  grid-column: 2;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-height: 0;
}
.side-group {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: var(--space-2);
}
.side-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px var(--space-1) var(--space-2);
  font-size: var(--text-xs);
}
.side-title { color: var(--text-3); text-transform: none; }
.prow {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto 28px;
  grid-template-rows: auto 3px;
  column-gap: var(--space-2);
  row-gap: 3px;
  align-items: center;
  padding: 5px var(--space-2);
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: var(--text-sm);
}
.prow:hover { background: var(--surface-hover); }
.prow.sel { background: var(--surface-active); box-shadow: inset 2px 0 0 var(--accent); }
.prow.dead .pname { color: var(--text-disabled); text-decoration: line-through; }
.pname { color: var(--text); }
.pflags { display: flex; gap: 4px; align-items: center; }
.pflag {
  font-size: 10px;
  line-height: 14px;
  padding: 0 3px;
  border-radius: 3px;
  color: var(--text-inverse);
  background: var(--gray-0);
}
.pk { font-size: var(--text-xs); color: var(--text-2); }
.php { text-align: right; color: var(--text-2); }
.hpbar { grid-column: 1 / -1; height: 3px; background: var(--border-subtle); border-radius: 2px; overflow: hidden; }
.hpfill { display: block; height: 100%; transition: width 120ms linear; }
.hpfill.t { background: var(--t-side); }
.hpfill.ct { background: var(--ct-side); }

.feed { flex: 1; min-height: 120px; overflow-y: auto; }
.feed-empty { padding: var(--space-2); font-size: var(--text-sm); }
.krow {
  width: 100%;
  display: grid;
  grid-template-columns: 34px minmax(0, 1fr) auto minmax(0, 1fr);
  gap: var(--space-2);
  align-items: center;
  padding: 4px var(--space-2);
  font-size: var(--text-sm);
  text-align: left;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--text-2);
  cursor: pointer;
}
.krow:hover { background: var(--surface-hover); }
.krow.future { opacity: 0.35; }
.ktime { color: var(--text-3); font-size: var(--text-xs); }
.kname.t { color: var(--t-side); }
.kname.ct { color: var(--ct-side); }
.kweap { color: var(--text-3); font-size: var(--text-xs); white-space: nowrap; }
.hs { margin-left: 3px; color: var(--text); font-weight: 600; }

.rp-transport {
  grid-column: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
}
.rounds-strip { display: flex; flex-wrap: wrap; gap: 3px; }
.rbtn {
  min-width: 24px;
  height: 20px;
  font-size: var(--text-xs);
  color: var(--text-3);
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-bottom-width: 2px;
  border-radius: 3px;
  cursor: pointer;
}
.rbtn.w-t { border-bottom-color: var(--t-side); }
.rbtn.w-ct { border-bottom-color: var(--ct-side); }
.rbtn:hover { color: var(--text); border-color: var(--border-strong); }
.rbtn.cur { color: var(--text); background: var(--surface-active); border-color: var(--accent); }
.controls { display: flex; align-items: center; gap: var(--space-2); }
.play { min-width: 56px; }
.timeline { flex: 1; height: 24px; display: flex; align-items: center; cursor: pointer; touch-action: none; }
.tl-track { position: relative; width: 100%; height: 6px; background: var(--border-subtle); border-radius: 3px; }
.tl-freeze { position: absolute; left: 0; top: 0; bottom: 0; background: var(--border); border-radius: 3px 0 0 3px; }
.tl-fill { position: absolute; left: 0; top: 0; bottom: 0; background: var(--text-3); border-radius: 3px; }
.tl-mark { position: absolute; top: -4px; width: 2px; height: 14px; margin-left: -1px; background: var(--text-3); }
.tl-mark.kill.t { background: var(--t-side); }
.tl-mark.kill.ct { background: var(--ct-side); }
.tl-mark.bomb { width: 6px; height: 6px; top: 0; margin-left: -3px; border-radius: 1px; background: var(--danger); }
.tl-mark.bomb.defused { background: var(--success); }
.tl-mark.end { background: var(--text-disabled); }
.tl-thumb {
  position: absolute;
  top: 50%;
  width: 12px;
  height: 12px;
  margin: -6px 0 0 -6px;
  border-radius: 50%;
  background: var(--text);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.tl-time { width: 96px; text-align: right; color: var(--text-2); font-size: var(--text-sm); }
.speed button { padding: 0 var(--space-2); }
.hints { font-size: var(--text-xs); color: var(--text-disabled); }
.truncate { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
