<template>
  <div class="hl-page">
    <!-- 工具栏：选手 / 时间 / 地图 -->
    <div class="toolbar">
      <div class="who">
        <span class="who-label">选手</span>
        <span class="who-name">{{ playerName }}</span>
        <span v-if="isMain" class="tag">我</span>
        <button v-else-if="mainSteamid" class="btn btn-sm" type="button" @click="steamid = mainSteamid">回到我</button>
        <PersonPicker class="picker" small placeholder="切换选手：名字或 SteamID" @pick="pickPerson" />
      </div>
      <div class="toolbar-right">
        <select v-model.number="days" class="select" aria-label="时间范围">
          <option v-for="r in RANGES" :key="r.days" :value="r.days">{{ r.label }}</option>
        </select>
        <select v-model="map" class="select" aria-label="地图">
          <option value="">全部地图</option>
          <option v-for="m in mapOptions" :key="m" :value="m">{{ mapLabel(m) }}</option>
        </select>
      </div>
    </div>

    <div v-if="!steamid && mainLoaded" class="callout callout-warning">
      还没有设置主玩家。到 <NuxtLink to="/settings">设置</NuxtLink> 里选择「我是谁」，或在上面搜索一名选手。
    </div>
    <div v-if="matchId" class="callout callout-info scope">
      <span>只看这一场：<span class="strong">{{ matchTitle }}</span></span>
      <span class="scope-actions">
        <button class="btn btn-sm" type="button" @click="allPlayers = !allPlayers">{{ allPlayers ? `只看 ${playerName}` : '看全部选手' }}</button>
        <NuxtLink :to="`/matches/${matchId}`" class="btn btn-sm">比赛详情</NuxtLink>
        <button class="btn btn-sm" type="button" @click="matchId = ''">看全部对局</button>
      </span>
    </div>
    <div v-if="error" class="callout callout-danger">{{ error }}</div>
    <div v-if="list?.pending" class="callout callout-info">
      还有 <span class="num">{{ list.pending }}</span> 场对局的集锦在后台计算，稍后刷新即可看到。
    </div>

    <!-- 概览 -->
    <div class="tiles">
      <StatTile label="高光" :value="highlightTotal" compact />
      <StatTile label="失误" :value="mistakeTotal" compact />
      <StatTile label="对局" :value="list?.matches ?? '—'" compact />
      <StatTile label="当前列表" :value="list?.total ?? '—'" compact :hint="list && list.total > list.items.length ? `显示前 ${list.items.length} 条` : undefined" />
    </div>

    <!-- 类型筛选 -->
    <section class="block">
      <div class="seg" role="tablist">
        <button
          v-for="c in CATEGORIES"
          :key="c.key"
          type="button"
          class="seg-item"
          :class="{ active: category === c.key }"
          @click="setCategory(c.key)"
        >
          {{ c.label }}
        </button>
      </div>
      <div class="kinds">
        <button
          v-for="k in visibleKinds"
          :key="k.kind"
          type="button"
          class="kind-chip"
          :class="{ active: kinds.includes(k.kind), zero: !countOf(k.kind), off: disabledKinds.has(k.kind) }"
          :title="disabledKinds.has(k.kind) ? '这条规则在设置里关掉了' : k.definition"
          @click="toggleKind(k.kind)"
        >
          <span class="kind-dot" :class="k.category" />
          <span>{{ k.label }}</span>
          <span class="num kind-count">{{ countOf(k.kind) }}</span>
          <MetricHelp :metric="`highlight.${k.kind}`" side="bottom" />
        </button>
        <button v-if="kinds.length" type="button" class="btn-ghost btn-sm" @click="kinds = []">清除类型</button>
      </div>
    </section>

    <!-- 列表 -->
    <section class="block">
      <SectionHeader :title="matchId ? '本场集锦' : '集锦'" :count="list?.total" :description="listDesc">
        <NuxtLink to="/settings#highlights" class="btn btn-sm">规则设置</NuxtLink>
      </SectionHeader>
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th v-if="!matchId">对局</th>
              <th class="num">回合</th>
              <th>类型</th>
              <th>经过</th>
              <th class="num">分数 <MetricHelp metric="highlight.score" /></th>
              <th class="actions"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loading && !list"><td :colspan="matchId ? 5 : 6" class="empty">加载中…</td></tr>
            <tr v-else-if="list && !list.items.length">
              <td :colspan="matchId ? 5 : 6" class="empty">{{ emptyText }}</td>
            </tr>
            <tr v-for="h in list?.items ?? []" :key="h.id" :class="{ selected: playing?.id === h.id }">
              <td v-if="!matchId">
                <div class="match-cell">
                  <NuxtLink :to="`/matches/${h.matchId}`" class="strong">{{ mapLabel(h.mapName) }}</NuxtLink>
                  <span class="muted num">{{ fmtDate(h.playedAt) }}</span>
                </div>
              </td>
              <td class="num">
                <span class="round">
                  <SideBadge v-if="h.side" :side="h.side" />
                  {{ h.round }}
                </span>
              </td>
              <td>
                <span class="kind-cell">
                  <span class="kind-dot" :class="h.category" />
                  <span class="strong">{{ highlightLabel(h.kind) }}</span>
                  <MetricHelp :metric="`highlight.${h.kind}`" />
                </span>
              </td>
              <td class="desc">
                <span v-if="h.steamid !== steamid" class="who-prefix">{{ h.playerName ?? h.steamid }}：</span>{{ describeHighlight(h, list!.names) }}
              </td>
              <td class="num">{{ h.weightedScore.toFixed(0) }}</td>
              <td class="actions">
                <button
                  class="btn btn-sm"
                  type="button"
                  :disabled="h.demoStorage === 'deleted'"
                  :title="h.demoStorage === 'deleted' ? '原始 demo 已删除，无法播放' : '在 CS2 中播放'"
                  @click="playing = h"
                >
                  {{ h.demoStorage === 'deleted' ? '不可播放' : '播放' }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <HighlightsPlayModal :item="playing" :netcon-enabled="netconEnabled" @close="playing = null" />
  </div>
</template>

<script setup lang="ts">
import type { HighlightCategory, HighlightKind, HighlightList, MatchDto, PersonSummaryDto } from '@cs-analyst/shared-types';
import { describeHighlight, getHighlightSettings, listHighlights } from '~/composables/useHighlights';
import { mapLabel } from '~/composables/useMatchDetail';
import { tauriInvoke } from '~/composables/useTauri';

const route = useRoute();
const router = useRouter();
const { steamid: mainSteamid, displayName: mainName, loaded: mainLoaded, ready } = useMainPlayer();

// ── 筛选（和 URL 查询参数同步：?steamid= &type= &days= &map= &match=） ──
const RANGES = [
  { days: 0, label: '全部时间' },
  { days: 7, label: '最近 7 天' },
  { days: 30, label: '最近 30 天' },
  { days: 90, label: '最近 90 天' },
  { days: 365, label: '最近一年' },
];
const CATEGORIES: { key: 'all' | HighlightCategory; label: string }[] = [
  { key: 'all', label: '全部' },
  { key: 'highlight', label: '高光' },
  { key: 'mistake', label: '失误' },
];

const q = (k: string) => (typeof route.query[k] === 'string' ? (route.query[k] as string) : '');
const steamid = ref('');
const days = ref(0);
const map = ref('');
const matchId = ref('');
const category = ref<'all' | HighlightCategory>('all');
const kinds = ref<HighlightKind[]>([]);
/** 单场时可以看全部选手 */
const allPlayers = ref(false);

/** `type` 可以是 highlight / mistake，也可以是具体类型（例如 multi_kill），逗号分隔多个 */
function setKinds(ks: HighlightKind[]) {
  if (ks.join(',') !== kinds.value.join(',')) kinds.value = ks;
}

function applyQuery() {
  const t = q('type');
  if (t === 'highlight' || t === 'mistake') {
    category.value = t;
    setKinds([]);
  } else if (t) {
    const ks = t.split(',').filter((k) => HIGHLIGHT_KIND_MAP[k]) as HighlightKind[];
    setKinds(ks);
    const cats = new Set(ks.map((k) => HIGHLIGHT_KIND_MAP[k]!.category));
    category.value = cats.size === 1 ? [...cats][0]! : 'all';
  } else {
    category.value = 'all';
    setKinds([]);
  }
  const d = Number(q('days'));
  days.value = Number.isFinite(d) && d > 0 ? d : 0;
  if (days.value && !RANGES.some((r) => r.days === days.value)) {
    RANGES.push({ days: days.value, label: `最近 ${days.value} 天` });
    RANGES.sort((a, b) => a.days - b.days);
  }
  map.value = q('map');
  matchId.value = q('match');
  allPlayers.value = !!matchId.value && q('all') === '1';
  if (q('steamid')) steamid.value = q('steamid');
}

function toQuery() {
  const out: Record<string, string> = {};
  if (steamid.value && steamid.value !== mainSteamid.value) out.steamid = steamid.value;
  if (kinds.value.length) out.type = kinds.value.join(',');
  else if (category.value !== 'all') out.type = category.value;
  if (days.value) out.days = String(days.value);
  if (map.value) out.map = map.value;
  if (matchId.value) out.match = matchId.value;
  if (matchId.value && allPlayers.value) out.all = '1';
  return out;
}

function setCategory(c: 'all' | HighlightCategory) {
  category.value = c;
  setKinds(kinds.value.filter((k) => c === 'all' || HIGHLIGHT_KIND_MAP[k]?.category === c));
}

function toggleKind(k: HighlightKind) {
  kinds.value = kinds.value.includes(k) ? kinds.value.filter((x) => x !== k) : [...kinds.value, k];
}

function pickPerson(p: PersonSummaryDto) {
  if (p.steamids[0]) steamid.value = p.steamids[0];
}

// ── 数据 ──
const list = ref<HighlightList | null>(null);
const loading = ref(false);
const error = ref('');
const disabledKinds = ref(new Set<string>());
const netconEnabled = ref(false);
const playing = ref<HighlightList['items'][number] | null>(null);
const pickedName = ref<string | null>(null);
const matchInfo = ref<MatchDto | null>(null);

const isMain = computed(() => !!steamid.value && steamid.value === mainSteamid.value);
const playerName = computed(() => {
  if (!steamid.value) return '未选择';
  if (isMain.value && mainName.value) return mainName.value;
  return list.value?.names[steamid.value] ?? pickedName.value ?? steamid.value;
});
const mapOptions = computed(() => {
  const ms = [...(list.value?.maps ?? [])];
  if (map.value && !ms.includes(map.value)) ms.unshift(map.value);
  return ms;
});
const visibleKinds = computed(() => HIGHLIGHT_KINDS.filter((k) => category.value === 'all' || k.category === category.value));
const countOf = (k: string) => list.value?.counts.find((c) => c.kind === k)?.count ?? 0;
const sumCat = (c: HighlightCategory) => (list.value ? list.value.counts.filter((x) => x.category === c).reduce((a, b) => a + b.count, 0) : '—');
const highlightTotal = computed(() => sumCat('highlight'));
const mistakeTotal = computed(() => sumCat('mistake'));
const matchTitle = computed(() => {
  const m = matchInfo.value;
  if (!m) return matchId.value;
  return `${mapLabel(m.mapName)}${m.playedAt ? ` · ${fmtDate(m.playedAt)}` : ''}`;
});
const listDesc = computed(() => (matchId.value ? '按回合顺序' : '按分数（原始分 × 规则权重）从高到低'));
const emptyText = computed(() => {
  if (!steamid.value) return '先选择一名选手';
  if (kinds.value.length || category.value !== 'all') return '当前筛选下没有集锦';
  return '没有找到集锦。对局解析完成后会自动识别。';
});

function fmtDate(iso: string | null | undefined) {
  if (!iso) return '—';
  const d = new Date(iso.includes('T') || iso.endsWith('Z') ? iso : `${iso.replace(' ', 'T')}Z`);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' });
}

let seq = 0;
async function load() {
  if (!steamid.value) {
    list.value = null;
    return;
  }
  const my = ++seq;
  loading.value = true;
  error.value = '';
  try {
    const res = await listHighlights({
      steamid: allPlayers.value && matchId.value ? null : steamid.value,
      matchId: matchId.value || null,
      category: category.value === 'all' ? null : category.value,
      kinds: kinds.value.length ? kinds.value : null,
      maps: map.value ? [map.value] : null,
      days: days.value || null,
      limit: 500,
    });
    if (my === seq) list.value = res;
  } catch (e) {
    if (my === seq) error.value = `读取集锦失败：${e}`;
  } finally {
    if (my === seq) loading.value = false;
  }
}

async function loadSettings() {
  try {
    const s = await getHighlightSettings();
    disabledKinds.value = new Set(s.rules.filter((r) => !r.enabled).map((r) => r.kind));
    netconEnabled.value = s.netconEnabled;
  } catch {
    /* 浏览器里没有 Tauri */
  }
}

async function loadMatchInfo() {
  matchInfo.value = null;
  if (!matchId.value) return;
  try {
    const all = await tauriInvoke<MatchDto[]>('list_matches');
    matchInfo.value = all.find((m) => m.id === matchId.value) ?? null;
  } catch {
    /* ignore */
  }
}

watch([steamid, days, map, matchId, category, kinds, allPlayers], () => {
  const next = toQuery();
  const cur = Object.fromEntries(Object.entries(route.query).filter(([, v]) => typeof v === 'string')) as Record<string, string>;
  if (JSON.stringify(next) !== JSON.stringify(cur)) router.replace({ query: next });
  load();
});
watch(matchId, loadMatchInfo);
// 侧边栏 / 个人主页跳进来时（同一页面内改查询参数）
// （改了的筛选会触发上面的 watch 重新加载）
watch(() => route.query, () => {
  applyQuery();
  if (!q('steamid') && mainSteamid.value) steamid.value = mainSteamid.value;
});
watch(mainSteamid, (id, old) => {
  if (!q('steamid') && (!steamid.value || steamid.value === old) && id) steamid.value = id;
});

onMounted(async () => {
  applyQuery();
  loadSettings();
  loadMatchInfo();
  if (!steamid.value) {
    await ready;
    steamid.value = mainSteamid.value ?? '';
  }
  if (steamid.value) load();
});
</script>

<style scoped>
.hl-page { display: flex; flex-direction: column; gap: var(--space-5); padding-bottom: var(--space-8); }
.toolbar { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex-wrap: wrap; }
.toolbar-right { display: flex; gap: var(--space-2); }
.who { display: flex; align-items: center; gap: var(--space-2); min-width: 0; flex-wrap: wrap; }
.who-label { font-size: var(--text-xs); color: var(--text-3); }
.who-name { font-size: var(--text-md); font-weight: 600; color: var(--text); }
.picker { width: 240px; }
.scope { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
.scope-actions { display: flex; gap: var(--space-2); }
.tiles { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: var(--space-3); }
.block { display: flex; flex-direction: column; gap: var(--space-3); min-width: 0; }

.seg { display: inline-flex; align-self: flex-start; border: 1px solid var(--border); border-radius: var(--radius-md); overflow: hidden; }
.seg-item {
  height: var(--control-h-sm);
  padding: 0 var(--space-4);
  font-size: var(--text-sm);
  color: var(--text-2);
  background: transparent;
  border: none;
  border-right: 1px solid var(--border);
  cursor: pointer;
}
.seg-item:last-child { border-right: none; }
.seg-item:hover { background: var(--surface-hover); color: var(--text); }
.seg-item.active { background: var(--surface-active); color: var(--text); font-weight: 600; box-shadow: inset 0 -2px 0 var(--accent); }

.kinds { display: flex; flex-wrap: wrap; gap: var(--space-2); align-items: center; }
.kind-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 var(--space-2);
  font-size: var(--text-sm);
  color: var(--text-2);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  cursor: pointer;
}
.kind-chip:hover { border-color: var(--border-strong); color: var(--text); }
.kind-chip.active { border-color: var(--accent-line); background: var(--accent-soft); color: var(--text); }
.kind-chip.zero { color: var(--text-3); }
.kind-chip.off { opacity: 0.5; text-decoration: line-through; }
.kind-count { color: var(--text-3); font-size: var(--text-xs); }
.kind-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
.kind-dot.highlight { background: var(--value-good); }
.kind-dot.mistake { background: var(--value-bad); }

.match-cell { display: flex; align-items: baseline; gap: var(--space-2); white-space: nowrap; }
.match-cell .muted { font-size: var(--text-xs); }
.round { display: inline-flex; align-items: center; gap: 6px; justify-content: flex-end; }
.kind-cell { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
.desc { color: var(--text-2); min-width: 280px; }
.who-prefix { color: var(--text); font-weight: 500; }
td.empty { padding: var(--space-6); text-align: center; color: var(--text-3); }
</style>
