<template>
  <div class="home">
    <!-- ── 没有主玩家：引导设置 ── -->
    <template v-if="mp.loaded.value && !mp.isSet.value">
      <section class="card card-pad setup">
        <SectionHeader title="先告诉我你是谁" size="lg" description="个人主页按「主玩家」统计。设置后这里会显示你的趋势、分项统计、个人最佳和重点改进。" />
        <div v-if="mp.suggestion.value" class="suggest">
          <div class="suggest-info">
            <span class="muted">看起来你是</span>
            <span class="suggest-name">{{ mp.suggestion.value.displayName ?? mp.suggestion.value.steamid }}</span>
            <span class="mono muted">{{ mp.suggestion.value.steamid }}</span>
            <span class="tag">{{ mainPlayerSourceLabel(mp.suggestion.value.source) }}</span>
            <span v-if="mp.suggestion.value.matchCount" class="muted num">{{ mp.suggestion.value.matchCount }} 场</span>
          </div>
          <button type="button" class="btn-primary" :disabled="adopting" @click="adopt">是我，设为主玩家</button>
        </div>
        <div v-else class="muted">还没法推测你是谁：先导入几场自己的对局，或者手动选择。</div>
        <div class="setup-actions">
          <NuxtLink to="/welcome" class="btn">打开首次启动向导</NuxtLink>
          <NuxtLink to="/settings" class="btn btn-ghost">到设置里手动选择</NuxtLink>
        </div>
        <div v-if="adoptError" class="form-error">{{ adoptError }}</div>
      </section>
    </template>

    <template v-else-if="mp.isSet.value">
      <!-- ── 头部 ── -->
      <section class="hero">
        <div class="hero-main">
          <div class="hero-name">{{ profile?.displayName ?? mp.displayName.value ?? mp.steamid.value }}</div>
          <div class="hero-sub">
            <span class="mono">{{ mp.steamid.value }}</span>
            <span v-if="profile && profile.steamids.length > 1" class="tag" :title="profile.steamids.join('\n')">
              {{ profile.steamids.length }} 个 Steam 身份
            </span>
            <NuxtLink :to="`/players/${mp.steamid.value}`" class="muted link">选手页 →</NuxtLink>
          </div>
        </div>
        <div class="hero-controls">
          <label class="ctl">
            <span class="ctl-label">范围</span>
            <select v-model.number="lastMatches" class="select" aria-label="统计范围">
              <option :value="0">全部对局</option>
              <option :value="20">最近 20 场</option>
              <option :value="50">最近 50 场</option>
              <option :value="100">最近 100 场</option>
            </select>
          </label>
          <label class="ctl">
            <span class="ctl-label">滑动窗口</span>
            <select v-model.number="windowN" class="select" aria-label="滑动平均窗口">
              <option v-for="n in WINDOWS" :key="n" :value="n">{{ n }} 场</option>
            </select>
          </label>
        </div>
      </section>

      <div v-if="error" class="callout callout-danger">{{ error }}</div>
      <div v-if="loading && !profile" class="empty">加载中…</div>

      <template v-if="profile">
        <div v-if="profile.summary.matches === 0" class="empty">
          主玩家还没有已解析的对局。导入或下载对局并解析完成后，这里会自动更新。
        </div>

        <template v-else>
          <!-- 概览数字 + 平台分数 -->
          <section class="tiles">
            <StatTile label="场次" :value="profile.summary.matches" compact>
              <template #hint>{{ profile.summary.rounds }} 回合</template>
            </StatTile>
            <StatTile label="胜率" :value="profile.summary.winRate == null ? '—' : profile.summary.winRate.toFixed(0)" unit="%" compact>
              <template #hint>{{ profile.summary.wins }} 胜 {{ profile.summary.losses }} 负<template v-if="profile.summary.draws"> {{ profile.summary.draws }} 平</template></template>
            </StatTile>
            <StatTile label="K / D" :value="`${profile.summary.kills} / ${profile.summary.deaths}`" compact>
              <template #hint>{{ fmtMetric('kd', metric('kd')?.value) }}</template>
            </StatTile>
            <div v-for="s in profile.scores" :key="s.system" class="score-tile">
              <div class="score-head">
                <span class="score-label">{{ s.label }}</span>
                <span class="muted num">最高 {{ s.best }}</span>
              </div>
              <div class="score-body">
                <span class="score-value num">{{ s.latest }}</span>
                <ProfileSparkline :points="s.points" :label="`${s.label}历史`" />
              </div>
              <div class="score-foot muted">{{ s.points.length }} 场有分数 · 最近 {{ shortDate(s.points[s.points.length - 1]?.at) }}</div>
            </div>
            <div v-if="!profile.scores.length" class="score-tile score-none">
              <div class="score-label">平台分数</div>
              <div class="muted">库里的 demo 没有记录你的 Premier / 竞技段位分数</div>
            </div>
          </section>

          <!-- 重点改进 -->
          <section class="block">
            <SectionHeader title="重点改进" size="lg" :description="focusDesc" />
            <div v-if="profile.focus.length" class="focus-grid">
              <div v-for="f in profile.focus" :key="f.key" class="focus-card">
                <div class="focus-head">
                  <MetricHelp :metric="f.key" :label="f.name" />
                  <span class="focus-pct num bad">P{{ f.percentile.toFixed(0) }}</span>
                </div>
                <div class="focus-value num">
                  {{ fmtMetric(f.key, f.value) }}<span class="unit">{{ metric(f.key)?.unit }}</span>
                </div>
                <div class="focus-sub muted">
                  {{ scopeLabel(f.scope) }} {{ f.sampleSize }} 人的中位数 {{ fmtMetric(f.key, f.sampleMedian) }}{{ metric(f.key)?.unit }}
                </div>
                <div class="focus-actions">
                  <NuxtLink :to="MISTAKES_LINK" class="btn btn-sm">看最近 30 天的失误集锦 →</NuxtLink>
                  <button type="button" class="btn btn-ghost btn-sm" @click="trendKey = f.key">看趋势</button>
                </div>
              </div>
            </div>
            <div v-else class="empty small">{{ focusEmpty }}</div>
          </section>

          <!-- 关键指标 + 百分位 -->
          <section class="block">
            <SectionHeader title="关键指标" size="lg" :description="sampleDesc" />
            <div class="metric-grid">
              <button
                v-for="m in profile.metrics"
                :key="m.key"
                type="button"
                class="metric-card"
                :class="{ selected: trendKey === m.key }"
                @click="trendKey = m.key"
              >
                <div class="mc-head">
                  <MetricHelp :metric="m.key" :label="m.name" />
                  <span v-if="m.category === 'aim'" class="tag">枪法</span>
                </div>
                <div class="mc-value num">
                  <template v-if="m.value != null">{{ fmtMetric(m.key, m.value) }}<span class="unit">{{ m.unit }}</span></template>
                  <span v-else class="na">不可用</span>
                </div>
                <div class="mc-recent muted">
                  <template v-if="m.recent != null && profile.summary.matches > profile.window">
                    最近 {{ profile.window }} 场 <span class="num" :class="deltaClass(m)">{{ fmtMetric(m.key, m.recent) }}</span>
                  </template>
                  <template v-else-if="m.value != null">{{ m.matchesWithData }} 场有数据</template>
                  <template v-else>{{ naReason(m.key) }}</template>
                </div>
                <PercentileBar :p="m.percentile" :available="m.value != null" />
              </button>
            </div>
          </section>

          <!-- 趋势 -->
          <section class="block">
            <SectionHeader title="趋势" size="lg" :description="`每场一个点，折线是最近 ${profile.window} 场（有数据的）合并计算的滑动平均；点击某一场打开比赛详情`">
              <MetricHelp v-if="trendMetric" :metric="trendMetric.key" />
            </SectionHeader>
            <div class="seg trend-tabs">
              <button
                v-for="m in profile.metrics"
                :key="m.key"
                type="button"
                class="seg-btn"
                :class="{ on: trendKey === m.key, dim: m.value == null }"
                @click="trendKey = m.key"
              >
                {{ m.name }}
              </button>
            </div>
            <div v-if="trendMetric && trendSeries" class="card card-pad">
              <ProfileTrendChart
                :metric="trendMetric.key"
                :label="trendMetric.name"
                :unit="trendMetric.unit"
                :window="profile.window"
                :matches="profile.trend.matches"
                :values="trendSeries.values"
                :rolling="trendSeries.rolling"
                :median="trendMetric.percentile.scope !== 'none' ? trendMetric.percentile.sampleMedian : null"
                @open="(id) => navigateTo(`/matches/${encodeURIComponent(id)}`)"
              />
            </div>
          </section>

          <!-- 分项 -->
          <section class="split">
            <div class="block">
              <SectionHeader title="T / CT" description="按回合统计（只算你参加的回合）" />
              <div class="table-wrap">
                <table class="data-table compact">
                  <thead>
                    <tr>
                      <th>阵营</th>
                      <th class="num">回合</th>
                      <th class="num">回合胜率</th>
                      <th class="num">K/D <MetricHelp metric="kd" /></th>
                      <th class="num">ADR <MetricHelp metric="adr" /></th>
                      <th class="num">KAST <MetricHelp metric="kast" /></th>
                      <th class="num">首杀 <MetricHelp metric="openingDuel" /></th>
                      <th class="num">Swing <MetricHelp metric="swing" /></th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="s in profile.sides" :key="s.side">
                      <td><SideBadge :side="s.side" /></td>
                      <td class="num">{{ s.rounds }}</td>
                      <td class="num">{{ fmtRate(s.roundWinRate) }}</td>
                      <td class="num">{{ fmtMetric('kd', s.kd) }}</td>
                      <td class="num">{{ fmtMetric('adr', s.adr) }}</td>
                      <td class="num">{{ fmtRate(s.kast, 1) }}</td>
                      <td class="num">{{ s.openingKills }}–{{ s.openingDeaths }} <span class="muted">{{ fmtRate(s.openingRate) }}</span></td>
                      <td class="num">{{ s.swing == null ? '不可用' : fmtMetric('swing', s.swing) }}</td>
                    </tr>
                    <tr v-if="!profile.sides.length"><td colspan="8" class="empty">暂无数据</td></tr>
                  </tbody>
                </table>
              </div>
            </div>

            <div class="block">
              <SectionHeader title="地图" :count="profile.maps.length" />
              <div class="table-wrap">
                <table class="data-table compact">
                  <thead>
                    <tr>
                      <th>地图</th>
                      <th class="num">场次</th>
                      <th class="num">胜率</th>
                      <th class="num">ADR</th>
                      <th class="num">KAST</th>
                      <th class="num">HLTV 2.0 <MetricHelp metric="rating" /></th>
                      <th class="num">Swing</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="m in profile.maps" :key="m.mapName">
                      <td class="strong">{{ mapShort(m.mapName) }}</td>
                      <td class="num">{{ m.matches }} <span class="muted">{{ m.wins }}-{{ m.losses }}<template v-if="m.draws">-{{ m.draws }}</template></span></td>
                      <td class="num">{{ fmtRate(m.winRate) }}</td>
                      <td class="num">{{ fmtMetric('adr', m.adr) }}</td>
                      <td class="num">{{ fmtRate(m.kast, 1) }}</td>
                      <td class="num">{{ fmtMetric('rating', m.rating) }}</td>
                      <td class="num">{{ m.swing == null ? '不可用' : fmtMetric('swing', m.swing) }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </section>

          <section class="block">
            <SectionHeader title="武器" :count="profile.weapons.length" description="击杀来自击杀事件；开枪和命中来自枪法分析（刀和道具没有命中率）" />
            <div class="table-wrap weapons">
              <table class="data-table compact">
                <thead>
                  <tr>
                    <th>武器</th>
                    <th class="num">击杀</th>
                    <th class="num">爆头击杀 <MetricHelp metric="hsPct" /></th>
                    <th class="num">开枪</th>
                    <th class="num">命中率 <MetricHelp metric="accuracy" /></th>
                    <th class="num">爆头命中 <MetricHelp metric="headAccuracy" /></th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="w in profile.weapons" :key="w.weapon">
                    <td class="strong">{{ weaponName(w.weapon) }}</td>
                    <td class="num">{{ w.kills }}</td>
                    <td class="num">{{ w.headshotKills }} <span class="muted">{{ fmtRate(w.hsPct) }}</span></td>
                    <td class="num">{{ w.shots || '—' }}</td>
                    <td class="num">{{ fmtRate(w.accuracy, 1) }}</td>
                    <td class="num">{{ fmtRate(w.headHitRate) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </section>

          <!-- 个人最佳 -->
          <section class="block">
            <SectionHeader title="个人最佳" size="lg" description="点击打开那一场的比赛详情" />
            <div class="best-grid">
              <NuxtLink v-for="b in profile.bests" :key="b.key" :to="`/matches/${encodeURIComponent(b.matchId)}`" class="best-card">
                <div class="best-head">
                  <span class="best-label">{{ b.label }}</span>
                  <MetricHelp :metric="b.metric" />
                </div>
                <div class="best-value num">{{ fmtBest(b) }}</div>
                <div class="best-sub muted">
                  {{ mapShort(b.mapName) }} · {{ b.scoreOwn }}:{{ b.scoreOpp }}
                  <ResultMark :result="b.result" />
                  <span v-if="shortDate(b.at)">· {{ shortDate(b.at) }}</span>
                </div>
              </NuxtLink>
            </div>
          </section>
        </template>
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { PersonalBestDto, ProfileDto, ProfileMetricDto, ProfileMetricKey } from '@cs-analyst/shared-types';
import { listen } from '@tauri-apps/api/event';
import { mainPlayerSourceLabel, useMainPlayer } from '~/composables/useMainPlayer';
import { weaponName } from '~/composables/useAim';
import {
  MISTAKES_LINK,
  fetchProfile,
  fmtMetric,
  fmtRate,
  mapShort,
  scopeLabel,
  shortDate,
} from '~/composables/useProfile';

definePageMeta({ layout: 'default' });

const WINDOWS = [3, 5, 10, 20, 30];

const mp = useMainPlayer();
const profile = shallowRef<ProfileDto | null>(null);
const loading = ref(false);
const error = ref('');
const windowN = ref(10);
const lastMatches = ref(0);
const trendKey = ref<ProfileMetricKey>('rating');
const adopting = ref(false);
const adoptError = ref('');

let seq = 0;
async function load() {
  const sid = mp.steamid.value;
  if (!sid) {
    profile.value = null;
    return;
  }
  const my = ++seq;
  loading.value = true;
  error.value = '';
  try {
    const p = await fetchProfile(sid, { window: windowN.value, lastMatches: lastMatches.value || undefined });
    if (my === seq) profile.value = p;
  } catch (e) {
    if (my === seq) error.value = `加载个人主页失败：${String(e)}`;
  } finally {
    if (my === seq) loading.value = false;
  }
}

watch([mp.steamid, windowN, lastMatches], () => void load());
onMounted(async () => {
  await mp.ready;
  await load();
});
// 新对局解析完：稍后刷新
let refreshTimer: ReturnType<typeof setTimeout> | null = null;
let unlisten: (() => void) | null = null;
listen<{ status: string }>('analysis_updated', (e) => {
  if (e.payload.status !== 'done') return;
  if (refreshTimer) clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => void load(), 2000);
})
  .then((fn) => (unlisten = fn))
  .catch(() => {});
onBeforeUnmount(() => {
  unlisten?.();
  if (refreshTimer) clearTimeout(refreshTimer);
});

async function adopt() {
  const s = mp.suggestion.value;
  if (!s) return;
  adopting.value = true;
  adoptError.value = '';
  try {
    await mp.set(s.steamid);
  } catch (e) {
    adoptError.value = String(e);
  } finally {
    adopting.value = false;
  }
}

function metric(key: ProfileMetricKey): ProfileMetricDto | undefined {
  return profile.value?.metrics.find((m) => m.key === key);
}
const trendMetric = computed(() => metric(trendKey.value));
const trendSeries = computed(() => profile.value?.trend.series.find((s) => s.key === trendKey.value));

function deltaClass(m: ProfileMetricDto) {
  if (m.value == null || m.recent == null) return '';
  const d = m.recent - m.value;
  if (Math.abs(d) < 1e-9) return '';
  return d > 0 === m.higherIsBetter ? 'good' : 'bad';
}

function naReason(key: ProfileMetricKey) {
  if (key === 'ttd' || key === 'crosshairPlacement') return '没有可见性可靠的对局（如 ESL demo）';
  if (key === 'counterStrafe' || key === 'sprayError') return '没有枪法分析数据';
  if (key === 'clutch') return '没有打过残局';
  if (key === 'openingDuel') return '没有参与开局对枪';
  return '没有数据';
}

function fmtBest(b: PersonalBestDto) {
  switch (b.key) {
    case 'kills':
    case 'headshotKills':
      return String(b.value);
    case 'multiKill':
      return `${b.value}K`;
    case 'clutch':
      return `1v${b.value}`;
    case 'adr':
      return fmtMetric('adr', b.value);
    case 'rating':
      return fmtMetric('rating', b.value);
    case 'swing':
      return fmtMetric('swing', b.value);
    case 'ttd':
      return `${fmtMetric('ttd', b.value)} ms`;
  }
}

const sampleDesc = computed(() => {
  const p = profile.value;
  if (!p) return '';
  const s = p.sample;
  const tier = p.tier
    ? `段位：${p.tier.systemLabel} ${p.tier.score}（${p.tier.bandLabel}）`
    : '库里没有你的平台分数，无法分段位';
  let pool: string;
  if (p.tier && s.tierPlayers >= s.minTierSample) pool = `百分位按同段位 ${s.tierPlayers} 名选手计算`;
  else if (s.libraryPlayers >= s.minSample)
    pool = `${p.tier ? `同段位只有 ${s.tierPlayers} 人（少于 ${s.minTierSample}），` : ''}百分位改用全库 ${s.libraryPlayers} 名选手`;
  else pool = `全库只有 ${s.libraryPlayers} 名其他选手（少于 ${s.minSample}），暂不计算百分位`;
  const low = s.libraryPlayers < s.confidentSample ? `；样本少于 ${s.confidentSample} 人时只作参考` : '';
  return `${tier}。${pool}${low}。P50 = 样本中位数。`;
});

const focusDesc = computed(() => '百分位最低（且低于样本中位数）的 2–3 项，附带对应的失误集锦');
const focusEmpty = computed(() => {
  const p = profile.value;
  if (!p) return '';
  if (!p.metrics.some((m) => m.percentile.value != null)) return '样本太少，还没法和其他选手比较。多解析几场对局后再来看。';
  return '所有有百分位的指标都不低于样本中位数。';
});
</script>

<style scoped>
.home { display: flex; flex-direction: column; gap: var(--space-6); padding-bottom: var(--space-8); }
.block { display: flex; flex-direction: column; min-width: 0; }
.empty { padding: var(--space-6); text-align: center; font-size: var(--text-sm); color: var(--text-3); background: var(--surface); border: 1px solid var(--border-subtle); border-radius: var(--radius-lg); }
.empty.small { padding: var(--space-4); }
.link { text-decoration: none; }
.link:hover { color: var(--text); }
.unit { margin-left: 2px; font-size: var(--text-sm); font-weight: 500; color: var(--text-3); }
.na { font-size: var(--text-md); font-weight: 500; color: var(--text-disabled); }

/* 未设置主玩家 */
.setup { display: flex; flex-direction: column; gap: var(--space-4); max-width: 760px; }
.suggest { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); padding: var(--space-3) var(--space-4); border: 1px solid var(--accent-line); border-radius: var(--radius-lg); background: var(--accent-soft); flex-wrap: wrap; }
.suggest-info { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
.suggest-name { font-weight: 600; color: var(--text); }
.setup-actions { display: flex; gap: var(--space-2); }

/* 头部 */
.hero { display: flex; align-items: flex-end; justify-content: space-between; gap: var(--space-4); flex-wrap: wrap; }
.hero-name { font-size: var(--text-2xl); font-weight: 600; color: var(--text); letter-spacing: -0.01em; }
.hero-sub { display: flex; align-items: center; gap: var(--space-2); margin-top: 2px; font-size: var(--text-xs); color: var(--text-3); }
.hero-controls { display: flex; gap: var(--space-3); }
.ctl { display: flex; align-items: center; gap: var(--space-2); }
.ctl-label { font-size: var(--text-xs); color: var(--text-3); }

.tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: var(--space-3); }
.score-tile { display: flex; flex-direction: column; gap: 4px; padding: var(--space-2) var(--space-3); background: var(--surface); border: 1px solid var(--border-subtle); border-radius: var(--radius-lg); min-width: 0; }
.score-head { display: flex; justify-content: space-between; font-size: var(--text-xs); }
.score-label { color: var(--text-3); }
.score-body { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); }
.score-value { font-size: var(--text-xl); font-weight: 600; color: var(--text); }
.score-foot { font-size: var(--text-xs); }
.score-none { justify-content: center; font-size: var(--text-xs); }

