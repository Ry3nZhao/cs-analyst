<template>
  <AppModal v-model:open="open" title="雷达底图">
    <div class="rs">
      <p class="muted intro">
        CS2 的雷达图归 Valve 所有，应用不自带。默认从本机 CS2 安装目录读取（只读，解码后缓存在数据目录），
        也可以指定一个自己的图片目录。都没有时按坐标直接画点。
      </p>

      <div class="field">
        <div class="row">
          <span class="field-label">读取本机 CS2</span>
          <button type="button" class="toggle sm" :class="{ on: form.useCs2 }" @click="form.useCs2 = !form.useCs2"><span class="toggle-knob" /></button>
        </div>
        <div class="row">
          <input v-model="form.cs2Dir" class="input input-sm" placeholder="留空 = 通过 Steam 自动查找" :disabled="!form.useCs2" />
          <button type="button" class="btn btn-sm" :disabled="!form.useCs2" @click="pick('cs2Dir')">选择…</button>
        </div>
        <span class="field-hint">
          {{ info?.cs2Dir ? `正在使用：${info.cs2Dir}` : '未找到 CS2 安装目录' }}
        </span>
      </div>

      <div class="field">
        <span class="field-label">自定义图片目录</span>
        <div class="row">
          <input v-model="form.imageDir" class="input input-sm" placeholder="可选" />
          <button type="button" class="btn btn-sm" @click="pick('imageDir')">选择…</button>
        </div>
        <span class="field-hint">
          文件名：<code>{{ map }}_radar.png</code> 或 <code>{{ map }}.png</code>（也支持 jpg / webp），下层用 <code>{{ map }}_lower_radar.png</code>。
          需要时放一份 <code>{{ map }}.txt</code>（CS2 overview 格式，含 pos_x / pos_y / scale）覆盖坐标参数。优先于 CS2 安装目录。
        </span>
      </div>

      <dl v-if="info" class="kv-grid">
        <dt>地图</dt><dd class="mono">{{ info.mapName }}</dd>
        <dt>底图</dt><dd>{{ info.levels.length ? info.levels.map((l) => `${l.level === 'lower' ? '下层' : '上层'}（${SRC[l.source]}）`).join('、') : '无' }}</dd>
        <dt>坐标参数</dt>
        <dd>
          <template v-if="info.calibration">
            <span class="mono num">pos {{ info.calibration.posX }}, {{ info.calibration.posY }} · scale {{ info.calibration.scale }}</span>
            <span class="muted">（{{ SRC[info.calibrationSource ?? ''] }}）</span>
          </template>
          <template v-else>无，按数据范围自动拟合</template>
        </dd>
        <template v-if="info.note"><dt>说明</dt><dd class="muted">{{ info.note }}</dd></template>
      </dl>
      <div v-if="err" class="form-error">{{ err }}</div>
    </div>
    <template #footer="{ close }">
      <button type="button" class="btn btn-sm" @click="close">取消</button>
      <button type="button" class="btn btn-primary btn-sm" :disabled="saving" @click="save">保存并重新加载</button>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { RadarInfo, RadarSettings } from '@cs-analyst/shared-types';

const open = defineModel<boolean>('open', { default: false });
const props = defineProps<{ mapName: string | null; info: RadarInfo | null }>();
const emit = defineEmits<{ saved: [] }>();

const SRC: Record<string, string> = { user: '自定义目录', cs2: '本机 CS2', builtin: '内置表' };
const map = computed(() => props.mapName ?? 'de_dust2');
const form = reactive({ imageDir: '', cs2Dir: '', useCs2: true });
const saving = ref(false);
const err = ref<string | null>(null);

watch(open, async (o) => {
  if (!o) return;
  err.value = null;
  try {
    const s = await invoke<RadarSettings>('get_radar_settings');
    form.imageDir = s.imageDir ?? '';
    form.cs2Dir = s.cs2Dir ?? '';
    form.useCs2 = s.useCs2 !== false;
  } catch (e) {
    err.value = String(e);
  }
});

async function pick(key: 'imageDir' | 'cs2Dir') {
  const dir = await openDialog({ directory: true, multiple: false });
  if (typeof dir === 'string') form[key] = dir;
}

async function save() {
  saving.value = true;
  err.value = null;
  try {
    await invoke('update_radar_settings', {
      settings: { imageDir: form.imageDir || null, cs2Dir: form.cs2Dir || null, useCs2: form.useCs2 },
    });
    emit('saved');
    open.value = false;
  } catch (e) {
    err.value = String(e);
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
.rs { display: flex; flex-direction: column; gap: var(--space-4); width: 520px; max-width: 100%; }
.intro { font-size: var(--text-sm); line-height: 1.6; margin: 0; }
.row { display: flex; align-items: center; gap: var(--space-2); justify-content: space-between; }
.row .input { flex: 1; }
code { font-family: var(--font-mono); font-size: 11px; color: var(--text-2); }
</style>
