<template>
  <!-- W3-A：集锦规则开关 / 权重、在 CS2 中播放。settings.vue 里只引用这个组件。 -->
  <div id="highlights" class="card card-pad">
    <div class="card-head">
      <div>
        <div class="card-title">集锦规则</div>
        <div class="card-desc">
          每条规则可以单独开关；权重会乘到规则原始分（0–100）上，决定集锦页的排序。改动立即生效，不需要重新计算。
        </div>
      </div>
      <div class="head-actions">
        <button type="button" class="btn btn-sm" :disabled="!s || isDefault" @click="resetRules">恢复默认</button>
        <button type="button" class="btn btn-sm" :disabled="recomputing" @click="recompute">
          {{ recomputing ? '重新计算中…' : '重新计算全部' }}
        </button>
      </div>
    </div>
    <p v-if="message" class="muted msg">{{ message }}</p>
    <p v-if="error" class="form-error">{{ error }}</p>

    <div v-if="s" class="rule-groups">
      <div v-for="g in groups" :key="g.category" class="rule-group">
        <div class="group-title">
          <span class="kind-dot" :class="g.category" />
          {{ g.label }}
        </div>
        <div v-for="r in g.rules" :key="r.kind" class="rule-row" :class="{ off: !r.enabled }">
          <button class="toggle" :class="{ on: r.enabled }" :aria-label="`开关 ${highlightLabel(r.kind)}`" @click="r.enabled = !r.enabled; save()">
            <span class="toggle-knob" />
          </button>
          <span class="rule-name">
            {{ highlightLabel(r.kind) }}
            <MetricHelp :metric="`highlight.${r.kind}`" />
          </span>
          <label class="weight">
            <span class="muted">权重</span>
            <input
              v-model.number="r.weight"
              type="number"
              min="0"
              max="3"
              step="0.1"
              class="input input-sm num-input"
              :disabled="!r.enabled"
              @change="save"
            />
          </label>
        </div>
      </div>
    </div>
    <div v-else class="muted">加载中…</div>
  </div>

  <div class="card card-pad">
    <div class="card-title">在 CS2 中播放</div>
    <div class="card-desc">
      集锦页的「播放」会给出 <span class="mono">playdemo</span> 和 <span class="mono">demo_gototick</span> 两条控制台命令，也可以直接用 CS2 打开 demo。
      压缩过的 demo 会先解压到系统临时目录（只保留最近 2 个）。
    </div>
    <template v-if="s">
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">提前几秒开始</span>
          <span class="setting-desc">跳到事件开始前 N 秒（不早于这一回合开始）</span>
        </div>
        <input v-model.number="s.leadSecs" type="number" min="0" max="30" class="input num-input" @change="save" />
      </div>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">
            自动跳转
            <StatusChip tone="warning" label="实验" />
          </span>
          <span class="setting-desc">
            启动 CS2 时加上 <span class="mono">-netconport</span>，demo 加载完后通过本地控制台端口自动发送跳转命令。Windows 上不一定可用。
          </span>
        </div>
        <button class="toggle" :class="{ on: s.netconEnabled }" @click="s.netconEnabled = !s.netconEnabled; save()">
          <span class="toggle-knob" />
        </button>
      </div>
      <template v-if="s.netconEnabled">
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-name">控制台端口</span>
            <span class="setting-desc">只监听本机 127.0.0.1</span>
          </div>
          <input v-model.number="s.netconPort" type="number" min="1024" max="65535" class="input num-input wide" @change="save" />
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <span class="setting-name">附加启动参数</span>
            <span class="setting-desc">端口在 Windows 上打不开时可以试试 <span class="mono">-tools</span>（参见 docs/highlights.md）</span>
          </div>
          <input v-model="s.extraLaunchArgs" class="input mono args" placeholder="例如 -tools" @change="save" />
        </div>
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { HighlightCategory, HighlightSettings } from '@cs-analyst/shared-types';
import { getHighlightSettings, recomputeHighlights, updateHighlightSettings } from '~/composables/useHighlights';

const s = ref<HighlightSettings | null>(null);
const error = ref('');
const message = ref('');
const recomputing = ref(false);

const ORDER = HIGHLIGHT_KINDS.map((k) => k.kind as string);
const groups = computed(() => {
  const rules = [...(s.value?.rules ?? [])].sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind));
  return (['highlight', 'mistake'] as HighlightCategory[]).map((c) => ({
    category: c,
    label: c === 'highlight' ? '高光' : '失误',
    rules: rules.filter((r) => r.category === c),
  }));
});
const isDefault = computed(() => (s.value?.rules ?? []).every((r) => r.enabled && Math.abs(r.weight - r.defaultWeight) < 1e-9));

async function load() {
  try {
    s.value = await getHighlightSettings();
  } catch (e) {
    error.value = `读取集锦设置失败：${e}`;
  }
}

async function save() {
  if (!s.value) return;
  error.value = '';
  try {
    s.value = await updateHighlightSettings(s.value);
  } catch (e) {
    error.value = `保存失败：${e}`;
  }
}

function resetRules() {
  if (!s.value) return;
  for (const r of s.value.rules) {
    r.enabled = true;
    r.weight = r.defaultWeight;
  }
  save();
}

async function recompute() {
  recomputing.value = true;
  message.value = '';
  error.value = '';
  try {
    const n = await recomputeHighlights(null);
    message.value = `已重新计算 ${n} 场对局的集锦。`;
  } catch (e) {
    error.value = `重新计算失败：${e}`;
  } finally {
    recomputing.value = false;
  }
}

onMounted(load);
</script>

<style scoped>
.card-head { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-4); }
.head-actions { display: flex; gap: var(--space-2); flex-shrink: 0; }
.card-title { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-md); font-weight: 600; color: var(--text); margin-bottom: 2px; }
.card-desc { font-size: var(--text-xs); color: var(--text-3); margin-bottom: var(--space-3); line-height: 1.5; }
.msg { font-size: var(--text-xs); margin: 0 0 var(--space-2); }

.rule-groups { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-5); }
.rule-group { display: flex; flex-direction: column; }
.group-title { display: flex; align-items: center; gap: 6px; font-size: var(--text-xs); color: var(--text-3); padding-bottom: var(--space-1); border-bottom: 1px solid var(--border-subtle); }
.rule-row {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: var(--space-3);
  min-height: 36px;
  border-bottom: 1px solid var(--border-subtle);
}
.rule-row:last-child { border-bottom: none; }
.rule-row.off .rule-name { color: var(--text-3); }
.rule-name { display: inline-flex; align-items: center; gap: 4px; font-size: var(--text-sm); color: var(--text); }
.weight { display: inline-flex; align-items: center; gap: var(--space-2); font-size: var(--text-xs); }
.num-input { width: 64px; text-align: right; }
.num-input.wide { width: 90px; }
.args { width: 220px; }
.kind-dot { width: 6px; height: 6px; border-radius: 50%; }
.kind-dot.highlight { background: var(--value-good); }
.kind-dot.mistake { background: var(--value-bad); }

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: 44px;
  border-bottom: 1px solid var(--border-subtle);
}
.setting-row:last-child { border-bottom: none; }
.setting-info { display: flex; flex-direction: column; gap: 2px; }
.setting-name { display: inline-flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); color: var(--text); }
.setting-desc { font-size: var(--text-xs); color: var(--text-3); }
</style>
