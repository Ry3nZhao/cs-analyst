<template>
  <div class="runner" data-testid="training-runner">
    <canvas ref="canvas" class="runner-canvas" :class="{ 'hide-cursor': state === 'running' }" />

    <div v-if="state === 'ready'" class="runner-overlay" data-testid="training-start" @click="start">
      <div class="overlay-box">
        <div class="overlay-title">{{ meta.label }}</div>
        <div class="overlay-desc">{{ meta.description }}</div>
        <div class="overlay-hint">{{ startHint }}</div>
        <div class="overlay-keys">Esc 暂停</div>
      </div>
    </div>

    <div v-else-if="state === 'paused'" class="runner-overlay">
      <div class="overlay-box">
        <div class="overlay-title">已暂停</div>
        <div v-if="message" class="overlay-desc">{{ message }}</div>
        <div class="overlay-actions">
          <button class="btn-primary" type="button" @click="resume">继续</button>
          <button class="btn" type="button" @click="finishNow">结束并查看结果</button>
          <button class="btn-ghost" type="button" @click="emit('cancel')">放弃</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 全屏训练：覆盖整个窗口的 Canvas。点击开始时请求 Pointer Lock（需要用户手势），Esc 暂停。
 */
import type { TrainingModeMeta } from '@cs-analyst/shared-types';
import { TrainingEngine, type EngineFinish, type EngineState } from '~/training/engine';
import type { TrainingModeImpl } from '~/training/core';

const props = defineProps<{
  meta: TrainingModeMeta;
  mode: TrainingModeImpl;
  sensitivity: number;
}>();
const emit = defineEmits<{ done: [EngineFinish]; cancel: [] }>();

const canvas = ref<HTMLCanvasElement | null>(null);
const state = ref<EngineState>('ready');
const message = ref<string | undefined>();
let engine: TrainingEngine | null = null;

const startHint = computed(() =>
  props.meta.mode === 'strafe' || props.meta.mode === 'reaction'
    ? '点击开始'
    : '点击开始（会锁定鼠标，移动鼠标转动视角）',
);

onMounted(() => {
  engine = new TrainingEngine(canvas.value!, props.mode, {
    sensitivity: props.sensitivity,
    wantPointerLock: props.meta.pointerLock,
    onState: (s, msg) => {
      state.value = s;
      message.value = msg;
    },
    onDone: (f) => emit('done', f),
  });
  engine.mount();
});

onBeforeUnmount(() => engine?.destroy());

function start() {
  engine?.start();
}
function resume() {
  engine?.resume();
}
function finishNow() {
  engine?.finishNow();
}
</script>

<style scoped>
.runner {
  position: fixed;
  inset: 0;
  z-index: 100;
  background: var(--bg);
}
.runner-canvas { width: 100%; height: 100%; display: block; }
.runner-canvas.hide-cursor { cursor: none; }
.runner-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--bg) 72%, transparent);
  cursor: pointer;
}
.overlay-box {
  min-width: 320px;
  max-width: 440px;
  padding: var(--space-6);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  text-align: center;
  cursor: default;
}
.runner-overlay:has(.overlay-actions) { cursor: default; }
.overlay-title { font-size: var(--text-xl); font-weight: 600; color: var(--text); }
.overlay-desc { margin-top: var(--space-2); font-size: var(--text-sm); color: var(--text-2); line-height: 1.5; }
.overlay-hint { margin-top: var(--space-5); font-size: var(--text-md); color: var(--text); }
.overlay-keys { margin-top: var(--space-2); font-size: var(--text-xs); color: var(--text-3); }
.overlay-actions { margin-top: var(--space-5); display: flex; gap: var(--space-2); justify-content: center; }
.runner-overlay:not(:has(.overlay-actions)) .overlay-box { cursor: pointer; }
</style>
