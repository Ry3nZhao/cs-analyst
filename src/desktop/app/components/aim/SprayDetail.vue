<template>
  <div class="spray-detail">
    <div class="sd-head">
      <div class="sd-title">
        <span class="strong">{{ weaponName(spray.weapon) }}</span>
        <SideBadge v-if="spray.team === 2 || spray.team === 3" :side="spray.team" />
        <span class="muted num">第 {{ spray.round }} 回合 · {{ spray.shots.length }} 发 · 命中 {{ spray.hits }}（爆头 {{ spray.headHits }}）</span>
        <StatusChip v-if="spray.killed" tone="success" label="击杀" />
      </div>
      <div class="sd-stats num">
        <span>控制 <b>{{ fmtNum(spray.control) }}</b></span>
        <span>平均误差 <b>{{ fmtDeg(spray.meanErr) }}</b></span>
        <span>垂直 <b>{{ signed(spray.meanErrV) }}</b> <i :class="vClass">{{ verticalVerdict(spray.meanErrV) }}</i></span>
        <span>水平 <b>{{ signed(spray.meanErrH) }}</b></span>
        <span v-if="spray.startSpeed != null">首发速度 <b>{{ fmtNum(spray.startSpeed) }}</b> u/s</span>
      </div>
    </div>

    <div class="sd-body">
      <div class="sd-chart">
        <AimSprayChart v-model:active="active" :bullets="spray.shots" :break-shot="spray.breakShot" :height="320" />
      </div>
      <div class="sd-side">
        <AimSprayErrorChart v-model:active="active" :bullets="spray.shots" :break-shot="spray.breakShot" />
        <div class="table-wrap sd-table">
          <table class="data-table compact">
            <thead>
              <tr>
                <th class="num">#</th>
                <th class="num">垂直</th>
                <th class="num">水平</th>
                <th class="num">|误差|</th>
                <th>判断</th>
                <th>命中</th>
                <th v-if="hasTgt" class="num">离头</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="b in spray.shots"
                :key="b.idx"
                :class="{ selected: active === b.idx }"
                @mouseenter="active = b.idx"
                @mouseleave="active = null"
              >
                <td class="num strong">{{ b.idx }}</td>
                <td class="num">{{ signed(b.errV) }}</td>
                <td class="num">{{ signed(b.errH) }}</td>
                <td class="num" :class="toneClass(Math.hypot(b.errV, b.errH))">{{ Math.hypot(b.errV, b.errH).toFixed(2) }}°</td>
                <td class="muted">{{ b.idx === 1 ? '原点' : verdict(b) }}</td>
                <td>
                  <span v-if="b.hit" class="hit">{{ hitgroupName(b.hitgroup) }}</span>
                  <span v-else class="dim">—</span>
                </td>
                <td v-if="hasTgt" class="num muted">
                  {{ b.tgtV != null && b.tgtH != null ? `${Math.hypot(b.tgtV, b.tgtH).toFixed(1)}°` : '—' }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 单次连发的详情：弹道图 + 误差曲线 + 逐发表格，三者悬停联动。
 * 比赛详情页可以直接复用：<AimSprayDetail :spray="spray" />
 */
import type { Spray, SprayShot } from '@cs-analyst/shared-types';
import { errTone, fmtDeg, fmtNum, horizontalVerdict, verticalVerdict, weaponName } from '~/composables/useAim';

const props = defineProps<{ spray: Spray }>();
const active = ref<number | null>(null);

const hasTgt = computed(() => props.spray.shots.some((b) => b.tgtV != null));

function signed(v: number) {
  return `${v > 0 ? '+' : ''}${v.toFixed(2)}°`;
}
function toneClass(e: number) {
  const t = errTone(e);
  return t === 'bad' ? 'bad' : t === 'warn' ? 'warn' : '';
}
function verdict(b: SprayShot) {
  const v = verticalVerdict(b.errV);
  const h = horizontalVerdict(b.errH);
  if (v === '到位' && h === '到位') return '到位';
  return [v !== '到位' ? v : '', h !== '到位' ? h : ''].filter(Boolean).join('、');
}
const vClass = computed(() => (Math.abs(props.spray.meanErrV) > 1 ? 'warn' : ''));

const HITGROUPS: Record<string, string> = {
  head: '头',
  chest: '胸',
  stomach: '腹',
  left_arm: '左臂',
  right_arm: '右臂',
  left_leg: '左腿',
  right_leg: '右腿',
  neck: '颈',
  generic: '身体',
};
function hitgroupName(g: string | null) {
  if (!g) return '命中';
  return HITGROUPS[g.toLowerCase()] ?? g;
}
</script>

<style scoped>
.spray-detail { display: flex; flex-direction: column; gap: var(--space-3); }
.sd-head { display: flex; flex-direction: column; gap: var(--space-1); }
.sd-title { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); }
.sd-title .strong { color: var(--text); font-weight: 600; }
.sd-title .muted { color: var(--text-3); font-size: var(--text-xs); }
.sd-stats { display: flex; flex-wrap: wrap; gap: var(--space-1) var(--space-4); font-size: var(--text-xs); color: var(--text-3); }
.sd-stats b { color: var(--text); font-weight: 600; }
.sd-stats i { font-style: normal; color: var(--text-2); }
.sd-stats i.warn, .warn { color: var(--warning); }
.sd-body { display: grid; grid-template-columns: minmax(240px, 340px) minmax(0, 1fr); gap: var(--space-4); align-items: start; }
.sd-side { display: flex; flex-direction: column; gap: var(--space-3); min-width: 0; }
.sd-table { max-height: 260px; overflow: auto; }
.hit { color: var(--text-2); }
@media (max-width: 1000px) {
  .sd-body { grid-template-columns: 1fr; }
}
</style>
