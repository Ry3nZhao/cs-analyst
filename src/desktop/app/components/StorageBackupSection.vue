<template>
  <div class="storage-section">
    <!-- 占用 -->
    <div class="card card-pad">
      <SectionHeader title="存储占用" description="按类别、平台和账号统计应用数据目录里的实际文件大小。">
        <button class="btn btn-sm" :disabled="usageLoading" @click="loadUsage">{{ usageLoading ? '统计中…' : '刷新' }}</button>
      </SectionHeader>
      <div class="tiles">
        <StatTile label="原始 demo" :value="fmtBytes(usage?.demoBytes)" :hint="demoHint" compact />
        <StatTile label="回放文件" :value="fmtBytes(usage?.replayBytes)" compact />
        <StatTile label="数据库" :value="fmtBytes(usage?.databaseBytes)" compact />
        <StatTile label="备份与导出" :value="fmtBytes(usage ? usage.backupBytes + usage.exportBytes : null)" compact />
      </div>
      <div v-if="usage" class="usage-meta muted">
        <span>原始 {{ usage.rawCount }}</span>
        <span>已压缩 {{ usage.compressedCount }}</span>
        <span>已删除 {{ usage.deletedCount }}</span>
        <span>收藏 {{ usage.favoriteCount }}</span>
        <span v-if="usage.savedBytes > 0">已节省 {{ fmtBytes(usage.savedBytes) }}</span>
        <span v-if="usage.externalDemoBytes > 0" title="不在应用数据目录里的 demo，不参与压缩和清理">外部 demo {{ fmtBytes(usage.externalDemoBytes) }}</span>
      </div>
      <div v-if="usage" class="usage-tables">
        <div v-for="g in usageGroups" :key="g.title" class="table-wrap">
          <table class="data-table compact">
            <thead><tr><th>{{ g.title }}</th><th class="num">对局</th><th class="num">demo</th><th class="num">回放</th></tr></thead>
            <tbody>
              <tr v-for="row in g.rows" :key="row.key">
                <td class="truncate strong" :title="row.label">{{ row.label }}</td>
                <td class="num">{{ row.matches }}</td>
                <td class="num">{{ fmtBytes(row.demoBytes) }}</td>
                <td class="num">{{ fmtBytes(row.replayBytes) }}</td>
              </tr>
              <tr v-if="g.rows.length === 0"><td colspan="4" class="empty">暂无对局</td></tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <!-- 原始 demo 策略 -->
    <div class="card card-pad">
      <SectionHeader title="原始 demo" description="解析成功、数据入库后，对原始 .dem 文件的处理方式。只处理应用数据目录里的 demo。" />
      <div class="policy-options" role="radiogroup" aria-label="原始 demo 处理方式">
        <button
          v-for="opt in POLICY_OPTIONS"
          :key="opt.value"
          type="button"
          role="radio"
          class="policy-option"
          :class="{ selected: settings.demoPolicy === opt.value }"
          :aria-checked="settings.demoPolicy === opt.value"
          @click="choosePolicy(opt.value)"
        >
          <span class="policy-name">{{ opt.label }}</span>
          <span class="policy-desc">{{ opt.desc }}</span>
        </button>
      </div>
      <div v-if="settings.demoPolicy === 'delete'" class="callout callout-danger">
        删除原始 demo 后<strong>无法重新解析</strong>（解析器升级后也不能补算新指标），也<strong>无法在 CS2 里播放集锦</strong>。
        收藏的对局不会被删除。
      </div>
      <div v-else-if="settings.demoPolicy === 'compress'" class="callout callout-info">
        压缩为 .dem.zst，重新解析时自动解压，无需手动处理；在 CS2 里播放集锦前需要先解压。
      </div>
      <div class="row-actions">
        <span class="muted small">对已有的 demo 执行一次：</span>
        <button class="btn btn-sm" @click="openPlan('compress')">压缩已有 demo…</button>
        <button class="btn-danger btn-sm" @click="openPlan('delete')">删除已有 demo…</button>
      </div>
    </div>

    <!-- 自动清理 -->
    <div class="card card-pad">
      <SectionHeader title="自动清理" description="按天数或总容量删除原始 demo，已入库的数据保留。收藏的对局永不清理。" />
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">早于</span>
          <span class="setting-desc">对局时间早于 N 天的 demo；留空不启用</span>
        </div>
        <div class="inline-input">
          <input v-model.number="rules.maxAgeDays" type="number" min="1" class="input num-input" placeholder="—" @change="saveRules" />
          <span class="muted">天</span>
        </div>
      </div>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">总容量上限</span>
          <span class="setting-desc">超出时从最旧的开始清理；留空不启用</span>
        </div>
        <div class="inline-input">
          <input v-model.number="rules.maxTotalGb" type="number" min="1" step="1" class="input num-input" placeholder="—" @change="saveRules" />
          <span class="muted">GB</span>
        </div>
      </div>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-name">启动时自动清理</span>
          <span class="setting-desc">
            {{ settings.lastCleanupAt ? `上次：${fmtDateTime(settings.lastCleanupAt)} · ${settings.lastCleanupSummary ?? ''}` : '开启前会先预览受影响的对局' }}
          </span>
        </div>
        <button class="toggle" :class="{ on: settings.autoCleanup }" :aria-pressed="settings.autoCleanup" @click="toggleAuto">
          <span class="toggle-knob" />
        </button>
      </div>
      <div class="row-actions">
        <button class="btn btn-sm" :disabled="!hasRule" @click="openPlan('cleanup')">预览并清理…</button>
      </div>
    </div>

    <!-- 导出 -->
    <div class="card card-pad">
      <SectionHeader title="导出数据" description="计分板、回合和各事件表。CSV 每张表一个文件；JSON 合成一个文件，SteamID 以字符串保存。" />
      <div class="export-row">
        <select v-model="exportFormat" class="select">
          <option value="csv">CSV</option>
          <option value="json">JSON</option>
        </select>
        <select v-model="exportScope" class="select">
          <option value="all">全部对局</option>
          <option value="selected">选中的对局</option>
        </select>
        <button v-if="exportScope === 'selected'" class="btn btn-sm" @click="pickerOpen = true">
          选择对局（{{ pickedIds.length }}）
        </button>
        <button class="btn" :disabled="exporting || (exportScope === 'selected' && pickedIds.length === 0)" @click="runExport">
          {{ exporting ? '导出中…' : '导出' }}
        </button>
      </div>
      <div v-if="exportResult" class="result-line">
        <span class="muted">已导出 {{ exportResult.matches }} 场到</span>
        <code class="mono path" :title="exportResult.dir">{{ exportResult.dir }}</code>
        <button class="btn btn-sm" @click="openPath(exportResult.dir)">打开文件夹</button>
      </div>
    </div>

    <!-- 备份与恢复 -->
    <div class="card card-pad">
      <SectionHeader title="备份与恢复" description="生成数据库的一致快照（不复制 -wal / -shm 文件）。恢复前会自动备份当前数据库。" :count="backups.length">
        <button v-if="backups.length" class="btn btn-sm" @click="openBackupsDir">打开目录</button>
      </SectionHeader>
      <div class="export-row">
        <label class="check">
          <input v-model="includeReplays" type="checkbox" />
          同时打包回放文件<span v-if="usage" class="muted">（{{ fmtBytes(usage.replayBytes) }}）</span>
        </label>
        <button class="btn" :disabled="backingUp" @click="runBackup">{{ backingUp ? '备份中…' : '立即备份' }}</button>
        <button class="btn btn-sm" @click="pickRestore">从文件夹恢复…</button>
      </div>
      <div v-if="backups.length" class="table-wrap backup-table">
        <table class="data-table compact">
          <thead>
            <tr><th>备份</th><th class="num">对局</th><th class="num">大小</th><th>内容</th><th class="actions"></th></tr>
          </thead>
          <tbody>
            <tr v-for="b in backups" :key="b.path">
              <td class="mono nowrap">{{ b.name }}</td>
              <td class="num">{{ b.manifest?.matches ?? '—' }}</td>
              <td class="num">{{ fmtBytes(b.bytes) }}</td>
              <td class="muted nowrap">
                <span v-if="b.manifest?.kind === 'pre-restore'" class="tag">恢复前自动</span>
                {{ b.manifest?.includesReplays ? `含回放 ${b.manifest.replayFiles}` : '仅数据库' }}
              </td>
              <td class="actions"><button class="btn-danger btn-sm" @click="askRestore(b.path, b)">恢复</button></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 弹窗 -->
    <StoragePlanModal
      v-model:open="planOpen"
      :kind="planKind"
      :rules="rules"
      :confirm-only="planConfirmOnly"
      confirm-label="开启自动清理"
      @done="afterPlan"
      @confirmed="enableAuto"
    />

    <AppModal v-model:open="policyConfirmOpen" title="解析后删除原始 demo？">
      <div class="callout callout-danger">
        之后每场解析完成的对局都会删除原始 demo：<strong>无法重新解析</strong>，也<strong>无法在 CS2 里播放集锦</strong>。
        已有的 demo 不受影响（可在下方单独处理）。
      </div>
      <template #footer>
        <button class="btn" @click="policyConfirmOpen = false">取消</button>
        <button class="btn btn-danger danger-solid" @click="savePolicy('delete'); policyConfirmOpen = false">确认改为删除</button>
      </template>
    </AppModal>

    <AppModal v-model:open="restoreOpen" title="恢复备份">
      <div class="restore-body">
        <dl class="kv-grid">
          <dt>备份</dt><dd class="mono">{{ restoreTarget?.name }}</dd>
          <dt>对局</dt><dd>{{ restoreTarget?.manifest?.matches ?? '—' }}</dd>
          <dt>时间</dt><dd>{{ restoreTarget?.manifest ? fmtDateTime(restoreTarget.manifest.createdAt) : '—' }}</dd>
          <dt>回放文件</dt><dd>{{ restoreTarget?.manifest?.includesReplays ? `${restoreTarget.manifest.replayFiles} 个` : '不包含' }}</dd>
        </dl>
        <div class="callout callout-warning">
          当前数据库会被备份内容<strong>整体替换</strong>，备份之后的新对局、收藏和设置都会回到备份时的状态。
          恢复前会自动备份当前数据库（含回放文件），可以再恢复回来。
        </div>
        <label class="ack">
          <input v-model="restoreAck" type="checkbox" />
          我已了解，用这个备份替换当前数据
        </label>
        <div v-if="restoreError" class="form-error">{{ restoreError }}</div>
      </div>
      <template #footer>
        <button class="btn" :disabled="restoring" @click="restoreOpen = false">取消</button>
        <button class="btn btn-danger danger-solid" :disabled="!restoreAck || restoring" @click="runRestore">
          {{ restoring ? '恢复中…' : '恢复' }}
        </button>
      </template>
    </AppModal>

    <AppModal v-model:open="pickerOpen" title="选择要导出的对局">
      <input v-model="pickerSearch" class="input picker-search" placeholder="搜索地图、来源…" />
      <div class="table-wrap picker-table">
        <table class="data-table compact">
          <tbody>
            <tr v-for="m in pickerRows" :key="m.id" class="clickable" @click="togglePick(m.id)">
              <td class="pick-col"><input type="checkbox" :checked="pickedSet.has(m.id)" @click.stop="togglePick(m.id)" /></td>
              <td class="strong truncate">{{ m.mapName || m.matchId }}</td>
              <td class="muted">{{ m.source }}</td>
              <td class="muted nowrap">{{ fmtDateTime(m.playedAt) }}</td>
            </tr>
            <tr v-if="pickerRows.length === 0"><td colspan="4" class="empty">没有已解析的对局</td></tr>
          </tbody>
        </table>
      </div>
      <template #footer>
        <span class="muted picker-count">已选 {{ pickedIds.length }}</span>
        <button class="btn btn-sm" @click="pickedIds = pickerRows.map(m => m.id)">全选</button>
        <button class="btn btn-sm" @click="pickedIds = []">清空</button>
        <button class="btn-primary" @click="pickerOpen = false">完成</button>
      </template>
    </AppModal>
  </div>
