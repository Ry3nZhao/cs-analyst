<template>
  <div class="settings-page">
    <!-- Storage -->
    <div class="card">
      <div class="card-title">数据目录</div>
      <div class="card-desc">
        Demo、数据库与运行状态的实际存储位置（应用本地数据目录，不可在此修改）。
      </div>
      <div class="row-input">
        <input
          :value="settings.dataRoot"
          class="field-input flex-1 field-readonly"
          readonly
          title="数据目录"
        />
        <button class="btn-ghost" type="button" @click="openDataRoot">打开目录</button>
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

    <!-- Valve Bot (Auth Code) -->
    <div class="card">
      <div class="card-title">Valve Bot 账号</div>
      <div class="card-desc">
        Auth Code 通道用于发现对局；下载 demo 需 Bot 账号登录 Steam 解析分享码。
        凭证仅保存在本地数据库。
      </div>
      <div class="bot-form">
        <div class="field">
          <label class="field-label">username</label>
          <input v-model="valveBot.username" class="field-input" placeholder="bot_account_name" />
        </div>
        <div class="field">
          <label class="field-label">
            password
            <span v-if="valveBot.passwordSet" class="field-hint">（已保存，留空则不修改）</span>
          </label>
          <input
            v-model="valveBotPassword"
            type="password"
            class="field-input"
            autocomplete="new-password"
            placeholder="留空则不修改"
          />
        </div>
        <div class="field">
          <label class="field-label">
            secret
            <span v-if="valveBot.secretSet" class="field-hint">（已保存，留空则不修改）</span>
          </label>
          <input
            v-model="valveBotSecret"
            type="password"
            class="field-input"
            autocomplete="new-password"
            placeholder="Steam Guard 共享密钥，可选"
          />
        </div>
        <div class="field">
          <label class="field-label">API_key</label>
          <input
            v-model="valveBot.steamApiKey"
            class="field-input"
            placeholder="Steam Web API Key"
          />
        </div>
        <div class="bot-actions">
          <button class="btn-primary" :disabled="valveBotSaving" @click="saveValveBot">
            {{ valveBotSaving ? '保存中…' : '保存 Bot 配置' }}
          </button>
          <span v-if="valveBotSaved" class="save-ok">已保存</span>
        </div>
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

    <!-- Background task logs -->
    <div class="card">
      <div class="card-title">后台任务日志</div>
      <div class="card-desc">查看发现与下载任务的运行日志。</div>
      <button class="btn-ghost" type="button" @click="logOpen = true">查看日志</button>
    </div>

    <!-- About -->
    <div class="card card-about">
      <div class="about-row">
        <div>
          <div class="card-title">CS Analyst</div>
          <div class="card-desc">版本 {{ appVersion }} · 自动发现并下载 CS2 Demo 文件</div>
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
import { getVersion } from '@tauri-apps/api/app';
import type { AppSettingsDto, ValveBotConfigDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const { logOpen } = useAppStatus();
const appVersion = ref('0.1.1');

const settings = reactive<AppSettingsDto>({ dataRoot: '', globalMaxConcurrent: 1, autoDiscoverOnStartup: false });
const platforms = ref<{ platform: string; autoRetryFailed: boolean; maxAutoAttempts: number | null }[]>([]);
const valveBot = reactive<ValveBotConfigDto>({
  username: null,
  passwordSet: false,
  secretSet: false,
  steamApiKey: null,
  cronSchedule: '0 * * * *',
  runOnStartup: true,
});
const valveBotPassword = ref('');
const valveBotSecret = ref('');
const valveBotSaving = ref(false);
const valveBotSaved = ref(false);
const toast = useToast();

async function load() {
  Object.assign(settings, await tauriInvoke<AppSettingsDto>('get_settings'));
  platforms.value = await tauriInvoke('list_platform_settings');
  Object.assign(valveBot, await tauriInvoke<ValveBotConfigDto>('get_valve_bot_config'));
  valveBotPassword.value = '';
  valveBotSecret.value = '';
  valveBotSaved.value = false;
}

async function save() {
  settings.globalMaxConcurrent = Math.min(10, Math.max(1, Number(settings.globalMaxConcurrent) || 1));
  try {
    await tauriInvoke('update_settings', { settings });
  } catch (error) {
    toast.add({
      title: '保存设置失败',
      description: error instanceof Error ? error.message : String(error),
      color: 'error',
    });
    await load();
  }
}

async function savePlatform(p: (typeof platforms.value)[0]) {
  if (p.maxAutoAttempts !== null && p.maxAutoAttempts !== undefined) {
    const attempts = Number(p.maxAutoAttempts);
    p.maxAutoAttempts = Number.isFinite(attempts) ? Math.max(1, Math.floor(attempts)) : null;
  }
  try {
    await tauriInvoke('update_platform_settings', { platform: p });
  } catch (error) {
    toast.add({
      title: '保存平台设置失败',
      description: error instanceof Error ? error.message : String(error),
      color: 'error',
    });
    await load();
  }
}

async function saveValveBot() {
  valveBotSaving.value = true;
  valveBotSaved.value = false;
  try {
    await tauriInvoke('update_valve_bot_config', {
      config: {
        username: valveBot.username?.trim() || null,
        password: valveBotPassword.value.trim() || null,
        secret: valveBotSecret.value.trim() || null,
        steamApiKey: valveBot.steamApiKey?.trim() || null,
      },
    });
    Object.assign(valveBot, await tauriInvoke<ValveBotConfigDto>('get_valve_bot_config'));
    valveBotPassword.value = '';
    valveBotSecret.value = '';
    valveBotSaved.value = true;
  } catch (error) {
    toast.add({
      title: '保存 Bot 配置失败',
      description: error instanceof Error ? error.message : String(error),
      color: 'error',
    });
  } finally {
    valveBotSaving.value = false;
  }
}

function platformLabel(p: string) { return { valve: 'Valve', renown: 'Renown', '5eplay': '5EPlay' }[p] ?? p; }

async function openDataRoot() { await tauriInvoke('open_data_subdirectory', { subdir: '' }); }
async function openDb()       { await tauriInvoke('open_data_subdirectory', { subdir: 'db' }); }
async function openLogs()     { await tauriInvoke('open_data_subdirectory', { subdir: 'state' }); }

onMounted(async () => {
  await load();
  try { appVersion.value = await getVersion(); } catch { /* dev mode */ }
});
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

.bot-form { display: flex; flex-direction: column; gap: 12px; }
.field { display: flex; flex-direction: column; gap: 5px; }
.field-label { font-size: 11px; color: var(--c-text-3); }
.field-hint { color: var(--c-text-faint); font-weight: 400; margin-left: 4px; }
.bot-actions { display: flex; align-items: center; gap: 10px; margin-top: 4px; }
.save-ok { font-size: 12px; color: var(--c-success); }

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
.field-readonly {
  color: var(--c-text-2);
  cursor: default;
}
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
