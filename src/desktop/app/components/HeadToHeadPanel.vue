<template>
  <div class="h2h">
    <div v-if="loading && !data" class="muted pad">加载中…</div>
    <p v-else-if="error" class="form-error">{{ error }}</p>
    <template v-else-if="data">
      <div v-if="!data.together.matches && !data.against.matches" class="callout">
        本地库里 {{ data.a.displayName }} 和 {{ data.b.displayName }} 还没有同场过。
      </div>
      <template v-else>
        <!-- 战绩 -->
        <div class="tiles">
          <StatTile label="同队" :value="data.together.matches" unit="场" compact>
            <template #hint>
              <span :class="rateTone(data.together.winRate, 3, data.together.matches)">胜率 {{ fmtPct(data.together.winRate) }}</span>
              <span class="dim">{{ fmtRecord(data.together) }}</span>
            </template>
          </StatTile>
          <StatTile label="对手" :value="data.against.matches" unit="场" compact>
            <template #hint>
              <span :class="rateTone(data.against.winRate, 3, data.against.matches)">胜率 {{ fmtPct(data.against.winRate) }}</span>
              <span class="dim">{{ fmtRecord(data.against) }}</span>
            </template>
          </StatTile>
          <StatTile label="对枪胜率" :value="fmtPct(data.duels.winRate)" compact>
            <template #hint>
              <span>{{ data.duels.won }} 胜 {{ data.duels.lost }} 负</span>
              <span class="dim">共 {{ data.duels.total }} 回合</span>
            </template>
          </StatTile>
          <StatTile label="开局对枪" :value="fmtPct(data.openings.winRate)" compact>
            <template #hint>
              <span>{{ data.openings.won }} 胜 {{ data.openings.lost }} 负</span>
            </template>
          </StatTile>
        </div>

        <!-- 互杀 / 互伤：左右对比条 -->
        <div class="versus card">
          <div class="vs-head">
            <span class="strong truncate">{{ data.a.displayName }}</span>
            <span class="muted">对比</span>
            <span class="strong truncate right">{{ data.b.displayName }}</span>
          </div>
          <div v-for="row in vsRows" :key="row.label" class="vs-row">
            <span class="num vs-val">{{ row.a }}</span>
            <div class="vs-bar">
              <span class="vs-fill a" :style="{ width: `${row.pa}%` }" />
              <span class="vs-fill b" :style="{ width: `${100 - row.pa}%` }" />
            </div>
            <span class="num vs-val right">{{ row.b }}</span>
            <span class="vs-label muted">{{ row.label }}</span>
          </div>
          <p class="vs-note dim">只算两人对位：击杀 = {{ data.a.displayName }} 杀 {{ data.b.displayName }}；伤害为实际扣血，不含队友伤害和自杀。</p>
        </div>

        <!-- 同场对局 -->
        <SectionHeader title="同场对局" :count="data.matches.length" description="最近 50 场，点击查看比赛详情" />
        <div class="table-wrap">
          <table class="data-table compact">
            <thead>
              <tr>
                <th>时间</th>
                <th>地图</th>
                <th>平台</th>
                <th>关系</th>
                <th>结果</th>
                <th class="num">互杀</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="m in data.matches" :key="m.matchId" class="clickable" @click="navigateTo(`/matches/${m.matchId}`)">
                <td class="muted">{{ fmtDate(m.playedAt) }}</td>
                <td class="strong">{{ mapLabel(m.mapName) }}</td>
                <td><span class="tag">{{ platformLabel(m.platform) }}</span></td>
                <td>{{ m.sameTeam ? '同队' : '对手' }}</td>
                <td><ResultMark :result="m.result" /></td>
                <td class="num">
                  <template v-if="m.sameTeam"><span class="dim">—</span></template>
                  <template v-else>{{ m.kills }} : {{ m.deaths }}</template>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { HeadToHeadDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import { errorMessage, fmtDate, fmtPct, fmtRecord, mapLabel, platformLabel, rateTone } from '~/composables/usePlayers';

const props = defineProps<{ a: number; b: number; refreshKey?: number }>();

const data = ref<HeadToHeadDto | null>(null);
const loading = ref(false);
const error = ref('');

async function load() {
  loading.value = true;
  error.value = '';
  try {
    data.value = await tauriInvoke<HeadToHeadDto>('get_head_to_head', { a: props.a, b: props.b });
  } catch (e) {
    error.value = errorMessage(e);
    data.value = null;
  } finally {
    loading.value = false;
  }
}
watch(() => [props.a, props.b, props.refreshKey], load, { immediate: true });

const vsRows = computed(() => {
  const d = data.value;
  if (!d) return [];
  const pct = (x: number, y: number) => (x + y ? (x / (x + y)) * 100 : 50);
  return [
    { label: '击杀', a: d.kills, b: d.deaths, pa: pct(d.kills, d.deaths) },
    { label: '伤害', a: d.damage, b: d.damageTaken, pa: pct(d.damage, d.damageTaken) },
    { label: '对枪', a: d.duels.won, b: d.duels.lost, pa: pct(d.duels.won, d.duels.lost) },
    { label: '开局', a: d.openings.won, b: d.openings.lost, pa: pct(d.openings.won, d.openings.lost) },
  ];
});
</script>

<style scoped>
.h2h { display: flex; flex-direction: column; gap: var(--space-4); }
.pad { padding: var(--space-4) 0; font-size: var(--text-sm); }
.tiles { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: var(--space-3); }
.tiles :deep(.stat-hint) { gap: var(--space-2); }

.versus { padding: var(--space-3) var(--space-4); display: flex; flex-direction: column; gap: var(--space-2); }
.vs-head { display: grid; grid-template-columns: 1fr auto 1fr; gap: var(--space-3); font-size: var(--text-sm); }
.vs-head .muted { font-size: var(--text-xs); }
.right { text-align: right; }
.vs-row { display: grid; grid-template-columns: 56px 1fr 56px 36px; align-items: center; gap: var(--space-3); font-size: var(--text-sm); }
.vs-val { color: var(--text); }
.vs-label { font-size: var(--text-xs); }
.vs-bar { display: flex; height: 6px; gap: 2px; border-radius: var(--radius-full); overflow: hidden; }
.vs-fill { display: block; min-width: 2px; }
/* 左侧（本页视角）用一级文字色，右侧用三级灰；不引入额外彩色 */
.vs-fill.a { background: var(--text); }
.vs-fill.b { background: var(--border-strong); }
.vs-note { margin: 0; font-size: var(--text-xs); }
</style>