</template>

<script setup lang="ts">
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type {
  BackupInfoDto, CleanupRulesDto, DemoPolicy, ExportFormat, ExportResultDto, MatchDto, RestoreResultDto,
  StoragePlanKind, StorageSettingsDto, StorageUsageDto,
} from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import { fmtBytes } from '~/composables/useStorageFormat';

const toast = useToast();

const POLICY_OPTIONS: { value: DemoPolicy; label: string; desc: string }[] = [
  { value: 'keep', label: '保留', desc: '不做处理，占用最大' },
  { value: 'compress', label: '压缩', desc: '.dem.zst，可重新解析' },
  { value: 'delete', label: '删除只留数据', desc: '不可重新解析' },
];

const settings = reactive<StorageSettingsDto>({
  demoPolicy: 'keep', autoCleanup: false, cleanupMaxAgeDays: null, cleanupMaxTotalGb: null,
});
const rules = reactive<CleanupRulesDto>({ maxAgeDays: null, maxTotalGb: null });
const usage = ref<StorageUsageDto | null>(null);
const usageLoading = ref(false);
const backups = ref<BackupInfoDto[]>([]);

const hasRule = computed(() => Boolean(rules.maxAgeDays && rules.maxAgeDays > 0) || Boolean(rules.maxTotalGb && rules.maxTotalGb > 0));
const demoHint = computed(() => (usage.value?.savedBytes ? `已节省 ${fmtBytes(usage.value.savedBytes)}` : undefined));
const usageGroups = computed(() => [
  { title: '平台', rows: usage.value?.byPlatform ?? [] },
  { title: '账号', rows: usage.value?.byAccount ?? [] },
]);

