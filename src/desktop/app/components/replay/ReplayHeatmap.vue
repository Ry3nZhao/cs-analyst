<template>
  <div class="heat">
    <div ref="stageEl" class="heat-stage">
      <canvas ref="canvasEl" class="heat-canvas" @wheel.prevent="onWheel" @pointerdown="onDown" @pointermove="onMove" @pointerup="drag = null" @dblclick="reset" />
      <div v-if="loading" class="heat-loading muted">计算中…</div>
      <div class="heat-legend">
        <span class="muted">少</span>
        <span class="legend-bar" />
        <span class="muted">多</span>
      </div>
      <div v-if="hasLower" class="heat-layers">
        <button type="button" class="chip" :class="{ on: level === 'default' }" @click="level = 'default'">上层</button>
        <button type="button" class="chip" :class="{ on: level === 'lower' }" @click="level = 'lower'">下层</button>
      </div>
    </div>

    <aside class="heat-side">
      <section class="panel">
        <div class="field">
          <span class="field-label">范围</span>
          <div class="seg">
            <button type="button" :class="{ active: scope === 'match' }" @click="scope = 'match'">本场</button>
            <button type="button" :class="{ active: scope === 'map' }" @click="scope = 'map'">{{ header.mapName ?? '同地图' }} 全部对局</button>
          </div>
        </div>
        <div class="field">
          <span class="field-label">数据</span>
          <div class="seg">
            <button v-for="k in KINDS" :key="k.value" type="button" :class="{ active: kind === k.value }" @click="kind = k.value">{{ k.label }}</button>
          </div>
        </div>
        <div class="field">
          <span class="field-label">阵营</span>
          <div class="seg">
            <button type="button" :class="{ active: side === null }" @click="side = null">全部</button>
            <button type="button" :class="{ active: side === 2 }" @click="side = 2">T</button>
            <button type="button" :class="{ active: side === 3 }" @click="side = 3">CT</button>
          </div>
        </div>
        <div class="field">
          <span class="field-label">回合类型（该选手所在队伍）</span>
          <div class="chips">
            <button v-for="b in BUYS" :key="b.value" type="button" class="chip" :class="{ on: buys.includes(b.value) }" @click="toggleBuy(b.value)">{{ b.label }}</button>
          </div>
        </div>
        <div class="field">
          <span class="field-label">选手</span>
          <select v-model="steamid" class="select input-sm">
            <option value="">全部选手</option>
            <option v-for="p in sortedPlayers" :key="p.steamid" :value="p.steamid">{{ p.name }}</option>
          </select>
        </div>
        <div class="field">
          <span class="field-label">显示</span>
          <div class="seg">
            <button type="button" :class="{ active: mode === 'density' }" @click="mode = 'density'">密度</button>
            <button type="button" :class="{ active: mode === 'points' }" @click="mode = 'points'">散点</button>
          </div>
        </div>
      </section>

      <section class="panel summary">
        <div v-if="error" class="form-error">{{ error }}</div>
        <template v-else>
          <div class="sum-row"><span class="muted">点数</span><span class="num strong">{{ counts.total.toLocaleString() }}</span></div>
          <div class="sum-row"><SideBadge :side="2" /><span class="num">{{ counts.t.toLocaleString() }}</span></div>
          <div class="sum-row"><SideBadge :side="3" /><span class="num">{{ counts.ct.toLocaleString() }}</span></div>
          <div class="sum-row"><span class="muted">对局</span><span class="num">{{ result?.matches.length ?? 0 }}</span></div>
          <div v-if="hidden" class="sum-note muted">另有 {{ hidden.toLocaleString() }} 个点在{{ level === 'lower' ? '上层' : '下层' }}</div>
          <div v-if="result?.truncated" class="sum-note warn">点数超过上限，只显示了一部分</div>
          <div v-if="kind === 'presence'" class="sum-note muted">站位：存活时每秒采样一次（来自回放文件）</div>
        </template>
      </section>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import type { BuyType, HeatKind, HeatmapFilters, HeatmapResult, ReplayHeader } from '@cs-analyst/shared-types';
import { cssVar, densityGrid, densityImage, isLower, worldToRadar, type Projection, type Radar } from '~/lib/replay';

const props = defineProps<{ header: ReplayHeader; radar: Radar; projection: Projection | null }>();

const KINDS: { value: HeatKind; label: string }[] = [
  { value: 'kill', label: '击杀点' },
  { value: 'death', label: '死亡点' },
  { value: 'presence', label: '站位' },
];
const BUYS: { value: BuyType; label: string }[] = [
  { value: 'pistol', label: '手枪局' },
  { value: 'eco', label: 'ECO' },
  { value: 'force', label: '强起' },
  { value: 'full', label: '全起' },
];

