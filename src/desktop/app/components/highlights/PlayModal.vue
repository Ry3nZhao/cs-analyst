<template>
  <AppModal :open="!!item" title="在 CS2 中播放" @update:open="(v: boolean) => { if (!v) emit('close'); }">
    <div v-if="item" class="play">
      <div class="play-head">
        <span class="tag">{{ highlightLabel(item.kind) }}</span>
        <span class="strong">{{ item.playerName ?? item.steamid }}</span>
        <span class="muted">{{ mapLabel(item.mapName) }} · 第 {{ item.round }} 回合 · tick {{ item.startTick }}</span>
      </div>

      <div v-if="loading" class="muted">{{ item.demoStorage === 'compressed' ? '正在把压缩的 demo 解压到临时目录…' : '正在定位 demo…' }}</div>
      <div v-else-if="error" class="callout callout-danger">{{ error }}</div>

      <template v-else-if="pb">
        <div v-if="pb.status !== 'ready'" class="callout callout-warning">
          {{ pb.message ?? '这场的 demo 不可用，无法播放。' }}
        </div>

        <template v-else>
          <p class="hint">
            CS2 里按 <kbd>~</kbd> 打开控制台，先粘贴第一条打开 demo，加载完后再粘贴第二条跳到事件前
            <span class="num">{{ pb.leadSecs }}</span> 秒（tick <span class="num">{{ pb.gotoTick }}</span>）。
            <span v-if="pb.decompressed">demo 原本是压缩的，已解压到临时目录。</span>
          </p>

          <div class="cmd">
            <span class="cmd-label">1 · 打开 demo</span>
            <code class="mono cmd-text" :title="pb.playdemoCommand ?? ''">{{ pb.playdemoCommand }}</code>
            <button class="btn btn-sm" type="button" @click="copy('play', pb.playdemoCommand)">{{ copied === 'play' ? '已复制' : '复制' }}</button>
          </div>
          <div class="cmd">
            <span class="cmd-label">2 · 跳转</span>
            <code class="mono cmd-text">{{ pb.gotoCommand }}</code>
            <button class="btn btn-sm" type="button" @click="copy('goto', pb.gotoCommand)">{{ copied === 'goto' ? '已复制' : '复制' }}</button>
          </div>

          <div v-if="launch" class="callout" :class="launch.method === 'unavailable' ? 'callout-warning' : 'callout-info'">
            {{ launch.message }}
          </div>
          <div v-if="netcon" class="netcon muted">
            <StatusChip :tone="netcon.stage === 'failed' ? 'danger' : netcon.stage === 'sent' ? 'success' : 'info'" :pulse="!['sent', 'failed'].includes(netcon.stage)" :label="NETCON_STAGE[netcon.stage]" />
            <span>{{ netcon.message }}</span>
          </div>
          <p class="hint dim">
            「用 CS2 打开」通过 Steam 启动 CS2 并自动执行第一条命令。CS2 已经开着时 Steam 可能不会传递启动参数，请改用复制命令。
            <template v-if="!netconEnabled">在设置的「集锦」里可以打开实验性的自动跳转（-netconport）。</template>
          </p>
        </template>
      </template>
    </div>

    <template #footer="{ close }">
      <button class="btn" type="button" @click="close">关闭</button>
      <button
        v-if="pb?.status === 'ready'"
        class="btn"
        type="button"
        @click="copy('both', pb.combinedCommand)"
      >
        {{ copied === 'both' ? '已复制' : '复制两条' }}
      </button>
      <button
        v-if="pb?.status === 'ready'"
        class="btn-primary"
        type="button"
        :disabled="launching"
        @click="doLaunch"
      >
        {{ launching ? '正在启动…' : '用 CS2 打开' }}
      </button>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import type { HighlightItem, HighlightLaunchResult, HighlightNetconEvent, HighlightPlayback } from '@cs-analyst/shared-types';
import { copyText, launchHighlightInCs2, listenHighlightNetcon, prepareHighlightPlayback } from '~/composables/useHighlights';
import { mapLabel } from '~/composables/useMatchDetail';

const props = defineProps<{ item: HighlightItem | null; netconEnabled?: boolean }>();
const emit = defineEmits<{ close: [] }>();

const NETCON_STAGE: Record<HighlightNetconEvent['stage'], string> = {
  connecting: '等待端口',
  connected: '已连接',
  loading: '等待加载',
  sent: '已跳转',
  failed: '自动跳转失败',
};

const pb = ref<HighlightPlayback | null>(null);
const loading = ref(false);
const error = ref('');
const copied = ref<string | null>(null);
const launching = ref(false);
const launch = ref<HighlightLaunchResult | null>(null);
const netcon = ref<HighlightNetconEvent | null>(null);

let seq = 0;
watch(
  () => props.item?.id,
  async (id) => {
    pb.value = null;
    error.value = '';
    launch.value = null;
    netcon.value = null;
    copied.value = null;
    if (id == null) return;
    const my = ++seq;
    loading.value = true;
    try {
      const res = await prepareHighlightPlayback(id);
      if (my === seq) pb.value = res;
    } catch (e) {
      if (my === seq) error.value = `准备播放失败：${e}`;
    } finally {
      if (my === seq) loading.value = false;
    }
  },
  { immediate: true },
);

let copiedTimer: ReturnType<typeof setTimeout> | undefined;
async function copy(key: string, text: string | null | undefined) {
  if (!text) return;
  if (await copyText(text)) {
    copied.value = key;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied.value = null), 1500);
  }
}

async function doLaunch() {
  if (!props.item) return;
  launching.value = true;
  netcon.value = null;
  try {
    launch.value = await launchHighlightInCs2(props.item.id);
    if (!launch.value.autoGoto) await copy('goto', launch.value.playback.gotoCommand);
  } catch (e) {
    error.value = `启动 CS2 失败：${e}`;
  } finally {
    launching.value = false;
  }
}

const off = listenHighlightNetcon((e) => {
  if (e.highlightId === props.item?.id) netcon.value = e;
});
onBeforeUnmount(() => {
  off();
  clearTimeout(copiedTimer);
});
</script>

<style scoped>
.play { display: flex; flex-direction: column; gap: var(--space-3); min-width: 0; }
.play-head { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; font-size: var(--text-sm); }
.hint { margin: 0; font-size: var(--text-sm); color: var(--text-2); line-height: 1.6; }
.hint.dim { font-size: var(--text-xs); color: var(--text-3); }
kbd { font-family: var(--font-mono); font-size: var(--text-xs); padding: 0 4px; border: 1px solid var(--border-strong); border-radius: var(--radius-sm); }
.cmd { display: grid; grid-template-columns: 72px minmax(0, 1fr) auto; align-items: center; gap: var(--space-2); }
.cmd-label { font-size: var(--text-xs); color: var(--text-3); }
.cmd-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 4px 8px;
  font-size: var(--text-xs);
  color: var(--text);
  background: var(--surface-raised);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.netcon { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-xs); }
</style>
