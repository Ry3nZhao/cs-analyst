<template>
  <span class="metric-help">
    <span v-if="label" class="mh-label">{{ label === true ? def?.name : label }}</span>
    <UPopover
      v-if="def"
      :open="open"
      mode="hover"
      :open-delay="150"
      :close-delay="120"
      :content="{ side, sideOffset: 6, collisionPadding: 12 }"
      @update:open="onUpdate"
    >
      <button
        type="button"
        class="mh-trigger"
        :class="{ pinned }"
        :aria-label="`${def.name} 的说明`"
        @click.stop.prevent="togglePin"
      >
        ?
      </button>
      <template #content>
        <div ref="panel" class="mh-panel" @click.stop>
          <div class="mh-head">
            <span class="mh-name">{{ def.name }}</span>
            <span v-if="def.unit" class="mh-unit">{{ def.unit }}</span>
            <span v-if="def.higherIsBetter !== undefined" class="mh-dir">
              {{ def.higherIsBetter ? '越高越好' : '越低越好' }}
            </span>
          </div>
          <p class="mh-def">{{ def.definition }}</p>
          <div class="mh-section">
            <div class="mh-section-title">计算方式</div>
            <p class="mh-text">{{ def.calculation }}</p>
          </div>
          <div v-if="def.references?.length" class="mh-section">
            <div class="mh-section-title">参考值</div>
            <div class="mh-refs">
              <div v-for="r in def.references" :key="r.range" class="mh-ref">
                <span class="mh-ref-dot" :class="r.tone ?? 'neutral'" />
                <span class="mh-ref-range num">{{ r.range }}</span>
                <span class="mh-ref-label">{{ r.label }}</span>
              </div>
            </div>
          </div>
          <p v-if="def.caveat" class="mh-caveat">{{ def.caveat }}</p>
          <slot />
        </div>
      </template>
    </UPopover>
  </span>
</template>

<script setup lang="ts">
/**
 * 指标说明：「?」悬停显示，点击固定（再点或点外面关闭）。
 *
 * ```vue
 * <th>ADR <MetricHelp metric="adr" /></th>
 * <MetricHelp metric="ttd" label />            <!-- 连同指标名一起显示 -->
 * <MetricHelp metric="kast" label="KAST%" side="bottom" />
 * ```
 * metric 取 utils/metricGlossary.ts 里的 key（也接受常见别名，例如 hs、cp、timeToDamage）。
 */
const props = withDefaults(
  defineProps<{
    metric: string;
    /** true = 显示指标名；字符串 = 自定义文字；不传只显示「?」 */
    label?: string | boolean;
    side?: 'top' | 'bottom' | 'left' | 'right';
  }>(),
  { label: undefined, side: 'top' },
);

const def = computed(() => getMetric(props.metric));
if (import.meta.dev && !def.value) console.warn(`[MetricHelp] 未知指标：${props.metric}`);

const open = ref(false);
const pinned = ref(false);

function onUpdate(v: boolean) {
  if (!v && pinned.value) return; // 固定时不随鼠标离开关闭
  open.value = v;
}

function togglePin() {
  pinned.value = !pinned.value;
  open.value = pinned.value;
}

function onDocClick() {
  if (pinned.value) {
    pinned.value = false;
    open.value = false;
  }
}
onMounted(() => document.addEventListener('click', onDocClick));
onBeforeUnmount(() => document.removeEventListener('click', onDocClick));
</script>

<style scoped>
.metric-help {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  vertical-align: middle;
}
.mh-label { white-space: nowrap; }
.mh-trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  padding: 0;
  border: 1px solid var(--border-strong);
  border-radius: 50%;
  background: transparent;
  color: var(--text-3);
  font-size: 9px;
  font-weight: 700;
  line-height: 1;
  cursor: help;
  flex-shrink: 0;
}
.mh-trigger:hover { color: var(--text); border-color: var(--text-3); }
.mh-trigger.pinned { color: var(--text); border-color: var(--accent); }
.mh-trigger:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus-ring); }

.mh-panel {
  width: 300px;
  max-width: calc(100vw - 24px);
  padding: var(--space-3);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  font-size: var(--text-xs);
  color: var(--text-2);
  line-height: 1.55;
  text-align: left;
  white-space: normal;
  font-weight: 400;
  text-transform: none;
  letter-spacing: normal;
}
.mh-head { display: flex; align-items: baseline; gap: var(--space-2); }
.mh-name { font-size: var(--text-sm); font-weight: 600; color: var(--text); }
.mh-unit { color: var(--text-3); }
.mh-dir { margin-left: auto; color: var(--text-3); white-space: nowrap; }
.mh-def { margin: 0; color: var(--text); }
.mh-section { display: flex; flex-direction: column; gap: 2px; }
.mh-section-title { font-size: 10px; font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.06em; }
.mh-text { margin: 0; }
.mh-refs { display: flex; flex-direction: column; gap: 2px; }
.mh-ref { display: flex; align-items: center; gap: var(--space-2); }
.mh-ref-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--text-disabled); }
.mh-ref-dot.good { background: var(--value-good); }
.mh-ref-dot.bad { background: var(--value-bad); }
.mh-ref-range { min-width: 84px; color: var(--text); }
.mh-ref-label { color: var(--text-3); }
.mh-caveat {
  margin: 0;
  padding-top: var(--space-2);
  border-top: 1px solid var(--border-subtle);
  color: var(--text-3);
}
</style>