/* 重点改进 */
.focus-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: var(--space-3); }
.focus-card { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-3) var(--space-4); background: var(--surface); border: 1px solid var(--border-subtle); border-left: 2px solid var(--danger-line); border-radius: var(--radius-lg); }
.focus-head { display: flex; align-items: center; justify-content: space-between; font-size: var(--text-sm); color: var(--text-2); }
.focus-pct { font-size: var(--text-sm); font-weight: 600; }
.focus-value { font-size: var(--text-xl); font-weight: 600; color: var(--text); }
.focus-sub { font-size: var(--text-xs); }
.focus-actions { display: flex; gap: var(--space-2); flex-wrap: wrap; }

/* 指标卡 */
.metric-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: var(--space-3); }
.metric-card { position: relative; display: flex; flex-direction: column; gap: 6px; padding: var(--space-3); text-align: left; background: var(--surface); border: 1px solid var(--border-subtle); border-radius: var(--radius-lg); cursor: pointer; color: inherit; font: inherit; }
.metric-card:hover { border-color: var(--border-strong); }
.metric-card.selected { border-color: var(--accent-line); background: var(--surface-active); }
.mc-head { display: flex; align-items: center; justify-content: space-between; font-size: var(--text-xs); color: var(--text-3); }
.mc-value { font-size: var(--text-xl); font-weight: 600; color: var(--text); line-height: 1.15; }
.mc-recent { font-size: var(--text-xs); min-height: 16px; }

