<template>
  <div class="pb" :class="`tone-${tone}`">
    <div class="pb-track" :title="text">
      <div v-if="p.value != null" class="pb-fill" :style="{ width: `${Math.max(2, p.value)}%` }" />
      <div class="pb-mid" />
    </div>
    <div class="pb-text">
      <span v-if="p.value != null" class="pb-num num">P{{ p.value.toFixed(0) }}</span>
      <span class="pb-desc">{{ desc }}</span>
      <span v-if="p.value != null && p.lowConfidence" class="pb-low" title="样本少或本人数据少，百分位只作参考">参考</span>
    </div>
  </div>
</template>

<script setup lang="ts">
/** 百分位条：中线 = 样本中位数（P50）。颜色只表达好坏（≥ P67 好，≤ P33 差）。 */
import type { PercentileDto } from '@cs-analyst/shared-types';
import { percentileText, percentileTone, scopeLabel } from '~/composables/useProfile';

const props = defineProps<{ p: PercentileDto; available: boolean }>();
const tone = computed(() => percentileTone(props.p.value));
const text = computed(() => percentileText(props.p));
const desc = computed(() => {
  if (!props.available) return '不可用';
  if (props.p.scope === 'none') return `样本不足（${props.p.sampleSize} 人）`;
  if (props.p.value == null) return '不可用';
  return `${scopeLabel(props.p.scope)} ${props.p.sampleSize} 人`;
});
</script>

<style scoped>
.pb { display: flex; flex-direction: column; gap: 4px; }
.pb-track { position: relative; height: 4px; border-radius: var(--radius-full); background: var(--surface-active); overflow: hidden; }
.pb-fill { position: absolute; inset: 0 auto 0 0; background: var(--text-3); border-radius: var(--radius-full); }
.pb-mid { position: absolute; left: 50%; top: 0; bottom: 0; width: 1px; background: var(--border-strong); }
.tone-success .pb-fill { background: var(--value-good); }
.tone-danger .pb-fill { background: var(--value-bad); }
.pb-text { display: flex; align-items: baseline; gap: 6px; font-size: var(--text-xs); color: var(--text-3); }
.pb-num { color: var(--text-2); font-weight: 600; }
.tone-success .pb-num { color: var(--value-good); }
.tone-danger .pb-num { color: var(--value-bad); }
.pb-low { padding: 0 4px; border: 1px solid var(--border); border-radius: var(--radius-sm); font-size: 10px; color: var(--text-3); }
</style>