const scope = ref<'match' | 'map'>('match');
const kind = ref<HeatKind>('kill');
const side = ref<2 | 3 | null>(null);
const buys = ref<BuyType[]>([]);
const steamid = ref('');
const mode = ref<'density' | 'points'>('density');
const level = ref<'default' | 'lower'>('default');
const loading = ref(false);
const error = ref<string | null>(null);
const result = shallowRef<HeatmapResult | null>(null);

const hasLower = computed(() => props.projection?.calibration.lowerMaxZ != null);
const sortedPlayers = computed(() => [...props.header.players].sort((a, b) => a.name.localeCompare(b.name)));

function toggleBuy(b: BuyType) {
  buys.value = buys.value.includes(b) ? buys.value.filter((x) => x !== b) : [...buys.value, b];
}

let seq = 0;
async function query() {
  const my = ++seq;
  loading.value = true;
  error.value = null;
  const filters: HeatmapFilters = {
    matchIds: scope.value === 'match' ? [props.header.matchId] : undefined,
    maps: props.header.mapName ? [props.header.mapName] : undefined,
    steamids: steamid.value ? [steamid.value] : undefined,
    side: side.value ?? undefined,
    buyTypes: buys.value.length ? buys.value : undefined,
    kinds: [kind.value],
  };
  try {
    const r = await invoke<HeatmapResult>('get_heatmap_points', { filters });
    if (my === seq) result.value = r;
  } catch (e) {
    if (my === seq) error.value = String(e);
  } finally {
    if (my === seq) loading.value = false;
  }
}

watch([scope, kind, side, buys, steamid], query);
onMounted(query);

/** 当前层的点（底图像素坐标）。 */
const visible = computed(() => {
  const r = result.value;
  const p = props.projection;
  if (!r || !p) return { px: [] as number[], py: [] as number[], side: [] as number[], hidden: 0 };
  const px: number[] = [];
  const py: number[] = [];
  const sd: number[] = [];
  let hidden = 0;
  const wantLower = level.value === 'lower';
  for (let i = 0; i < r.x.length; i++) {
    if (p.calibration.lowerMaxZ != null && isLower(p, r.z[i]!) !== wantLower) {
      hidden++;
      continue;
    }
    const [x, y] = worldToRadar(p, r.x[i]!, r.y[i]!);
    px.push(x);
    py.push(y);
    sd.push(r.side[i]!);
  }
  return { px, py, side: sd, hidden };
});
const hidden = computed(() => visible.value.hidden);
const counts = computed(() => {
  const s = visible.value.side;
  let t = 0;
  for (const v of s) if (v === 2) t++;
  return { total: s.length, t, ct: s.length - t };
});

// ---- canvas
const stageEl = ref<HTMLDivElement | null>(null);
const canvasEl = ref<HTMLCanvasElement | null>(null);
let ro: ResizeObserver | null = null;
let W = 0;
let H = 0;
let dpr = 1;
const cam = { x: 512, y: 512, zoom: 1 };
let heatCanvas: HTMLCanvasElement | null = null;
const GRID = 256;

onMounted(() => {
  ro = new ResizeObserver(() => {
    const st = stageEl.value;
    const c = canvasEl.value;
    if (!st || !c) return;
    const r = st.getBoundingClientRect();
    dpr = window.devicePixelRatio || 1;
    W = r.width;
    H = r.height;
    c.width = Math.round(W * dpr);
    c.height = Math.round(H * dpr);
    c.style.width = `${W}px`;
    c.style.height = `${H}px`;
    draw();
  });
  if (stageEl.value) ro.observe(stageEl.value);
});
onBeforeUnmount(() => ro?.disconnect());

function scale() {
  return (Math.min(W, H) / 1024) * cam.zoom;
}
function toCanvas(rx: number, ry: number): [number, number] {
  const s = scale();
  return [W / 2 + (rx - cam.x) * s, H / 2 + (ry - cam.y) * s];
}
function reset() {
  cam.x = 512;
  cam.y = 512;
  cam.zoom = 1;
  draw();
}
function onWheel(e: WheelEvent) {
  const rect = canvasEl.value!.getBoundingClientRect();
  const mx = e.clientX - rect.left;
  const my = e.clientY - rect.top;
  const s0 = scale();
  const rx = cam.x + (mx - W / 2) / s0;
  const ry = cam.y + (my - H / 2) / s0;
  cam.zoom = Math.max(0.6, Math.min(8, cam.zoom * Math.exp(-e.deltaY * 0.0015)));
  const s1 = scale();
  cam.x = rx - (mx - W / 2) / s1;
  cam.y = ry - (my - H / 2) / s1;
  draw();
}
const drag = ref<{ x: number; y: number; cx: number; cy: number } | null>(null);
function onDown(e: PointerEvent) {
  drag.value = { x: e.clientX, y: e.clientY, cx: cam.x, cy: cam.y };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}
