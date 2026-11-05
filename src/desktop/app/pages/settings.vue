<template>
  <div class="settings-page">
    <!-- Storage -->
    <div class="card card-pad">
      <div class="card-title">数据目录</div>
      <div class="card-desc">
        Demo、数据库与运行状态的实际存储位置（应用本地数据目录，不可在此修改）。
      </div>
      <div class="row-input">
        <input
          :value="settings.dataRoot"
          class="input flex-1"
          readonly
          title="数据目录"
        />
        <button class="btn" type="button" @click="openDataRoot">打开目录</button>
      </div>
    </div>

    <!-- W3-C 主玩家 / 监听目录 / 通知 / 托盘 -->
    <SettingsExperience />
    <!-- 存储与备份（W3-D） -->
    <StorageBackupSection />

    <!-- Scheduling -->
    <div class="card card-pad">
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
          class="input num-input"
          @change="save"
        />
      </div>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">启动时自动发现</span>
          <span class="setting-desc">应用启动后立即执行一次 Demo 发现</span>
        </div>
        <button
          class="toggle"
          :class="{ on: settings.autoDiscoverOnStartup }"
          @click="settings.autoDiscoverOnStartup = !settings.autoDiscoverOnStartup; save()"
        >
          <span class="toggle-knob" />
        </button>
      </div>
    </div>

    <!-- Valve Bot (Auth Code) -->
    <div class="card card-pad">
      <div class="card-title">Valve Bot 账号</div>
      <div class="card-desc">
        Auth Code 通道用于发现对局；下载 demo 需 Bot 账号登录 Steam 解析分享码。
        凭证仅保存在本地数据库。
      </div>
      <div class="bot-form">
        <div class="field">
          <label class="field-label">username</label>
          <input v-model="valveBot.username" class="input" placeholder="bot_account_name" />
        </div>
        <div class="field">
          <label class="field-label">
            password
            <span v-if="valveBot.passwordSet" class="field-hint">（已保存，留空则不修改）</span>
          </label>
          <input
            v-model="valveBotPassword"
            type="password"
            class="input"
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
            class="input"
            autocomplete="new-password"
            placeholder="Steam Guard 共享密钥，可选"
          />
        </div>
        <div class="field">
          <label class="field-label">API_key</label>
          <input
            v-model="valveBot.steamApiKey"
            class="input"
            placeholder="Steam Web API Key"
          />
        </div>
        <div class="bot-actions">
          <button class="btn-primary" :disabled="valveBotSaving" @click="saveValveBot">
            {{ valveBotSaving ? '保存中…' : '保存 Bot 配置' }}
          </button>
          <StatusChip v-if="valveBotSaved" tone="success" label="已保存" />
        </div>
      </div>
    </div>

    <!-- Downloader module -->
    <DownloaderStatusCard />

    <!-- Platform settings -->
    <div class="card card-pad">
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
            class="toggle"
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
            class="input num-input"
            @change="savePlatform(p)"
          />
        </div>
      </div>
    </div>

    <!-- Background task logs -->
    <div class="card card-pad">
      <div class="card-title">后台任务日志</div>
      <div class="card-desc">查看发现与下载任务的运行日志。</div>
      <button class="btn" type="button" @click="logOpen = true">查看日志</button>
    </div>

    <!-- About -->
    <div class="card card-pad card-about">
      <div class="about-row">
        <div>
          <div class="card-title">CS Analyst</div>
          <div class="card-desc">版本 {{ appVersion }} · 自动发现并下载 CS2 Demo 文件</div>
        </div>
        <div class="about-actions">
          <button class="btn" @click="openDb">数据库目录</button>
          <button class="btn" @click="openLogs">日志目录</button>
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
  max-width: 680px;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.card-title { font-size: var(--text-md); font-weight: 600; color: var(--text); margin-bottom: 2px; }
.card-desc  { font-size: var(--text-xs); color: var(--text-3); margin-bottom: var(--space-3); line-height: 1.5; }
.card-title + .setting-row,
.card-title + .platform-section { margin-top: var(--space-2); }

.bot-form { display: flex; flex-direction: column; gap: var(--space-3); }
.bot-form .field-hint { display: inline; margin-left: var(--space-1); }
.bot-actions { display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-1); }

.row-input { display: flex; gap: var(--space-2); align-items: center; }
.flex-1 { flex: 1; }

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
.setting-desc { font-size: var(--text-xs); color: var(--text-3); }

.num-input { width: 64px; text-align: center; }

/* 平台分组 */
.platform-section { padding: var(--space-2) 0; }
.platform-section.not-last { border-bottom: 1px solid var(--border); }
.platform-name {
  margin-bottom: var(--space-1);
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--text-3);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.setting-row.sub { min-height: 38px; }

/* 关于 */
.about-row { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
.about-actions { display: flex; gap: var(--space-2); }
.card-about .card-desc { margin-bottom: 0; }
</style>
