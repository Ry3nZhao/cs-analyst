<template>
  <!-- W3-C：主玩家 / 监听目录 / 通知 / 托盘。settings.vue 里只引用这个组件。 -->
  <div class="card card-pad">
    <div class="card-head">
      <div>
        <div class="card-title">主玩家</div>
        <div class="card-desc">「我」是谁。个人主页、趋势和集锦都以主玩家的视角展示。</div>
      </div>
      <button type="button" class="btn btn-sm" @click="pickerOpen = !pickerOpen">{{ pickerOpen ? '收起' : isSet ? '更改' : '选择' }}</button>
    </div>
    <div class="me-row">
      <template v-if="isSet">
        <span class="strong">{{ displayName ?? '（暂无名字）' }}</span>
        <span class="mono muted">{{ steamid }}</span>
        <span class="muted num">{{ matchCount }} 场</span>
        <span class="spacer" />
        <button type="button" class="btn-danger btn-sm" @click="clearMain">清除</button>
      </template>
      <template v-else>
        <span class="muted">未设置</span>
        <template v-if="suggestion">
          <span class="dim">·</span>
          <span class="muted">建议：</span>
          <span class="strong">{{ suggestion.displayName ?? suggestion.steamid }}</span>
          <span class="tag">{{ mainPlayerSourceLabel(suggestion.source) }}</span>
          <span class="spacer" />
          <button type="button" class="btn-primary btn-sm" @click="setMain(suggestion.steamid)">设为主玩家</button>
        </template>
      </template>
    </div>
    <div v-if="pickerOpen" class="picker-wrap">
      <MainPlayerPicker @selected="pickerOpen = false" />
    </div>
  </div>

  <div class="card card-pad">
    <div class="card-title">CS2 录像目录</div>
    <div class="card-desc">在 CS2 里下载的比赛录像（game/csgo/replays）出现后自动复制导入，不会挪走或修改游戏里的文件。</div>
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">自动导入新录像</span>
        <span class="setting-desc">
          <StatusChip v-if="!exp.watchReplays" tone="neutral" label="已关闭" />
          <StatusChip v-else-if="watch?.watching" tone="success" dot label="监听中" />
          <StatusChip v-else-if="watch?.dir" tone="warning" label="目录不存在，等待出现" />
          <StatusChip v-else tone="warning" label="未找到 CS2" />
          <span v-if="watch && watch.importedCount > 0" class="num">　本次已导入 {{ watch.importedCount }} 个</span>
        </span>
      </div>
      <button class="toggle" :class="{ on: exp.watchReplays }" @click="exp.watchReplays = !exp.watchReplays; save()">
        <span class="toggle-knob" />
      </button>
    </div>
    <div class="dir-row">
      <input class="input flex-1 mono" :value="watch?.dir ?? ''" placeholder="未检测到 CS2 安装位置" readonly />
      <span v-if="watch?.dir" class="tag">{{ watch.source === 'manual' ? '手动指定' : '自动检测' }}</span>
      <button type="button" class="btn" @click="chooseDir">选择目录…</button>
      <button v-if="exp.replaysDir" type="button" class="btn-ghost" @click="exp.replaysDir = null; save()">自动检测</button>
    </div>
    <p v-if="watch?.lastError" class="form-error">{{ watch.lastError }}</p>
  </div>

  <div class="card card-pad">
    <div class="card-title">通知与托盘</div>
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">新对局解析完成时通知</span>
        <span class="setting-desc">弹出系统通知，点击打开这场对局</span>
      </div>
      <div class="row-actions">
        <button type="button" class="btn btn-sm" :disabled="!exp.notifyNewMatch" @click="testNotify">测试</button>
        <button class="toggle" :class="{ on: exp.notifyNewMatch }" @click="exp.notifyNewMatch = !exp.notifyNewMatch; save()">
          <span class="toggle-knob" />
        </button>
      </div>
    </div>
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">关闭窗口时最小化到托盘</span>
        <span class="setting-desc">后台继续下载和解析；从托盘菜单「退出」才会真正关闭</span>
      </div>
      <button class="toggle" :class="{ on: exp.closeToTray }" @click="exp.closeToTray = !exp.closeToTray; save()">
        <span class="toggle-knob" />
      </button>
    </div>
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-name">首次启动向导</span>
        <span class="setting-desc">重新走一遍：数据目录、获取对局、选择主玩家</span>
      </div>
      <button type="button" class="btn btn-sm" @click="openWizard">打开向导</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { ExperienceSettingsDto, WatchStatusDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import { mainPlayerSourceLabel } from '~/composables/useMainPlayer';

const toast = useToast();
const { steamid, displayName, matchCount, isSet, suggestion, set, clear } = useMainPlayer();
const pickerOpen = ref(false);

const exp = reactive<ExperienceSettingsDto>({ closeToTray: true, notifyNewMatch: true, watchReplays: true, replaysDir: null });
const watch = ref<WatchStatusDto | null>(null);

function errorText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

async function load() {
  try {
    Object.assign(exp, await tauriInvoke<ExperienceSettingsDto>('get_experience_settings'));
    watch.value = await tauriInvoke<WatchStatusDto>('get_watch_status');
  } catch {
    /* 浏览器 */
  }
}

async function save() {
  try {
    watch.value = await tauriInvoke<WatchStatusDto>('update_experience_settings', { settings: { ...exp } });
  } catch (e) {
    toast.add({ title: '保存设置失败', description: errorText(e), color: 'error' });
    await load();
  }
}

async function chooseDir() {
  const dir = await openDialog({ directory: true, multiple: false, title: '选择 CS2 replays 目录', defaultPath: watch.value?.dir ?? undefined });
  if (typeof dir !== 'string' || !dir) return;
  exp.replaysDir = dir;
  await save();
}

async function setMain(id: string) {
  try {
    await set(id);
  } catch (e) {
    toast.add({ title: '设置主玩家失败', description: errorText(e), color: 'error' });
  }
}

async function clearMain() {
  try {
    await clear();
  } catch (e) {
    toast.add({ title: '清除失败', description: errorText(e), color: 'error' });
  }
}

async function testNotify() {
  try {
    await tauriInvoke('send_test_notification');
  } catch (e) {
    toast.add({ title: '发送失败', description: errorText(e), color: 'error' });
  }
}

function openWizard() {
  useState('onboarding-wizard-step', () => 0).value = 0;
  void navigateTo('/welcome');
}

let offWatch: (() => void) | null = null;
listen<WatchStatusDto>('watch_status', (e) => (watch.value = e.payload))
  .then((fn) => (offWatch = fn))
  .catch(() => {});
onBeforeUnmount(() => offWatch?.());
onMounted(load);
</script>

<style scoped>
.card-head { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-4); }
.card-title { font-size: var(--text-md); font-weight: 600; color: var(--text); margin-bottom: 2px; }
.card-desc { font-size: var(--text-xs); color: var(--text-3); margin-bottom: var(--space-3); line-height: 1.5; }

.me-row { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-2); min-height: var(--control-h); font-size: var(--text-sm); }
.spacer { flex: 1; }
.picker-wrap { margin-top: var(--space-3); padding-top: var(--space-3); border-top: 1px solid var(--border-subtle); }

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
.setting-name { font-size: var(--text-sm); color: var(--text); }
.setting-desc { display: flex; align-items: center; gap: var(--space-1); font-size: var(--text-xs); color: var(--text-3); }
.row-actions { display: flex; align-items: center; gap: var(--space-2); }

.dir-row { display: flex; align-items: center; gap: var(--space-2); margin-top: var(--space-3); }
.flex-1 { flex: 1; min-width: 0; }
</style>
