<template>
  <div class="welcome">
    <div class="wizard">
      <header class="wizard-head">
        <div class="brand">
          <span class="brand-title">CS Analyst</span>
          <span class="brand-sub">首次使用设置</span>
        </div>
        <button type="button" class="btn-ghost btn-sm" @click="finish">跳过向导</button>
      </header>

      <!-- 步骤条 -->
      <ol class="steps">
        <li
          v-for="(s, i) in steps"
          :key="s.key"
          class="step"
          :class="{ current: i === step, done: i < step }"
          @click="i < step && (step = i)"
        >
          <span class="step-index num">{{ i + 1 }}</span>
          <span class="step-label">{{ s.label }}</span>
        </li>
      </ol>

      <section class="body">
        <!-- 1. 数据目录 -->
        <template v-if="current === 'data'">
          <h2 class="title">数据存在哪里</h2>
          <p class="desc">demo 文件、数据库和解析结果都放在这个目录。完整解析后每场大约占用 100–300 MB，建议选空间充足的磁盘。</p>
          <div class="field">
            <label class="field-label">当前数据目录</label>
            <div class="row">
              <input class="input flex-1 mono" :value="state?.dataRoot ?? ''" readonly />
              <button type="button" class="btn" :disabled="state?.dataRootLocked" @click="chooseDataRoot">更改…</button>
            </div>
            <span v-if="state?.dataRootLocked" class="field-hint">由环境变量 CS_ANALYST_DATA_ROOT 指定，不能在这里修改。</span>
            <span v-else-if="state && state.dataRoot !== state.defaultDataRoot" class="field-hint">
              默认位置是 <span class="mono">{{ state.defaultDataRoot }}</span>
              <button type="button" class="link" @click="resetDataRoot">恢复默认</button>
            </span>
            <span v-else class="field-hint">更改后应用会重启；已有的数据不会自动搬过去。</span>
          </div>
          <p v-if="dataError" class="form-error">{{ dataError }}</p>
        </template>

        <!-- 2. 获取对局 -->
        <template v-else-if="current === 'source'">
          <h2 class="title">从哪里获取对局</h2>
          <p class="desc">任选一种，之后随时可以补充。导入或下载完成后会自动解析。</p>

          <div class="options">
            <div class="option">
              <div class="option-main">
                <div class="option-title">导入 demo 文件</div>
                <div class="option-desc">选择本地的 .dem 文件，也可以直接把文件拖进窗口。</div>
              </div>
              <button type="button" class="btn-primary" @click="importFiles">选择文件…</button>
            </div>

            <div class="option">
              <div class="option-main">
                <div class="option-title">绑定平台账号</div>
                <div class="option-desc">绑定 Valve（官匹，需要 Auth Code）、5EPlay 或 Renown 账号，自动发现并下载新对局。</div>
              </div>
              <button type="button" class="btn" @click="goAccounts">去绑定</button>
            </div>

            <div class="option">
              <div class="option-main">
                <div class="option-title">
                  自动导入 CS2 录像
                  <StatusChip v-if="watch?.watching" tone="success" dot label="监听中" />
                  <StatusChip v-else-if="watch?.enabled && watch?.dir" tone="neutral" label="等待目录出现" />
                  <StatusChip v-else tone="neutral" label="未找到 CS2" />
                </div>
                <div class="option-desc">
                  <template v-if="watch?.dir">
                    在 CS2 里下载的比赛录像会从 <span class="mono">{{ watch.dir }}</span> 自动复制导入（不会挪走游戏里的文件）。
                  </template>
                  <template v-else>没有在 Steam 库里找到 CS2，可以稍后在设置里手动指定 replays 目录。</template>
                </div>
              </div>
            </div>
          </div>

          <div class="progress-line">
            <template v-if="(state?.matchCount ?? 0) > 0">
              已有 <span class="num strong">{{ state?.matchCount }}</span> 场对局，
              <span class="num strong">{{ state?.analyzedCount }}</span> 场已解析
              <StatusChip v-if="analysisQueued > 0" tone="info" pulse :label="`解析中，剩余 ${analysisQueued}`" />
            </template>
            <template v-else-if="(state?.accountCount ?? 0) > 0">
              已绑定 <span class="num strong">{{ state?.accountCount }}</span> 个账号，对局会在后台陆续下载。
            </template>
            <template v-else>还没有对局。</template>
          </div>
        </template>

        <!-- 3. 我是谁 -->
        <template v-else-if="current === 'me'">
          <h2 class="title">你是谁</h2>
          <p class="desc">选出你自己，之后的个人主页、趋势和集锦都以你的视角展示。</p>
          <div v-if="isSet" class="current-me">
            <StatusChip tone="success" dot label="已设置" />
            <span class="strong">{{ displayName ?? '（暂无名字）' }}</span>
            <span class="mono muted">{{ steamid }}</span>
          </div>
          <MainPlayerPicker />
          <p v-if="(state?.matchCount ?? 0) > 0 && (state?.analyzedCount ?? 0) === 0" class="field-hint">
            对局还在解析，解析完成后列表会自动更新。
          </p>
        </template>

        <!-- 4. 完成 -->
        <template v-else>
          <h2 class="title">准备好了</h2>
          <dl class="kv-grid summary">
            <dt>数据目录</dt>
            <dd class="mono">{{ state?.dataRoot }}</dd>
            <dt>对局</dt>
            <dd><span class="num">{{ state?.matchCount ?? 0 }}</span> 场（已解析 <span class="num">{{ state?.analyzedCount ?? 0 }}</span>）</dd>
            <dt>账号</dt>
            <dd><span class="num">{{ state?.accountCount ?? 0 }}</span> 个</dd>
            <dt>主玩家</dt>
            <dd>
              <template v-if="isSet">{{ displayName ?? steamid }} <span class="mono muted">{{ steamid }}</span></template>
              <span v-else class="muted">未设置，可以稍后在设置里选择</span>
            </dd>
          </dl>
          <ul class="tips">
            <li>关闭窗口后应用会留在托盘里，继续下载和解析；在托盘菜单里可以退出。</li>
            <li>新对局解析完成时会弹出系统通知，点击直接打开这场对局。</li>
            <li>指标旁边的「?」可以查看定义、算法和参考值，例如 <MetricHelp metric="adr" label="ADR" />。</li>
            <li>这些都可以在「设置」里关掉，也能从设置里重新打开这个向导。</li>
          </ul>
        </template>
      </section>

      <footer class="wizard-foot">
        <button v-if="step > 0" type="button" class="btn" @click="step--">上一步</button>
        <span class="spacer" />
        <button v-if="step < steps.length - 1" type="button" class="btn-ghost" @click="step++">跳过这一步</button>
        <button v-if="step < steps.length - 1" type="button" class="btn-primary" @click="step++">下一步</button>
        <button v-else type="button" class="btn-primary" @click="finish">开始使用</button>
      </footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { OnboardingStateDto, WatchStatusDto } from '@cs-analyst/shared-types';