/* 趋势 */
.trend-tabs { flex-wrap: wrap; margin-bottom: var(--space-3); align-self: flex-start; }
.seg { display: inline-flex; padding: 2px; background: var(--bg-sunken); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); }
.seg-btn { height: 22px; padding: 0 var(--space-2); border: none; border-radius: var(--radius-sm); background: transparent; color: var(--text-3); font-size: var(--text-xs); cursor: pointer; }
.seg-btn:hover { color: var(--text); }
.seg-btn.on { background: var(--surface-active); color: var(--text); }
.seg-btn.dim { color: var(--text-disabled); }

.split { display: grid; grid-template-columns: repeat(auto-fit, minmax(420px, 1fr)); gap: var(--space-5); }
.weapons { max-height: 360px; overflow: auto; }

/* 个人最佳 */
.best-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: var(--space-3); }
.best-card { display: flex; flex-direction: column; gap: 4px; padding: var(--space-3); background: var(--surface); border: 1px solid var(--border-subtle); border-radius: var(--radius-lg); text-decoration: none; color: inherit; }
.best-card:hover { border-color: var(--border-strong); background: var(--surface-hover); }
.best-head { display: flex; align-items: center; justify-content: space-between; font-size: var(--text-xs); color: var(--text-3); }
.best-value { font-size: var(--text-xl); font-weight: 600; color: var(--text); }
.best-sub { display: flex; align-items: center; gap: 4px; font-size: var(--text-xs); }
</style>
