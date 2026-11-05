<template>
  <div class="aim-page">
    <!-- 工具栏：选手 / 武器 / 时间范围 -->
    <div class="toolbar">
      <div class="who">
        <span class="who-label">选手</span>
        <select v-if="persons.length > 1" v-model="steamid" class="select">
          <option v-for="p in persons" :key="p.steamids[0]" :value="p.steamids[0]">{{ p.displayName }}</option>
        </select>
        <span v-else class="who-name">{{ personName }}</span>
        <span class="mono muted">{{ steamid }}</span>
      </div>
      <div class="toolbar-right">
        <select v-model="weapon" class="select" aria-label="武器">
          <option value="">全部武器</option>
          <option v-for="w in weaponOptions" :key="w" :value="w">{{ weaponName(w) }}</option>
        </select>
        <select v-model="range" class="select" aria-label="时间范围">
          <option v-for="r in RANGES" :key="r.key" :value="r.key">{{ r.label }}</option>
        </select>
      </div>
    </div>

    <div v-if="usingFallback" class="callout callout-warning">
      还没有「我关注的人」，暂时显示默认账号 <span class="mono">{{ FALLBACK_STEAMID }}</span>。
      到 <NuxtLink to="/players">选手</NuxtLink> 页找到自己并点「关注」，这里就会默认显示你。
    </div>
    <div v-if="error" class="callout callout-danger">{{ error }}</div>

    <!-- 枪法总览 -->
    <section class="block">
      <SectionHeader
        title="枪法总览"
        size="lg"
        :description="overviewDesc"
      />
      <AimSummary v-if="stats" :stats="stats" />
      <div v-else-if="loading" class="empty">加载中…</div>
    </section>

    <!-- 武器 -->
    <section v-if="stats && stats.weapons.length" class="block">
      <SectionHeader title="按武器" size="lg" :count="stats.weapons.length" description="点击可压枪分析的武器，查看它的平均弹道" />
      <div class="table-wrap">
        <table class="data-table compact">
          <thead>
            <tr>
              <th>武器</th>
              <th class="num">射击</th>
              <th class="num">命中率</th>
              <th class="num">爆头率</th>
              <th class="num">可见时命中</th>
              <th class="num">首发命中</th>
              <th class="num">急停率</th>
              <th class="num">TTD</th>
              <th class="num">准星偏移</th>
              <th class="num">连发</th>
              <th class="num">压枪控制</th>
              <th class="num">开始崩</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="w in stats.weapons"
              :key="w.weapon"
              :class="{ clickable: isSprayWeapon(w.weapon), selected: w.weapon === sprayWeapon }"
              @click="isSprayWeapon(w.weapon) && pickSprayWeapon(w.weapon)"
            >
              <td class="strong">{{ weaponName(w.weapon) }}</td>
              <td class="num">{{ w.shots }}</td>
              <td class="num">{{ pct(w.accuracy) }}</td>
              <td class="num">{{ pct(w.headshotRate) }}</td>
              <td class="num">{{ visPct(w.visibleAccuracy) }}</td>
              <td class="num">{{ visPct(w.firstShotAccuracy) }}</td>
              <td class="num">{{ pct(w.counterStrafeRate) }}</td>
              <td class="num">{{ vis(w.ttdMedianMs, (v) => `${v.toFixed(0)} ms`) }}</td>
              <td class="num">{{ vis(w.crosshairMedianDeg, (v) => `${v.toFixed(1)}°`) }}</td>
              <td class="num">{{ w.sprays || '—' }}</td>
              <td class="num">{{ fmtNum(w.sprayControl) }}</td>
              <td class="num">{{ w.breakShot ? `第 ${w.breakShot} 发` : '—' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <!-- 压枪分析 -->
    <section class="block">
      <SectionHeader title="压枪分析" size="lg" :description="sprayDesc">
        <AimHelp title="怎么读" align="right">
          <p>以第一发为原点。后坐力（−2 × Δpunch）决定了不压枪时子弹会去哪里，把它镜像过来就是理想的压枪轨迹。</p>
          <p>垂直误差 &gt; 0 表示子弹偏高（压少了），&lt; 0 表示压多了；水平误差 &gt; 0 表示偏右。</p>
          <p>跟移动中的目标会让水平误差变大，这部分是有意的跟枪，不一定是没压住。</p>
        </AimHelp>
      </SectionHeader>
      <div class="weapon-tabs" role="tablist">
        <button
          v-for="w in SPRAY_WEAPONS"
          :key="w"
          type="button"
          role="tab"
          class="wtab"
          :class="{ on: w === sprayWeapon, empty: !patternOf(w) }"
          :aria-selected="w === sprayWeapon"
          @click="pickSprayWeapon(w)"
        >
          {{ weaponName(w) }}
          <span class="num wtab-n">{{ patternOf(w)?.sprays ?? 0 }}</span>
        </button>
      </div>
      <div v-if="spraysLoading && !sprayRes" class="empty">加载中…</div>
      <AimSprayPattern v-else-if="currentPattern && currentPattern.sprays > 0" :key="sprayWeapon" :pattern="currentPattern" />
      <div v-else class="empty">{{ weaponName(sprayWeapon) }} 在这个范围内没有连发（至少连续 4 发才算一次连发）。</div>
    </section>

    <!-- 趋势 -->
    <section v-if="stats && stats.trend.length" class="block">
      <SectionHeader title="趋势" size="lg" :description="`${weaponName(weapon || null)} · 每场一个点，从旧到新`">
        <div class="seg">
          <button
            v-for="m in TREND_METRICS"
            :key="m.key"
            type="button"
            class="seg-btn"
            :class="{ on: trendMetric === m.key }"
            @click="trendMetric = m.key"
          >
            {{ m.label }}
          </button>
        </div>
      </SectionHeader>
      <div class="card card-pad">
        <AimTrendChart
          :points="stats.trend"
          :metric="trendDef.key"
          :label="trendDef.label"
          :unit="trendDef.unit"
          :percent="trendDef.percent"
        />
      </div>
    </section>

    <!-- 单次连发 -->
    <section class="block">
      <SectionHeader title="单次连发" size="lg" :description="`按对局浏览每一次连发${sprayFilterWeapon ? `（${weaponName(sprayFilterWeapon)}）` : ''}`" />
      <div class="browser">
        <div class="table-wrap match-list">
          <table class="data-table compact">
            <thead>
              <tr><th>对局</th><th class="num">连发</th><th class="num">控制</th></tr>
            </thead>
            <tbody>
              <tr
                v-for="m in sprayMatches"
                :key="m.matchId"
                class="clickable"
                :class="{ selected: m.matchId === browseMatch }"
                @click="browseMatch = m.matchId"
              >
                <td class="truncate">
                  <span class="strong">{{ matchLabel(m) }}</span>
                  <span class="tag">{{ m.platform }}</span>
                </td>
                <td class="num">{{ m.sprays }}</td>
                <td class="num">{{ fmtNum(m.sprayControl) }}</td>
              </tr>
              <tr v-if="!sprayMatches.length"><td colspan="3" class="empty">没有连发</td></tr>
            </tbody>
          </table>
        </div>

        <div class="table-wrap spray-list">
          <table class="data-table compact">
            <thead>
              <tr>
                <th class="num">回合</th>
                <th>武器</th>
                <th class="num">发数</th>
                <th class="num">命中</th>
                <th class="num">控制</th>
                <th class="num">误差</th>
                <th class="num">开始崩</th>
                <th>结果</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="s in matchSprays"
                :key="s.id"
                class="clickable"
                :class="{ selected: s.id === browseSprayId }"
                @click="browseSprayId = s.id"
              >
                <td class="num">{{ s.round }}</td>
                <td class="strong">{{ weaponName(s.weapon) }}</td>
                <td class="num">{{ s.shots.length }}</td>
                <td class="num">{{ s.hits }}</td>
                <td class="num">{{ fmtNum(s.control) }}</td>
                <td class="num" :class="{ bad: s.meanErr > BREAK_DEG }">{{ s.meanErr.toFixed(2) }}°</td>
                <td class="num">{{ s.breakShot ?? '—' }}</td>
                <td><StatusChip v-if="s.killed" tone="success" label="击杀" /><span v-else class="dim">—</span></td>
              </tr>
              <tr v-if="browseLoading"><td colspan="8" class="empty">加载中…</td></tr>
              <tr v-else-if="browseMatch && !matchSprays.length"><td colspan="8" class="empty">这场没有连发</td></tr>
              <tr v-else-if="!browseMatch"><td colspan="8" class="empty">在左侧选一场对局</td></tr>
            </tbody>
          </table>
        </div>
      </div>
      <div v-if="browseSpray" class="card card-pad spray-detail-card">
        <AimSprayDetail :key="browseSpray.id" :spray="browseSpray" />
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import type { AimFilters, AimStats, AimTrendPoint, PersonSummaryDto, Spray, SpraysResult, WeaponPattern } from '@cs-analyst/shared-types';
import {
  aggregateSprays,
  BREAK_DEG,
  FALLBACK_STEAMID,
  fmtNum,
  fmtPct,
  getAimStats,
  getSprays,
  isSprayWeapon,
  listFollowedPersons,
  matchLabel,
  SPRAY_WEAPONS,
  weaponName,
} from '~/composables/useAim';

const route = useRoute();

// ── 选手 ──
const persons = ref<PersonSummaryDto[]>([]);
const steamid = ref<string>('');
const usingFallback = ref(false);
const personName = computed(() => {
  const p = persons.value.find((x) => x.steamids.includes(steamid.value));
  return p?.displayName ?? (usingFallback.value ? '默认账号' : '未关注的选手');
});

// ── 筛选 ──
const RANGES = [
  { key: 'all', label: '全部对局' },
  { key: 'last5', label: '最近 5 场' },
  { key: 'last10', label: '最近 10 场' },
  { key: 'last20', label: '最近 20 场' },
  { key: 'd30', label: '最近 30 天' },
  { key: 'd90', label: '最近 90 天' },
] as const;
type RangeKey = (typeof RANGES)[number]['key'];
const range = ref<RangeKey>('all');
const weapon = ref<string>('');

function rangeFilters(): AimFilters {
  const r = range.value;
  if (r.startsWith('last')) return { lastMatches: Number(r.slice(4)) };
  if (r.startsWith('d')) {
    const d = new Date(Date.now() - Number(r.slice(1)) * 86400_000);
    return { from: d.toISOString() };
  }
  return {};
}

// ── 数据 ──
const stats = ref<AimStats | null>(null);
/** 全部武器的统计，用来填武器下拉（stats.weapons 本身就忽略武器筛选） */
const loading = ref(false);
const error = ref('');

const weaponOptions = computed(() => {
  const ws = stats.value?.weapons.map((w) => w.weapon) ?? [];
  if (weapon.value && !ws.includes(weapon.value)) ws.unshift(weapon.value);
  return ws;
});

let statsSeq = 0;
async function loadStats() {
  if (!steamid.value) return;
  const seq = ++statsSeq;
  loading.value = true;
  error.value = '';
  try {
    const res = await getAimStats(steamid.value, { ...rangeFilters(), weapon: weapon.value || undefined });
    if (seq === statsSeq) stats.value = res;
  } catch (e) {
    if (seq === statsSeq) error.value = `读取枪法数据失败：${e}`;
  } finally {
    if (seq === statsSeq) loading.value = false;
  }
}

const overviewDesc = computed(() => {
  const s = stats.value;
  if (!s) return '';
  const parts = [weaponName(weapon.value || null), `${s.matches} 场对局`];
  if (s.matches !== s.reliableMatches) parts.push(`其中 ${s.reliableMatches} 场有可靠的可见性数据`);
  return parts.join(' · ');
});

// 表格里依赖可见性的列：没有可靠对局时写「不可用」
function pct(v: number | null) {
  return v == null ? '—' : `${fmtPct(v)}%`;
}
function vis<T extends number | null>(v: T, f: (x: number) => string) {
  if ((stats.value?.reliableMatches ?? 0) === 0) return '不可用';
  return v == null ? '—' : f(v);
}
function visPct(v: number | null) {
  return vis(v, (x) => `${fmtPct(x)}%`);
}

// ── 压枪 ──
const sprayRes = ref<SpraysResult | null>(null);
const spraysLoading = ref(false);
const sprayWeapon = ref<string>('ak47');
let sprayWeaponPinned = false;

/** 时间范围内的对局（来自 trend），用于在客户端筛选连发 */
const rangeMatchIds = computed(() => new Set((stats.value?.trend ?? []).map((t) => t.matchId)));

const patterns = computed<WeaponPattern[]>(() => {
  const res = sprayRes.value;
  if (!res) return [];
  if (range.value === 'all') return res.patterns;
  // get_sprays 没有时间筛选：按 trend 里的对局过滤后在前端重新聚合
  const ids = rangeMatchIds.value;
  const byW = new Map<string, Spray[]>();
  for (const s of res.sprays) {
    if (!ids.has(s.matchId)) continue;
    const arr = byW.get(s.weapon) ?? [];
    arr.push(s);
    byW.set(s.weapon, arr);
  }
  return [...byW.entries()].map(([w, ss]) => aggregateSprays(w, ss));
});

function patternOf(w: string) {
  const p = patterns.value.find((x) => x.weapon === w);
  return p && p.sprays > 0 ? p : null;
}
const currentPattern = computed(() => patternOf(sprayWeapon.value));

const sprayDesc = computed(() => {
  const t = sprayRes.value?.truncated && range.value !== 'all' ? '（只取了最近 500 次连发）' : '';
  return `选武器查看平均弹道和逐发误差${t}`;
});

function pickSprayWeapon(w: string) {
  sprayWeapon.value = w;
  sprayWeaponPinned = true;
}

async function loadSprays() {
  if (!steamid.value) return;
  spraysLoading.value = true;
  try {
    sprayRes.value = await getSprays(steamid.value);
  } catch (e) {
    error.value = `读取压枪数据失败：${e}`;
  } finally {
    spraysLoading.value = false;
  }
}

// 默认武器：全局筛选是压枪武器就用它，否则用连发最多的那把
watch([patterns, weapon], () => {
  if (isSprayWeapon(weapon.value)) {
    sprayWeapon.value = weapon.value;
    return;
  }
  if (sprayWeaponPinned && patternOf(sprayWeapon.value)) return;
  const best = [...patterns.value].filter((p) => isSprayWeapon(p.weapon)).sort((a, b) => b.sprays - a.sprays)[0];
  if (best) sprayWeapon.value = best.weapon;
});

// ── 趋势 ──
const TREND_METRICS = [
  { key: 'sprayControl', label: '压枪控制', unit: '', percent: false },
  { key: 'accuracy', label: '命中率', unit: '%', percent: true },
  { key: 'counterStrafeRate', label: '急停率', unit: '%', percent: true },
  { key: 'ttdMedianMs', label: 'TTD', unit: ' ms', percent: false },
  { key: 'reactionMedianMs', label: '反应时间', unit: ' ms', percent: false },
  { key: 'crosshairMedianDeg', label: '准星偏移', unit: '°', percent: false },
] as const;
const trendMetric = ref<(typeof TREND_METRICS)[number]['key']>('sprayControl');
const trendDef = computed(() => TREND_METRICS.find((m) => m.key === trendMetric.value)!);

// ── 单次连发浏览 ──
const sprayFilterWeapon = computed(() => (isSprayWeapon(weapon.value) ? weapon.value : null));
const sprayMatches = computed<AimTrendPoint[]>(() =>
  [...(stats.value?.trend ?? [])].filter((t) => t.sprays > 0).reverse(),
);
const browseMatch = ref<string | null>(null);
const browseSprays = ref<Spray[]>([]);
const browseLoading = ref(false);
const browseSprayId = ref<number | null>(null);
const matchSprays = computed(() => browseSprays.value);
const browseSpray = computed(() => browseSprays.value.find((s) => s.id === browseSprayId.value) ?? null);

let browseSeq = 0;
watch([browseMatch, sprayFilterWeapon, steamid], async () => {
  browseSprayId.value = null;
  browseSprays.value = [];
  if (!browseMatch.value || !steamid.value) return;
  const seq = ++browseSeq;
  browseLoading.value = true;
  try {
    const res = await getSprays(steamid.value, { matchId: browseMatch.value, weapon: sprayFilterWeapon.value });
    if (seq !== browseSeq) return;
    browseSprays.value = res.sprays;
    // 默认打开最长的一次
    const longest = [...res.sprays].sort((a, b) => b.shots.length - a.shots.length)[0];
    browseSprayId.value = longest?.id ?? null;
  } catch (e) {
    if (seq === browseSeq) error.value = `读取连发失败：${e}`;
  } finally {
    if (seq === browseSeq) browseLoading.value = false;
  }
});

watch(sprayMatches, (ms) => {
  if (!ms.some((m) => m.matchId === browseMatch.value)) browseMatch.value = ms[0]?.matchId ?? null;
});

// ── 初始化 ──
watch([steamid, range, weapon], loadStats);
watch(steamid, () => {
  sprayWeaponPinned = false;
  loadSprays();
});

// 同一页面内改 ?steamid= 也要切换
watch(() => route.query.steamid, (q) => {
  if (typeof q === 'string' && q && q !== steamid.value) {
    usingFallback.value = false;
    steamid.value = q;
  }
});

onMounted(async () => {
  const q = typeof route.query.steamid === 'string' ? route.query.steamid : '';
  try {
    persons.value = (await listFollowedPersons()).filter((p) => p.steamids.length > 0);
  } catch {
    persons.value = [];
  }
  if (q) {
    steamid.value = q;
  } else if (persons.value.length) {
    steamid.value = persons.value[0]!.steamids[0]!;
  } else {
    usingFallback.value = true;
    steamid.value = FALLBACK_STEAMID;
  }
});
</script>

<style scoped>
.aim-page { display: flex; flex-direction: column; gap: var(--space-6); padding-bottom: var(--space-8); }
.toolbar { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex-wrap: wrap; }
.toolbar-right { display: flex; gap: var(--space-2); }
.who { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
.who-label { font-size: var(--text-xs); color: var(--text-3); }
.who-name { font-size: var(--text-md); font-weight: 600; color: var(--text); }
.block { display: flex; flex-direction: column; min-width: 0; }
.empty { padding: var(--space-6); text-align: center; font-size: var(--text-sm); color: var(--text-3); background: var(--surface); border: 1px solid var(--border-subtle); border-radius: var(--radius-lg); }

.weapon-tabs { display: flex; flex-wrap: wrap; gap: var(--space-1); margin-bottom: var(--space-3); }
.wtab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: var(--control-h-sm);
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--text-2);
  font-size: var(--text-xs);
  cursor: pointer;
}
.wtab:hover { border-color: var(--border-strong); color: var(--text); }
.wtab.on { background: var(--surface-active); border-color: var(--accent-line); color: var(--text); }
.wtab.empty { color: var(--text-disabled); }
.wtab-n { color: var(--text-3); }

.seg { display: inline-flex; padding: 2px; background: var(--bg-sunken); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); }
.seg-btn { height: 22px; padding: 0 var(--space-2); border: none; border-radius: var(--radius-sm); background: transparent; color: var(--text-3); font-size: var(--text-xs); cursor: pointer; }
.seg-btn:hover { color: var(--text); }
.seg-btn.on { background: var(--surface-active); color: var(--text); }

.browser { display: grid; grid-template-columns: minmax(220px, 300px) minmax(0, 1fr); gap: var(--space-3); align-items: start; }
.match-list, .spray-list { max-height: 300px; overflow: auto; }
.match-list .tag { margin-left: var(--space-2); }
.spray-detail-card { margin-top: var(--space-3); }
</style>