import { listen } from '@tauri-apps/api/event';
import { listenAnalysisQueue, listenJobsUpdated, tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: false });

const steps = [
  { key: 'data', label: '数据目录' },
  { key: 'source', label: '获取对局' },
  { key: 'me', label: '我是谁' },
  { key: 'done', label: '完成' },
] as const;

// 从其他页面（例如账户页）回来时停在原来的步骤
const step = useState('onboarding-wizard-step', () => 0);
const wizardActive = useState('onboarding-wizard-active', () => false);
const current = computed(() => steps[step.value]?.key ?? 'data');

const state = ref<OnboardingStateDto | null>(null);
const watch = ref<WatchStatusDto | null>(null);
const analysisQueued = ref(0);
const dataError = ref('');
const toast = useToast();
const { steamid, displayName, isSet, refresh: refreshMainPlayer } = useMainPlayer();

async function load() {
  try {
    state.value = await tauriInvoke<OnboardingStateDto>('get_onboarding_state');
    watch.value = await tauriInvoke<WatchStatusDto>('get_watch_status');
  } catch {
    /* 浏览器 */
  }
}

async function chooseDataRoot() {
  dataError.value = '';
  const dir = await openDialog({ directory: true, multiple: false, title: '选择数据目录' });
  if (typeof dir !== 'string' || !dir) return;
  if ((state.value?.matchCount ?? 0) > 0 && !confirm('当前目录里已经有对局，切换后不会自动搬过去。确定切换并重启吗？')) return;
  try {
    await tauriInvoke('set_data_root', { path: dir });
  } catch (e) {
    dataError.value = e instanceof Error ? e.message : String(e);
  }
}

