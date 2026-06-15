<template>
  <div class="settings-page">
    <!-- Storage -->
    <div class="card">
      <div class="card-title">数据目录</div>
      <div class="card-desc">Demo 文件和数据库的存储位置</div>
      <div class="row-input">
        <input v-model="settings.dataRoot" class="field-input flex-1" placeholder="/path/to/data" />
        <button class="btn-primary" @click="save">保存</button>
        <button class="btn-ghost" @click="openDataRoot">打开目录</button>
      </div>
    </div>

    <!-- Scheduling -->
    <div class="card">
      <div class="card-title">调度设置</div>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">全局最大并发下载数</span>
          <span class="setting-desc">同时进行的最大下载任务数量</span>
        </div>
        <input
          v-model.number="settings.globalMaxConcurrent"
          type="number"
          min="1"
          max="10"
          class="num-input"
          @change="save"
        />
      </div>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">启动时自动发现</span>
          <span class="setting-desc">应用启动后立即执行一次 Demo 发现</span>
        </div>
        <button
          class="toggle-btn"
          :class="{ on: settings.autoDiscoverOnStartup }"
          @click="settings.autoDiscoverOnStartup = !settings.autoDiscoverOnStartup; save()"
        >
          <span class="toggle-knob" />
        </button>
      </div>
    </div>

    <!-- Platform settings -->
    <div class="card">
      <div class="card-title">平台重试策略</div>
      <div
        v-for="(p, i) in platforms"
        :key="p.platform"
        class="platform-section"
        :class="{ 'not-last': i < platforms.length - 1 }"
      >
        <div class="platform-name">{{ platformLabel(p.platform) }}</div>
        <div class="setting-row sub">
          <div class="setting-info">
            <span class="setting-name">失败时自动重试</span>
          </div>
          <button
            class="toggle-btn"
            :class="{ on: p.autoRetryFailed }"
            @click="p.autoRetryFailed = !p.autoRetryFailed; savePlatform(p)"
          >
            <span class="toggle-knob" />
          </button>
        </div>
        <div class="setting-row sub">
          <div class="setting-info">
            <span class="setting-name">最大自动尝试次数</span>
            <span class="setting-desc">留空表示不限制</span>
          </div>
          <input
            v-model.number="p.maxAutoAttempts"
            type="number"
            min="1"
            placeholder="∞"
            class="num-input"
            @change="savePlatform(p)"
          />
        </div>
      </div>
    </div>

    <!-- About -->
    <div class="card card-about">
      <div class="about-row">
        <div>
          <div class="card-title">CS Demo Analyst</div>
          <div class="card-desc">版本 0.1.0 · 自动发现并下载 CS2 Demo 文件</div>
        </div>
        <div class="about-actions">
          <button class="btn-ghost" @click="openDb">数据库目录</button>
          <button class="btn-ghost" @click="openLogs">日志目录</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { AppSettingsDto } from '@cs-demo-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const settings = reactive<AppSettingsDto>({ dataRoot: '', globalMaxConcurrent: 3, autoDiscoverOnStartup: true });
const platforms = ref<{ platform: string; autoRetryFailed: boolean; maxAutoAttempts: number | null }[]>([]);

async function load() {
  Object.assign(settings, await tauriInvoke<AppSettingsDto>('get_settings'));
  platforms.value = await tauriInvoke('list_platform_settings');
}

async function save() { await tauriInvoke('update_settings', { settings }); }
async function savePlatform(p: (typeof platforms.value)[0]) { await tauriInvoke('update_platform_settings', { platform: p }); }

function platformLabel(p: string) { return { valve: 'Valve', renown: 'Renown', '5eplay': '5EPlay' }[p] ?? p; }

async function openDataRoot() { await tauriInvoke('open_path', { path: settings.dataRoot }); }
async function openDb()        { await tauriInvoke('open_path', { path: `${settings.dataRoot}/db` }); }
async function openLogs()      { await tauriInvoke('open_path', { path: `${settings.dataRoot}/state` }); }

onMounted(load);
</script>

<style scoped>
.settings-page {
  max-width: 640px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.card {
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 12px;
  padding: 18px 20px;
}
.card-title { font-size: 13px; font-weight: 600; color: #fff; margin-bottom: 3px; }
.card-desc  { font-size: 11px; color: var(--c-text-3); margin-bottom: 14px; }

.row-input { display: flex; gap: 8px; align-items: center; }
.field-input {
  padding: 8px 10px;
  border-radius: 7px;
  border: 1px solid var(--c-border);
  background: var(--c-card);
  color: var(--c-text);
  font-size: 13px;
  outline: none;
  min-width: 0;
}
.field-input:focus { border-color: var(--c-accent); }
.field-input::placeholder { color: var(--c-text-faint); }
.flex-1 { flex: 1; }

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 0;
  border-bottom: 1px solid var(--c-border-sub);
}
.setting-row:last-child, .setting-row.sub:last-child { border-bottom: none; }
.setting-info { display: flex; flex-direction: column; gap: 2px; }
.setting-name { font-size: 12px; color: var(--c-text); }
.setting-desc { font-size: 11px; color: var(--c-text-3); }

.num-input {
  width: 64px;
  padding: 6px 8px;
  border-radius: 6px;
  border: 1px solid var(--c-border);
  background: var(--c-card);
  color: var(--c-text);
  font-size: 13px;
  text-align: center;
  outline: none;
}
.num-input:focus { border-color: var(--c-accent); }

/* Toggle */
.toggle-btn {
  position: relative;
  width: 34px;
  height: 18px;
  border-radius: 9px;
  background: var(--c-border);
  border: none;
  cursor: pointer;
  transition: background 0.2s;
  flex-shrink: 0;
  padding: 0;
}
.toggle-btn.on { background: var(--c-accent); }
.toggle-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.2s;
  display: block;
}
.toggle-btn.on .toggle-knob { transform: translateX(16px); }

/* Platform section */
.platform-section { padding: 10px 0; }
.platform-section.not-last { border-bottom: 1px solid var(--c-border); }
.platform-name {
  font-size: 11px;
  font-weight: 600;
  color: var(--c-accent-lt);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin-bottom: 4px;
}
.setting-row.sub { padding: 7px 0; }

/* About */
.about-row { display: flex; align-items: center; justify-content: space-between; }
.about-actions { display: flex; gap: 8px; }
.card-about .card-desc { margin-bottom: 0; }

/* Buttons */
.btn-primary {
  padding: 7px 14px;
  border-radius: 7px;
  background: var(--c-accent);
  color: #fff;
  border: none;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
}
.btn-primary:hover { opacity: 0.85; }
.btn-ghost {
  padding: 7px 14px;
  border-radius: 7px;
  background: rgba(255,255,255,0.05);
  color: var(--c-text-2);
  border: 1px solid rgba(255,255,255,0.08);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
}
.btn-ghost:hover { background: rgba(255,255,255,0.08); color: var(--c-text); }
</style>
