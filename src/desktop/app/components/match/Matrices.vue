<template>
  <div class="matrices">
    <SectionHeader :title="metric === 'damage' ? '伤害矩阵' : '对枪矩阵'" :description="desc" size="lg">
      <div class="seg">
        <button class="seg-item" :class="{ active: metric === 'damage' }" @click="metric = 'damage'">伤害</button>
        <button class="seg-item" :class="{ active: metric === 'duel' }" @click="metric = 'duel'">击杀</button>
      </div>
      <button class="btn btn-sm" title="交换行列" @click="rowTeam = rowTeam === 'A' ? 'B' : 'A'">
        行：{{ teamLabel(rowTeam, mineTeam === rowTeam) }} ⇄
      </button>
    </SectionHeader>

    <div class="table-wrap matrix-wrap">
      <table class="matrix">
        <thead>
          <tr>
            <th class="corner">
              <span class="muted">{{ teamLabel(rowTeam, mineTeam === rowTeam) }} ↓ · {{ teamLabel(colTeam, mineTeam === colTeam) }} →</span>
            </th>
            <th v-for="c in cols" :key="c.steamid" class="col-h" :class="{ me: mine.has(c.steamid) }">
              <NuxtLink :to="`/players/${c.steamid}`" class="plink" :title="c.name">{{ c.name }}</NuxtLink>
            </th>
            <th class="col-h total-h">合计</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rows" :key="r.steamid">
            <th class="row-h" :class="{ me: mine.has(r.steamid) }">
              <NuxtLink :to="`/players/${r.steamid}`" class="plink" :title="r.name">{{ r.name }}</NuxtLink>
            </th>
            <td
              v-for="c in cols"
              :key="c.steamid"
              class="cell"
              :style="heat(val(r.steamid, c.steamid))"
              :title="cellTitle(r.name, c.name, r.steamid, c.steamid)"
            >
              <span class="main num">{{ val(r.steamid, c.steamid) || '·' }}</span>
              <span class="sub num">{{ val(c.steamid, r.steamid) || '' }}</span>
            </td>
            <td class="cell total num">{{ rowTotal(r.steamid) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { MatchDetailDto, SteamId, TeamId } from '@cs-analyst/shared-types';
import { teamLabel } from '~/composables/useMatchDetail';

const props = defineProps<{ detail: MatchDetailDto; mine: Set<SteamId>; mineTeam: TeamId | null }>();

const metric = ref<'damage' | 'duel'>('damage');
const rowTeam = ref<TeamId>(props.mineTeam ?? 'A');
const colTeam = computed<TeamId>(() => (rowTeam.value === 'A' ? 'B' : 'A'));

const desc = computed(() => metric.value === 'damage'
  ? '大数字：行玩家对列玩家造成的有效伤害；小数字：反方向。颜色越亮数值越大'
  : '大数字：行玩家击杀列玩家的次数；小数字：反方向。颜色越亮数值越大');

const players = (t: TeamId) => props.detail.scoreboard
  .filter(p => p.team === t)
  .sort((a, b) => b.derived.rating - a.derived.rating)
  .map(p => ({ steamid: p.steamid, name: p.name || p.steamid }));
const rows = computed(() => players(rowTeam.value));
const cols = computed(() => players(colTeam.value));

const dmg = computed(() => new Map(props.detail.damageMatrix.map(c => [`${c.attacker}>${c.victim}`, c])));
const duel = computed(() => new Map(props.detail.duelMatrix.map(c => [`${c.attacker}>${c.victim}`, c])));

function val(a: SteamId, v: SteamId) {
  const k = `${a}>${v}`;
  return metric.value === 'damage' ? (dmg.value.get(k)?.damage ?? 0) : (duel.value.get(k)?.kills ?? 0);
}

const maxVal = computed(() => {
  let m = 0;
  for (const r of rows.value) for (const c of cols.value) m = Math.max(m, val(r.steamid, c.steamid));
  return m || 1;
});

function rowTotal(a: SteamId) { return cols.value.reduce((s, c) => s + val(a, c.steamid), 0); }

/** 灰阶热度：在表面色上叠一层透明度随数值变化的一级文字色。 */
function heat(v: number) {
  if (v <= 0) return {};
  const t = v / maxVal.value;
  // 上限 40%：最亮的格子仍是深灰，白字和小字都保持足够对比度
  const p = Math.round(5 + t * 35);
  return {
    background: `color-mix(in srgb, var(--text) ${p}%, transparent)`,
    '--cell-text': 'var(--text)',
    '--cell-sub': t > 0.35 ? 'var(--text-2)' : 'var(--text-3)',
  } as Record<string, string>;
}

function cellTitle(rn: string, cn: string, r: SteamId, c: SteamId) {
  if (metric.value === 'damage') {
    const f = dmg.value.get(`${r}>${c}`);
    const b = dmg.value.get(`${c}>${r}`);
    return `${rn} → ${cn}：${f?.damage ?? 0} 伤害（${f?.hits ?? 0} 次命中）\n${cn} → ${rn}：${b?.damage ?? 0} 伤害（${b?.hits ?? 0} 次命中）`;
  }
  const f = duel.value.get(`${r}>${c}`);
  const b = duel.value.get(`${c}>${r}`);
  return `${rn} 击杀 ${cn}：${f?.kills ?? 0} 次（爆头 ${f?.headshotKills ?? 0}，首杀 ${f?.openingKills ?? 0}）\n${cn} 击杀 ${rn}：${b?.kills ?? 0} 次`;
}
</script>

<style scoped>
.seg { display: inline-flex; padding: 2px; gap: 2px; background: var(--bg-sunken); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); }
.seg-item { height: 22px; padding: 0 var(--space-3); border: 0; border-radius: var(--radius-sm); background: transparent; font-size: var(--text-sm); color: var(--text-3); cursor: pointer; }
.seg-item:hover { color: var(--text); }
.seg-item.active { background: var(--surface-raised); color: var(--text); box-shadow: inset 0 -2px 0 var(--accent); }

.matrix-wrap { overflow-x: auto; }
.matrix { border-collapse: separate; border-spacing: 2px; font-size: var(--text-sm); }
.corner { text-align: left; font-weight: 400; font-size: var(--text-xs); padding: 0 var(--space-2); white-space: nowrap; }
.col-h {
  width: 84px; max-width: 84px; padding: var(--space-1) var(--space-1) var(--space-2);
  font-weight: 500; font-size: var(--text-xs); color: var(--text-2); text-align: center;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.total-h { color: var(--text-3); }
.row-h {
  max-width: 160px; padding: 0 var(--space-3) 0 var(--space-2);
  text-align: left; font-weight: 500; color: var(--text-2);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.me { box-shadow: inset 2px 0 0 var(--accent); }
.col-h.me { box-shadow: inset 0 -2px 0 var(--accent); }
.plink { color: inherit; text-decoration: none; }
.plink:hover { color: var(--text); text-decoration: underline; text-underline-offset: 2px; }

.cell {
  --cell-text: var(--text-disabled); --cell-sub: var(--text-3);
  position: relative; width: 84px; height: 40px;
  background: var(--bg-sunken); border-radius: var(--radius-sm);
  text-align: center; cursor: default;
}
.main { display: block; font-size: var(--text-md); font-weight: 600; color: var(--cell-text); line-height: 1.1; }
.sub { display: block; font-size: 10px; color: var(--cell-sub); line-height: 1.2; min-height: 12px; }
.total { background: transparent; color: var(--text-2); font-weight: 600; }
</style>