async function resetDataRoot() {
  try {
    await tauriInvoke('set_data_root', { path: null });
  } catch (e) {
    dataError.value = e instanceof Error ? e.message : String(e);
  }
}

async function importFiles() {
  const selected = await openDialog({
    multiple: true,
    title: '选择 CS2 demo',
    filters: [{ name: 'CS2 Demo', extensions: ['dem'] }],
  });
  const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
  if (paths.length === 0) return;
  try {
    await tauriInvoke('import_demos', { filePaths: paths });
    toast.add({ title: `开始导入 ${paths.length} 个 demo`, color: 'neutral' });
  } catch (e) {
    toast.add({ title: '导入失败', description: e instanceof Error ? e.message : String(e), color: 'error' });
  }
}

function goAccounts() {
  wizardActive.value = true;
  void navigateTo('/accounts');
}

async function finish() {
  try {
    await tauriInvoke('complete_onboarding');
  } catch {
    /* 浏览器 */
  }
  wizardActive.value = false;
  step.value = 0;
  await navigateTo('/matches');
}

const offs: Array<() => void> = [];
offs.push(listenJobsUpdated(() => void load()));
offs.push(
  listenAnalysisQueue((q) => {
    analysisQueued.value = q.queued + (q.running ? 1 : 0);
    void load();
  }),
);
let offWatch: (() => void) | null = null;
listen<WatchStatusDto>('watch_status', (e) => (watch.value = e.payload))
  .then((fn) => (offWatch = fn))
  .catch(() => {});

onMounted(async () => {
  wizardActive.value = true;
  await load();
  await refreshMainPlayer();
  try {
    const q = await tauriInvoke<{ queued: number; running?: string | null }>('get_analysis_status');
    analysisQueued.value = q.queued + (q.running ? 1 : 0);
  } catch {
    /* 浏览器 */
  }
});
onBeforeUnmount(() => {
  offs.forEach((off) => off());
  offWatch?.();
});
</script>

<style scoped>
.welcome {
  min-height: 100vh;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: 8vh var(--space-6) var(--space-6);
  background: var(--bg);
}
.wizard {
  width: 100%;
  max-width: 720px;
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.wizard-head { display: flex; align-items: center; justify-content: space-between; }
.brand { display: flex; align-items: baseline; gap: var(--space-2); }
.brand-title { font-size: var(--text-lg); font-weight: 700; color: var(--text); }
.brand-sub { font-size: var(--text-sm); color: var(--text-3); }

.steps { display: flex; gap: var(--space-2); list-style: none; margin: 0; padding: 0; }
.step {
  flex: 1;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-top: var(--space-2);
  border-top: 2px solid var(--border);
  color: var(--text-3);
  font-size: var(--text-sm);
}
.step.done { cursor: pointer; color: var(--text-2); border-top-color: var(--border-strong); }
.step.current { color: var(--text); border-top-color: var(--accent); }
.step-index {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1px solid currentColor;
  font-size: var(--text-xs);
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-height: 340px;
  padding: var(--space-6);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}
.title { margin: 0; font-size: var(--text-xl); font-weight: 600; color: var(--text); }
.desc { margin: 0 0 var(--space-2); font-size: var(--text-sm); color: var(--text-2); line-height: 1.6; }

.row { display: flex; gap: var(--space-2); }
.flex-1 { flex: 1; min-width: 0; }
.link {
  margin-left: var(--space-2);
  padding: 0;
  border: none;
  background: none;
  color: var(--text-2);
  text-decoration: underline;
  cursor: pointer;
  font-size: inherit;
}

.options { display: flex; flex-direction: column; gap: var(--space-2); }
.option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}
.option-main { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.option-title { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-md); font-weight: 600; color: var(--text); }
.option-desc { font-size: var(--text-xs); color: var(--text-3); line-height: 1.5; word-break: break-all; }
.progress-line { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-1); margin-top: var(--space-2); font-size: var(--text-sm); color: var(--text-2); }

.current-me { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); }

.summary { margin: 0; }
.tips { margin: var(--space-2) 0 0; padding-left: var(--space-5); color: var(--text-2); font-size: var(--text-sm); line-height: 1.8; }

.wizard-foot { display: flex; align-items: center; gap: var(--space-2); }
.spacer { flex: 1; }
</style>
