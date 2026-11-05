<template>
  <div class="aim-summary" :class="{ compact }">
    <div v-if="loading && !data" class="as-empty">加载中…</div>
    <div v-else-if="error" class="callout callout-danger">{{ error }}</div>
    <div v-else-if="data && data.matches === 0" class="as-empty">这个范围内没有已完成枪法分析的对局。</div>
    <template v-else-if="data">
      <div v-if="unreliable > 0" class="callout callout-info vis-note">
        <template v-if="data.reliableMatches === 0">
          {{ data.matches === 1 ? '这场对局' : `这 ${data.matches} 场对局` }}没有可靠的可见性数据（例如 ESL 等职业服务器 demo），
          TTD、反应时间、准星偏移、可见时命中率和首发命中率不可用。其余指标照常计算。
        </template>
        <template v-else>
          {{ data.matches }} 场中有 {{ unreliable }} 场可见性数据不可靠，TTD、反应时间、准星偏移、可见时命中率和首发命中率只来自其余 {{ data.reliableMatches }} 场。
        </template>
      </div>

      <!-- 依赖可见性的指标：带分布图 -->
      <div class="as-grid as-grid-vis">
        <AimMetric
          label="Time to Damage"
          :help="`${AIM_HELP.ttd}\n${AIM_HELP.bias}`"
          :value="fmtNum(o.ttd.median)"
          unit="ms"
          :unavailable="noVis || o.ttd.n === 0"
          :unavailable-text="noVis ? AIM_HELP.unavailable : '样本不足。'"
          :sub="`均值 ${fmtNum(o.ttd.mean)} ms · ${o.ttd.n} 次接触`"
          :compact="compact"
        >
          <AimHistogram :hist="o.ttdHistogram" :median="o.ttd.median" unit="ms" />
        </AimMetric>

        <AimMetric
          label="反应时间"
          :help="AIM_HELP.reaction"
          :value="fmtNum(o.reaction.median)"
          unit="ms"
          :unavailable="noVis || o.reaction.n === 0"
          :unavailable-text="noVis ? AIM_HELP.unavailable : '样本不足。'"
          :sub="`均值 ${fmtNum(o.reaction.mean)} ms · ${o.reaction.n} 次`"
          :compact="compact"
        >
          <AimDistBox :dist="o.reaction" unit="" :max="reactionMax" />
        </AimMetric>

        <AimMetric
          label="准星偏移"
          :help="`${AIM_HELP.crosshair}\n${AIM_HELP.bias}`"
          help-align="right"
          :value="fmtNum(o.crosshair.median, 1)"
          unit="°"
          :unavailable="noVis || o.crosshair.n === 0"
          :unavailable-text="noVis ? AIM_HELP.unavailable : '样本不足。'"
          :sub="`均值 ${fmtDeg(o.crosshair.mean)} · ${o.crosshair.n} 次`"
          :compact="compact"
        >
          <AimHistogram :hist="o.crosshairHistogram" :median="o.crosshair.median" unit="°" />
</AimMetric>

        <AimMetric
          label="垂直偏移"
          :help="`${AIM_HELP.crosshair}
这里是带符号的垂直分量：正数 = 准星在头上方，负数 = 在头下方。`"
          :value="fmtSigned(o.crosshairV.median)"
          unit="°"
          :unavailable="noVis || o.crosshairV.n === 0"
          :unavailable-text="noVis ? AIM_HELP.unavailable : '样本不足。'"
          :sub="`${vNote} · |垂直| 中位 ${fmtDeg(o.crosshairVAbs.median)}`"
          :compact="compact"
        >
          <AimDistBox :dist="o.crosshairV" signed :digits="1" :max="crossMax" unit="°" />
        </AimMetric>

        <AimMetric
          label="水平偏移"
          :help="`${AIM_HELP.crosshair}