function errMsg(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

async function loadSettings() {
  Object.assign(settings, await tauriInvoke<StorageSettingsDto>('get_storage_settings'));
  rules.maxAgeDays = settings.cleanupMaxAgeDays;
  rules.maxTotalGb = settings.cleanupMaxTotalGb;
}

async function loadUsage() {
  usageLoading.value = true;
  try {
    usage.value = await tauriInvoke<StorageUsageDto>('get_storage_usage');
  } catch (e) {
    toast.add({ title: '统计占用失败', description: errMsg(e), color: 'error' });
  } finally {
    usageLoading.value = false;
  }
}

async function loadBackups() {
  backups.value = await tauriInvoke<BackupInfoDto[]>('list_backups');
}

async function saveSettings(patch: Partial<StorageSettingsDto>) {
  const next = { ...settings, ...patch };
  try {
    Object.assign(settings, await tauriInvoke<StorageSettingsDto>('update_storage_settings', { settings: next }));
  } catch (e) {
    toast.add({ title: '保存失败', description: errMsg(e), color: 'error' });
    await loadSettings();
  }
}

// ---- 策略
const policyConfirmOpen = ref(false);
function choosePolicy(p: DemoPolicy) {
  if (p === settings.demoPolicy) return;
  // 改为「删除」需要二次确认
  if (p === 'delete') policyConfirmOpen.value = true;
  else savePolicy(p);
}
function savePolicy(p: DemoPolicy) {
  saveSettings({ demoPolicy: p });
}

// ---- 清理规则
function normRule(v: number | null | '' | undefined) {
  const n = Number(v);
  return v === '' || v == null || !Number.isFinite(n) || n <= 0 ? null : n;
}
async function saveRules() {
  rules.maxAgeDays = normRule(rules.maxAgeDays) != null ? Math.floor(normRule(rules.maxAgeDays)!) : null;
  rules.maxTotalGb = normRule(rules.maxTotalGb);
  // 删除全部规则时同时关闭自动清理
  const autoCleanup = settings.autoCleanup && hasRule.value;
  await saveSettings({ cleanupMaxAgeDays: rules.maxAgeDays, cleanupMaxTotalGb: rules.maxTotalGb, autoCleanup });
}

// ---- 计划（预览 → 确认 → 执行）
const planOpen = ref(false);
const planKind = ref<StoragePlanKind>('cleanup');
const planConfirmOnly = ref(false);
function openPlan(kind: StoragePlanKind, confirmOnly = false) {
  planKind.value = kind;
  planConfirmOnly.value = confirmOnly;
  planOpen.value = true;
}
function toggleAuto() {
  if (settings.autoCleanup) {
    saveSettings({ autoCleanup: false });
    return;
  }
  if (!hasRule.value) {
    toast.add({ title: '先设置清理规则', description: '至少设置天数或总容量中的一项', color: 'warning' });
    return;
  }
  // 开启前先预览会被清理的对局
  openPlan('cleanup', true);
}
async function enableAuto() {
  planOpen.value = false;
  await saveSettings({ autoCleanup: true, cleanupMaxAgeDays: rules.maxAgeDays, cleanupMaxTotalGb: rules.maxTotalGb });
  toast.add({ title: '已开启自动清理', description: '每次启动约 1 分钟后按规则清理', color: 'success' });
}
async function afterPlan() {
  await Promise.all([loadUsage(), loadSettings()]);
}

// ---- 导出
const exportFormat = ref<ExportFormat>('csv');
const exportScope = ref<'all' | 'selected'>('all');
const exporting = ref(false);
const exportResult = ref<ExportResultDto | null>(null);
const pickerOpen = ref(false);
const pickerSearch = ref('');
const allMatches = ref<MatchDto[]>([]);
const pickedIds = ref<string[]>([]);
const pickedSet = computed(() => new Set(pickedIds.value));
const pickerRows = computed(() => {
  const q = pickerSearch.value.toLowerCase();
  return allMatches.value
    .filter(m => m.analysisStatus === 'done')
    .filter(m => !q || (m.mapName ?? '').toLowerCase().includes(q) || m.source.toLowerCase().includes(q));
});
function togglePick(id: string) {
  pickedIds.value = pickedSet.value.has(id) ? pickedIds.value.filter(x => x !== id) : [...pickedIds.value, id];
}
watch(pickerOpen, async (v) => {
  if (v && allMatches.value.length === 0) allMatches.value = await tauriInvoke<MatchDto[]>('list_matches');
});
async function runExport() {
  exporting.value = true;
  exportResult.value = null;
  try {
    exportResult.value = await tauriInvoke<ExportResultDto>('export_matches', {
      ids: exportScope.value === 'selected' ? pickedIds.value : null,
      format: exportFormat.value,
    });
    loadUsage();
  } catch (e) {
    toast.add({ title: '导出失败', description: errMsg(e), color: 'error' });
  } finally {
    exporting.value = false;
  }
}

// ---- 备份 / 恢复
const includeReplays = ref(true);
const backingUp = ref(false);
async function runBackup() {
  backingUp.value = true;
  try {
    const b = await tauriInvoke<BackupInfoDto>('create_backup', { includeReplays: includeReplays.value });
    toast.add({ title: '备份完成', description: `${b.name} · ${fmtBytes(b.bytes)}`, color: 'success' });
    await Promise.all([loadBackups(), loadUsage()]);
  } catch (e) {
    toast.add({ title: '备份失败', description: errMsg(e), color: 'error' });
  } finally {
    backingUp.value = false;
  }
}

const restoreOpen = ref(false);
const restoreAck = ref(false);
const restoring = ref(false);
const restoreError = ref('');
const restorePath = ref('');
const restoreTarget = ref<BackupInfoDto | null>(null);
function askRestore(path: string, info: BackupInfoDto | null) {
  restorePath.value = path;
  restoreTarget.value = info ?? { name: path, path, bytes: 0, manifest: null };
  restoreAck.value = false;
  restoreError.value = '';
  restoreOpen.value = true;
}
async function pickRestore() {
  const dir = await openDialog({ directory: true, multiple: false, title: '选择备份文件夹（包含 analyst.db）' });
  if (typeof dir !== 'string') return;
  try {
    await tauriInvoke<number>('inspect_backup', { path: dir });
  } catch (e) {
    toast.add({ title: '不能用于恢复', description: errMsg(e), color: 'error' });
    return;
  }
  askRestore(dir, backups.value.find(b => b.path === dir) ?? null);
}
async function runRestore() {
  if (!restoreAck.value) return;
  restoring.value = true;
  restoreError.value = '';
  try {
    const r = await tauriInvoke<RestoreResultDto>('restore_backup', { path: restorePath.value, confirm: 'RESTORE' });
    restoreOpen.value = false;
    toast.add({
      title: '恢复完成',
      description: `${r.matches} 场对局。当前数据已备份为 ${r.preRestoreBackup.name}。建议重启应用。`,
      color: 'success',
    });
    await Promise.all([loadSettings(), loadUsage(), loadBackups()]);
  } catch (e) {
    restoreError.value = errMsg(e);
  } finally {
    restoring.value = false;
  }
}

async function openBackupsDir() {
  await tauriInvoke('open_data_subdirectory', { subdir: 'backups' }).catch(() => {});
}

async function openPath(path: string) {
  if (!path) return;
  try {
    await tauriInvoke('open_path', { path });
  } catch (e) {
    toast.add({ title: '无法打开', description: errMsg(e), color: 'error' });
  }
}

function fmtDateTime(s: string | null | undefined) {
  if (!s) return '—';
  const d = new Date(s.includes('T') ? s : `${s.replace(' ', 'T')}Z`);
  return Number.isNaN(d.getTime()) ? s : d.toLocaleString('zh-CN', { hour12: false });
}

onMounted(async () => {
  try {
    await Promise.all([loadSettings(), loadBackups()]);
  } catch (e) {
    toast.add({ title: '读取存储设置失败', description: errMsg(e), color: 'error' });
  }
  loadUsage();
});
</script>

<style scoped>
.storage-section { display: flex; flex-direction: column; gap: var(--space-3); }

.tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--space-2); margin-top: var(--space-2); }
.usage-meta { display: flex; flex-wrap: wrap; gap: var(--space-3); font-size: var(--text-xs); margin-top: var(--space-2); }
.usage-tables { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); margin-top: var(--space-3); }
.usage-tables .table-wrap { max-height: 180px; overflow-y: auto; }
.usage-tables td.truncate { max-width: 120px; }