function onMove(e: PointerEvent) {
  const d = drag.value;
  if (!d) return;
  const s = scale();
  cam.x = d.cx - (e.clientX - d.x) / s;
  cam.y = d.cy - (e.clientY - d.y) / s;
  draw();
}

function rebuildHeat() {
  const v = visible.value;
  const g = densityGrid(v.px, v.py, GRID, 1024, kind.value === 'presence' ? 1.6 : 2.4);
  const img = densityImage(g, GRID, cssVar('--accent', '#3ccfdc'));
  heatCanvas ??= document.createElement('canvas');
  heatCanvas.width = GRID;
  heatCanvas.height = GRID;
  heatCanvas.getContext('2d')!.putImageData(img, 0, 0);
}

function draw() {
  const c = canvasEl.value;
  const p = props.projection;
  if (!c || W === 0) return;
  const ctx = c.getContext('2d')!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = cssVar('--bg');
  ctx.fillRect(0, 0, W, H);
  if (!p) return;
  const s = scale();
  const [ox, oy] = toCanvas(0, 0);
  const img = level.value === 'lower' ? props.radar.images.lower : props.radar.images.default;
  if (img && !p.fitted) {
    ctx.save();
    ctx.filter = 'grayscale(1) brightness(0.75)';
    ctx.globalAlpha = 0.5;
    ctx.drawImage(img, ox, oy, 1024 * s, 1024 * s);
    ctx.restore();
  } else {
    ctx.strokeStyle = cssVar('--border-subtle');
    ctx.strokeRect(ox, oy, 1024 * s, 1024 * s);
  }
  const v = visible.value;
  if (mode.value === 'density') {
    if (heatCanvas) {
      ctx.save();
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = 'high';
      ctx.drawImage(heatCanvas, ox, oy, 1024 * s, 1024 * s);
      ctx.restore();
    }
  } else {
    const colT = cssVar('--t-side');
    const colCT = cssVar('--ct-side');
    const r = kind.value === 'presence' ? 1.5 : 3;
    ctx.globalAlpha = kind.value === 'presence' ? 0.35 : 0.85;
    for (let i = 0; i < v.px.length; i++) {
      const [x, y] = toCanvas(v.px[i]!, v.py[i]!);
      ctx.fillStyle = v.side[i] === 3 ? colCT : colT;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.globalAlpha = 1;
  }
}

watch([visible, mode], () => {
  rebuildHeat();
  draw();
});
watch(() => [props.projection, props.radar], () => draw());
</script>

<style scoped>
.heat {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 300px;
  gap: var(--space-3);
}
.heat-stage {
  position: relative;
  min-height: 0;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.heat-canvas { display: block; cursor: grab; touch-action: none; }
.heat-loading { position: absolute; top: var(--space-3); left: var(--space-3); font-size: var(--text-sm); }
.heat-legend {
  position: absolute;
  left: var(--space-3);
  bottom: var(--space-3);
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-xs);
}
.legend-bar {
  width: 120px;
  height: 6px;
  border-radius: 3px;
  background: linear-gradient(90deg, color-mix(in srgb, var(--gray-300) 30%, transparent), var(--gray-0) 55%, var(--accent));
}
.heat-layers { position: absolute; right: var(--space-3); top: var(--space-3); display: flex; gap: var(--space-1); }
.heat-side { display: flex; flex-direction: column; gap: var(--space-3); min-height: 0; overflow-y: auto; }
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
}
.panel .field { display: flex; flex-direction: column; gap: 6px; }
.chips { display: flex; flex-wrap: wrap; gap: var(--space-1); }
.chip {
  height: 22px;
  padding: 0 var(--space-2);
  font-size: var(--text-xs);
  color: var(--text-3);
  background: transparent;
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  cursor: pointer;
}
.chip:hover { color: var(--text); }
.chip.on { color: var(--text); border-color: var(--accent-line); background: var(--accent-soft); }
.seg {
  display: inline-flex;
  align-self: flex-start;
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
  white-space: nowrap;
}
.seg button + button { border-left: 1px solid var(--border); }
.seg button:hover { color: var(--text); background: var(--surface-hover); }
.seg button.active { color: var(--text); background: var(--surface-active); box-shadow: inset 0 -2px 0 var(--accent); }
.summary { gap: var(--space-2); font-size: var(--text-sm); }
.sum-row { display: flex; align-items: center; justify-content: space-between; }
.strong { color: var(--text); font-weight: 600; }
.sum-note { font-size: var(--text-xs); }
.sum-note.warn { color: var(--warning); }
</style>