这里是水平分量的绝对值（左右偏了多少）。`"
          help-align="right"
          :value="fmtNum(o.crosshairHAbs.median, 1)"
          unit="°"
          :unavailable="noVis || o.crosshairHAbs.n === 0"
          :unavailable-text="noVis ? AIM_HELP.unavailable : '样本不足。'"
          :sub="`均值 ${fmtDeg(o.crosshairHAbs.mean)} · 左右偏差`"
          :compact="compact"
        >
          <AimDistBox :dist="o.crosshairHAbs" :digits="1" :max="crossMax" unit="°" />
        </AimMetric>
      </div>

      <!-- 比率类指标 -->
      <div class="as-grid as-grid-rates">
        <AimMetric
          label="急停率"
          :help="AIM_HELP.counterStrafe"
          :value="fmtPct(o.counterStrafeRate)"
          unit="%"
          :sub="`${o.csOk} / ${o.csShots} 首发`"
          :compact="true"
        />
        <AimMetric
          label="可见时命中率"
          :help="AIM_HELP.visibleAccuracy"
          :value="fmtPct(o.visibleAccuracy)"
          unit="%"
          :unavailable="noVis"
          :sub="`${o.visHits} / ${o.visShots} 发`"
          :compact="true"
        />
        <AimMetric
          label="首发命中率"
          :help="AIM_HELP.firstShot"
          :value="fmtPct(o.firstShotAccuracy)"
          unit="%"
          :unavailable="noVis"
          :sub="`${o.firstHits} / ${o.firstShots} 发`"
          :compact="true"
        />
        <AimMetric
          label="爆头率"
          :help="AIM_HELP.headshot"
          :value="fmtPct(o.headshotRate)"
          unit="%"
          :sub="`${o.headHits} / ${o.hits} 命中`"
          :compact="true"
        />
        <AimMetric
          label="命中率"
          :help="AIM_HELP.accuracy"
          :value="fmtPct(o.accuracy)"
          unit="%"
          :sub="`${o.hits} / ${o.shots} 发`"
          :compact="true"
        />
        <AimMetric
          label="压枪控制"
          :help="AIM_HELP.sprayControl"
          help-align="right"
          :value="fmtNum(o.sprayControl)"
          :sub="o.sprays ? `${o.sprays} 次连发 · 平均误差 ${fmtDeg(o.sprayErr)}` : '没有连发'"
          :compact="true"
        />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * 枪法总览：TTD / 反应时间 / 准星偏移（含垂直、水平）带分布图，下面一行是比率类指标。
 *
 * 用法：
 * - 直接传数据：<AimSummary :stats="aimStats" />
 * - 自己取数据（比赛详情页）：<AimSummary :steamid="sid" :match-id="matchId" compact />
 *
 * 可见性不可靠的对局（ESL 职业 demo 等）上，依赖可见性的指标显示「不可用」，不显示 0。
 */
import type { AimStats } from '@cs-analyst/shared-types';
import { AIM_HELP, fmtDeg, fmtNum, fmtPct, getAimStats } from '~/composables/useAim';

const props = withDefaults(defineProps<{
  stats?: AimStats | null;
  steamid?: string | null;
  matchId?: string | null;
  weapon?: string | null;
  compact?: boolean;
}>(), {
  stats: null,
  steamid: null,
  matchId: null,
  weapon: null,
  compact: false,
});

const fetched = ref<AimStats | null>(null);
const loading = ref(false);
const error = ref('');

const data = computed(() => props.stats ?? fetched.value);
const o = computed(() => data.value!.overall);
const noVis = computed(() => (data.value?.reliableMatches ?? 0) === 0);
const unreliable = computed(() => (data.value ? data.value.matches - data.value.reliableMatches : 0));

async function load() {
  if (props.stats || !props.steamid) return;
  loading.value = true;
  error.value = '';
  try {
    fetched.value = await getAimStats(props.steamid, {
      matchIds: props.matchId ? [props.matchId] : undefined,
      weapon: props.weapon ?? undefined,
    });
  } catch (e) {
    error.value = `读取枪法数据失败：${e}`;
  } finally {
    loading.value = false;
  }
}

watch(() => [props.steamid, props.matchId, props.weapon, props.stats], load, { immediate: true });

const reactionMax = computed(() => {
  const p75 = o.value.reaction.p75 ?? 0;
  return p75 > 0 ? Math.max(500, Math.ceil((p75 * 1.8) / 100) * 100) : 1000;
});
const crossMax = computed(() => {
  const v = Math.max(Math.abs(o.value.crosshairV.p75 ?? 0), Math.abs(o.value.crosshairV.p25 ?? 0), o.value.crosshairHAbs.p75 ?? 0);
  return v > 0 ? Math.max(5, Math.ceil(v * 1.6)) : 10;
});

function fmtSigned(v: number | null) {
  if (v == null) return '—';
  return `${v > 0 ? '+' : ''}${v.toFixed(1)}`;
}

const vNote = computed(() => {
  const v = o.value.crosshairV.median;
  if (v == null) return '';
  if (v > 0.5) return '多半偏高';
  if (v < -0.5) return '多半偏低';
  return '高度基本对齐';
});
</script>

<style scoped>
.aim-summary { display: flex; flex-direction: column; gap: var(--space-3); }
.as-empty { padding: var(--space-6); text-align: center; font-size: var(--text-sm); color: var(--text-3); }
.vis-note { font-size: var(--text-xs); }
.as-grid { display: grid; gap: var(--space-3); }
.as-grid-vis { grid-template-columns: repeat(5, minmax(0, 1fr)); }
.as-grid-rates { grid-template-columns: repeat(6, minmax(0, 1fr)); }
@media (max-width: 1100px) {
  .as-grid-vis { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .as-grid-rates { grid-template-columns: repeat(3, minmax(0, 1fr)); }
}
.compact.aim-summary { gap: var(--space-2); }
.compact .as-grid { gap: var(--space-2); }

</style>