.policy-options { display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-2); margin: var(--space-2) 0 var(--space-3); }
.policy-option {
  display: flex; flex-direction: column; align-items: flex-start; gap: 2px;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border); border-radius: var(--radius-md);
  background: var(--surface-raised); color: var(--text-2); cursor: pointer; text-align: left;
}
.policy-option:hover { border-color: var(--border-strong); }
.policy-option.selected { border-color: var(--accent-line); background: var(--surface-active); color: var(--text); box-shadow: inset 2px 0 0 var(--accent); }
.policy-name { font-size: var(--text-sm); font-weight: 600; }
.policy-desc { font-size: var(--text-xs); color: var(--text-3); }

.row-actions { display: flex; align-items: center; gap: var(--space-2); margin-top: var(--space-3); flex-wrap: wrap; }
.small { font-size: var(--text-xs); }

.setting-row {
  display: flex; align-items: center; justify-content: space-between; gap: var(--space-4);
  min-height: 44px; border-bottom: 1px solid var(--border-subtle);
}
.setting-info { display: flex; flex-direction: column; gap: 2px; }
.setting-name { font-size: var(--text-sm); color: var(--text); }
.setting-desc { font-size: var(--text-xs); color: var(--text-3); }
.inline-input { display: flex; align-items: center; gap: var(--space-2); }
.num-input { width: 72px; text-align: center; }

.export-row { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; margin-top: var(--space-2); }
.export-row .select { width: auto; }
.result-line { display: flex; align-items: center; gap: var(--space-2); margin-top: var(--space-3); font-size: var(--text-xs); min-width: 0; }
.path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-2); }
.check { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); color: var(--text-2); margin-right: auto; }

.backup-table { margin-top: var(--space-3); max-height: 220px; overflow-y: auto; }

.restore-body { display: flex; flex-direction: column; gap: var(--space-3); }
.ack { display: flex; gap: var(--space-2); align-items: flex-start; font-size: var(--text-sm); color: var(--text-2); }
.ack input { margin-top: 2px; }
.danger-solid:not(:disabled) { color: var(--danger); border-color: var(--danger-line); }

.picker-search { width: 100%; margin-bottom: var(--space-2); }
.picker-table { max-height: 320px; overflow-y: auto; }
.pick-col { width: 28px; }
.picker-count { margin-right: auto; font-size: var(--text-xs); }
</style>
