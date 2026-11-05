<template>
  <div class="training-page">
    <!-- ── 列表 ── -->
    <template v-if="stage === 'list'">
      <div class="card card-pad sens-card" data-testid="training-sensitivity">
        <div class="sens-main">
          <div class="sens-info">
            <div class="sens-title">鼠标灵敏度</div>
            <div class="sens-desc">
              和 CS2 一致：每个鼠标计数转 sensitivity × 0.022°，画面竖直视场 73.7°（游戏默认）。
            </div>
          </div>
          <div class="sens-value">
            <input
              v-model.number="sensInput"
              type="number"
              min="0.01"
              max="20"
              step="0.01"
              class="input sens-input num"
              aria-label="灵敏度"
              @change="applyManual"
            />
            <button class="btn" type="button" :disabled="sens.detecting.value" @click="sens.detect(true)">
              {{ sens.detecting.value ? '读取中…' : '从 CS2 读取' }}
            </button>
            <button class="btn" type="button" @click="sens.pickFile()">选择配置文件…</button>
          </div>
        </div>
        <div class="sens-meta">
          <template v-if="sens.current.value">
            <span class="tag">{{ sourceLabel }}</span>
            <span v-if="sens.current.value.path" class="muted truncate" :title="sens.current.value.path">{{ sens.current.value.path }}</span>
            <span class="dim num">每计数 {{ (sens.current.value.value * 0.022).toFixed(4) }}° · 转 360° 需 {{ countsPer360 }} 计数</span>
          </template>
          <span v-else class="muted">还没有灵敏度，请从 CS2 读取或手动输入。</span>
        </div>
        <div v-if="sens.candidates.value.length > 1" class="sens-candidates">
          <span class="field-label">本机有多个账号的配置：</span>
          <button
            v-for="c in sens.candidates.value"
            :key="c.path"
            type="button"
            class="btn btn-sm"
            :disabled="c.sensitivity == null"
            :title="c.path"
            @click="sens.set({ value: c.sensitivity!, source: 'cs2', path: c.path, steamid: c.steamid })"
          >
            {{ c.accountId }} · {{ c.sensitivity ?? '无' }}
          </button>
        </div>
        <p v-if="sens.detectError.value" class="callout callout-warning sens-error">
          {{ sens.detectError.value }}。可以手动输入，或选择 <code>userdata/&lt;id&gt;/730/local/cfg/cs2_user_convars_0_slot0.vcfg</code>。
        </p>
      </div>

      <p class="callout callout-info">
        训练在应用窗口里运行，WebView 有一定的输入和显示延迟。反应测试的数值只适合和自己以前的成绩比，不能和其他工具或网站横向比较。
      </p>

      <SectionHeader title="训练模式" size="lg" />
      <div class="mode-grid">
        <button
          v-for="m in TRAINING_MODES"
          :key="m.mode"
          type="button"
          class="mode-card"
          :data-testid="`mode-${m.mode}`"
          @click="openSetup(m.mode)"
        >
          <div class="mode-head">
            <span class="mode-name">{{ m.label }}</span>
            <span class="mode-icon" v-html="MODE_ICONS[m.mode]" />
          </div>
          <div class="mode-desc">{{ m.description }}</div>
          <div class="mode-stats">
            <template v-if="summary[m.mode]">
              <span>上次 <b class="num">{{ fmtScore(summary[m.mode]!.last, m) }}</b></span>
              <span>最好 <b class="num">{{ fmtScore(summary[m.mode]!.best, m) }}</b></span>
              <span class="dim num">{{ summary[m.mode]!.count }} 次</span>
            </template>
            <span v-else class="dim">还没有记录</span>
          </div>
        </button>
      </div>
    </template>

    <!-- ── 设置 ── -->
    <template v-else-if="stage === 'setup' && meta">
      <div class="crumbs">
        <button class="btn-ghost btn-sm" type="button" @click="backToList">← 训练</button>
      </div>
      <div class="card card-pad setup-card">
        <SectionHeader :title="meta.label" :description="meta.description" size="lg" />
        <div class="setup-grid">
          <label v-if="hasCount" class="field">
            <span class="field-label">{{ countLabel }}</span>
            <select v-model.number="opts.count" class="select">
              <option v-for="n in countChoices" :key="n" :value="n">{{ n }}</option>
            </select>
          </label>
          <label v-if="mode === 'tracking'" class="field">
            <span class="field-label">时长</span>
            <select v-model.number="opts.durationS" class="select">
              <option :value="20">20 秒</option>
              <option :value="30">30 秒</option>
              <option :value="60">60 秒</option>
            </select>
          </label>
          <label v-if="mode === 'flick' || mode === 'micro' || mode === 'tracking'" class="field">
            <span class="field-label">目标大小</span>
            <select v-model="opts.size" class="select">
              <option value="L">大</option>
              <option value="M">中</option>
              <option value="S">小</option>
            </select>
          </label>
          <label v-if="mode === 'flick' || mode === 'micro'" class="field">
            <span class="field-label">单个目标时限</span>
            <select v-model.number="opts.timeLimitMs" class="select">
              <option :value="1000">1.0 秒</option>
              <option :value="1500">1.5 秒</option>
              <option :value="2000">2.0 秒</option>
              <option :value="2500">2.5 秒</option>
            </select>
          </label>
          <label v-if="mode === 'tracking'" class="field">
            <span class="field-label">目标速度</span>
            <select v-model="opts.speed" class="select">
              <option value="slow">慢（10°/s）</option>
              <option value="medium">中（16°/s）</option>
              <option value="fast">快（24°/s）</option>
            </select>
          </label>
          <label v-if="mode === 'strafe'" class="field">
            <span class="field-label">武器（决定最大速度）</span>
            <select v-model="opts.strafeWeapon" class="select">
              <option v-for="(w, k) in STRAFE_WEAPONS" :key="k" :value="k">{{ w.label }} · {{ w.maxSpeed }} u/s</option>
            </select>
          </label>
          <label v-if="mode === 'spray'" class="field">
            <span class="field-label">武器</span>
            <select v-model="opts.sprayWeapon" class="select" data-testid="spray-weapon">
              <option v-for="(w, k) in SPRAY_PATTERNS" :key="k" :value="k">{{ w.label }} · {{ w.shots.length }} 发</option>
            </select>
          </label>
          <label v-if="mode === 'spray'" class="field">
            <span class="field-label">连发次数</span>
            <select v-model.number="opts.sprays" class="select">
              <option :value="5">5</option>
              <option :value="8">8</option>
              <option :value="12">12</option>
            </select>
          </label>
        </div>

        <ul class="setup-notes">
          <li v-for="(n, i) in setupNotes" :key="i">{{ n }}</li>
        </ul>

        <p v-if="!sens.current.value && meta.pointerLock" class="callout callout-warning">
          还没有设置灵敏度，先回到训练列表从 CS2 读取或手动输入。
        </p>

        <div class="setup-actions">
          <span class="muted num">灵敏度 {{ sens.current.value?.value ?? '—' }}</span>
          <button
            class="btn-primary"
            type="button"
            data-testid="training-go"
            :disabled="meta.pointerLock && !sens.current.value"
            @click="run"
          >
            开始训练
          </button>
        </div>
      </div>

      <div class="card card-pad">
        <SectionHeader :title="`${meta.scoreLabel}趋势`">
          <RangeSwitch v-model="range" />
        </SectionHeader>
        <TrainingTrend :values="trendValues" :label="meta.scoreLabel" :unit="meta.scoreUnit" :higher-is-better="meta.higherIsBetter" />
      </div>
    </template>

    <!-- ── 结果 ── -->
    <template v-else-if="stage === 'result' && meta && finish">
      <div class="crumbs">
        <button class="btn-ghost btn-sm" type="button" @click="backToList">← 训练</button>
      </div>
      <div class="card card-pad" data-testid="training-result">
        <SectionHeader :title="`${meta.label} · 结果`" :description="resultDesc" size="lg">
          <StatusChip v-if="saveState === 'saved'" tone="success" label="已保存" />
          <StatusChip v-else-if="saveState === 'saving'" tone="info" pulse label="保存中" />
          <StatusChip v-else-if="saveState === 'error'" tone="danger" :label="`保存失败`" :title="saveError ?? ''" />
          <StatusChip v-else-if="saveState === 'skipped'" label="没有有效成绩，未保存" />
          <button class="btn" type="button" @click="openSetup(mode)">调整设置</button>
          <button class="btn-primary" type="button" data-testid="training-again" @click="run">再来一次</button>
        </SectionHeader>
        <div class="tiles">
          <StatTile
            v-for="t in tiles"
            :key="t.label"
            :label="t.label"
            :value="t.value ?? '—'"
            :unit="t.value == null ? undefined : t.unit"
            :hint="t.hint"
            :tone="t.tone ?? 'neutral'"
            :delta="t.delta ?? null"
            :higher-is-better="t.higherIsBetter ?? true"
          />
        </div>
        <p v-if="!finish.pointerLock && meta.pointerLock" class="callout callout-warning result-note">
          这一轮没有锁定鼠标（Pointer Lock），视角用的是光标位移，手感和游戏不一致。
        </p>
      </div>

      <div class="result-grid">
        <div class="card card-pad">
          <SectionHeader :title="detailTitle" />
          <TrainingSprayPlot
            v-if="mode === 'spray' && sprayDetail"
            :bullets-v="sprayDetail.bulletsV"
            :bullets-h="sprayDetail.bulletsH"
            :recoil="sprayDetail.recoil"
          />
          <TrainingBars v-else :values="detailBars" :unit="detailUnit" :max="detailMax" :digits="mode === 'spray' ? 2 : 0" />
          <template v-if="mode === 'spray' && sprayDetail">
            <div class="sub-title">每一发的平均误差（°）</div>
            <TrainingBars :values="sprayDetail.perShotErr.map((v) => ({ value: v, ok: v == null ? null : v <= 2 }))" unit="°" :digits="1" />
          </template>
        </div>
        <div class="card card-pad">
          <SectionHeader :title="`${meta.scoreLabel}趋势`">
            <RangeSwitch v-model="range" />
          </SectionHeader>
          <TrainingTrend :values="trendValues" :label="meta.scoreLabel" :unit="meta.scoreUnit" :higher-is-better="meta.higherIsBetter" />
        </div>
      </div>
    </template>

    <TrainingRunner
      v-if="stage === 'run' && meta && runningMode"
      :key="runKey"
      :meta="meta"
      :mode="runningMode"
      :sensitivity="sens.current.value?.value ?? 1"
      @done="onDone"
      @cancel="stage = 'setup'"
    />
  </div>
</template>

<script setup lang="ts">
import type { TrainingMode, TrainingModeMeta, TrainingRange, TrainingResultInput, TrainingSessionDto } from '@cs-analyst/shared-types';
import { TRAINING_MODES } from '~/training/meta';
import { tauriInvoke } from '~/composables/useTauri';
import { createMode, DEFAULT_OPTIONS, optionsSnapshot, type TrainingOptions } from '~/training/modes';
import { STRAFE_WEAPONS } from '~/training/modes/strafe';
import { SPRAY_PATTERNS } from '~/training/modes/spray';
import { degPerCount, type TrainingModeImpl } from '~/training/core';
import type { EngineFinish } from '~/training/engine';

type Stage = 'list' | 'setup' | 'run' | 'result';

const RangeSwitch = defineComponent({
  props: { modelValue: { type: String, required: true } },
  emits: ['update:modelValue'],
  setup(props, { emit }) {
    const items: [TrainingRange, string][] = [['7d', '7 天'], ['30d', '30 天'], ['90d', '90 天'], ['all', '全部']];
    return () =>
      h('div', { class: 'range-switch' }, items.map(([v, l]) =>
        h('button', {
          type: 'button',
          class: ['btn', 'btn-sm', props.modelValue === v ? 'range-on' : ''],
          onClick: () => emit('update:modelValue', v),
        }, l)));
  },
});

const MODE_ICONS: Record<TrainingMode, string> = {
  reaction: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M13 2 4 14h7l-1 8 9-12h-7z"/></svg>',
  flick: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="18" cy="6" r="3"/><path d="M4 20 15 9"/></svg>',
  tracking: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M2 12h4M18 12h4"/></svg>',
  micro: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="1.5"/></svg>',
  strafe: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M3 12h18M7 8l-4 4 4 4M17 8l4 4-4 4"/></svg>',
  spray: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="20" r="1"/><circle cx="12" cy="15" r="1"/><circle cx="13" cy="10" r="1"/><circle cx="11" cy="5" r="1"/></svg>',
};

const route = useRoute();
const router = useRouter();
const sens = useTrainingSensitivity();

const stage = ref<Stage>('list');
const mode = ref<TrainingMode>('reaction');
const meta = computed<TrainingModeMeta | undefined>(() => TRAINING_MODES.find((m) => m.mode === mode.value));
const opts = reactive<TrainingOptions>({ ...DEFAULT_OPTIONS.reaction });
const runKey = ref(0);
const runningMode = shallowRef<TrainingModeImpl | null>(null);
const startedAt = ref('');
const finish = shallowRef<EngineFinish | null>(null);
const saveState = ref<'idle' | 'saving' | 'saved' | 'error' | 'skipped'>('idle');
const saveError = ref<string | null>(null);
const range = ref<TrainingRange>('30d');
const history = ref<TrainingSessionDto[]>([]);
const summary = ref<Partial<Record<TrainingMode, { last: number | null; best: number | null; count: number }>>>({});

// ── 灵敏度 ──
const sensInput = ref<number | null>(sens.current.value?.value ?? null);
watch(() => sens.current.value?.value, (v) => { sensInput.value = v ?? null; });
function applyManual() {
  const v = Number(sensInput.value);
  if (Number.isFinite(v) && v > 0 && v <= 20) sens.set({ value: v, source: 'manual' });
}
const sourceLabel = computed(() => ({ cs2: '自动读取', file: '配置文件', manual: '手动输入' })[sens.current.value?.source ?? 'manual']);
const countsPer360 = computed(() => (sens.current.value ? Math.round(360 / degPerCount(sens.current.value.value)) : '—'));

// ── 选项 ──
const OPTS_KEY = 'training.options';
function loadOpts(m: TrainingMode) {
  Object.assign(opts, DEFAULT_OPTIONS[m]);
  try {
    const saved = JSON.parse(localStorage.getItem(`${OPTS_KEY}.${m}`) ?? 'null');
    if (saved) Object.assign(opts, saved);
  } catch { /* ignore */ }
  if (!SPRAY_PATTERNS[opts.sprayWeapon]) opts.sprayWeapon = 'ak47';
}
function saveOpts() {
  try { localStorage.setItem(`${OPTS_KEY}.${mode.value}`, JSON.stringify(opts)); } catch { /* ignore */ }
}

const hasCount = computed(() => ['reaction', 'flick', 'micro', 'strafe'].includes(mode.value));
const countLabel = computed(() => ({ reaction: '次数', flick: '目标数', micro: '目标数', strafe: '有效开枪数' } as Record<string, string>)[mode.value] ?? '次数');
const countChoices = computed(() => ({ reaction: [5, 10, 15], flick: [20, 30, 50], micro: [30, 40, 60], strafe: [10, 20, 30] } as Record<string, number[]>)[mode.value] ?? [10]);

const setupNotes = computed(() => {
  switch (mode.value) {
    case 'reaction':
      return ['屏幕变成强调色后立即点击左键。', '变色前点击算犯规，这一次重来。', '成绩取中位数，偶尔一次走神不会拉低太多。'];
    case 'flick':
      return ['目标出现在视线 10–38° 之外，超出时限算漏掉。', '成绩是命中率（命中目标数 / 目标数），同时记录单个目标用时。'];
    case 'micro':
      return ['小目标出现在准星附近 1.2–4.5° 内。', '成绩是精度（命中 / 开枪次数），记录用时和偏离距离。'];
    case 'tracking':
      return ['目标会变向横移，像对手在左右晃。不需要开枪，准星留在目标上就算。', '开始前 1 秒静止准备，不计入成绩。'];
    case 'strafe':
      return [
        '按住 A 或 D 跑起来，再按反方向键（或松开）急停，速度进入阈值（最大速度的 34%）时点左键。',
        '速度用 CS2 的加速和摩擦参数模拟：sv_accelerate 5.5、sv_friction 5.2、sv_stopspeed 80，64 tick。',
        '开枪前 0.7 秒内没跑到 80% 最大速度的开枪不计。',
      ];
    case 'spray':
      return [
        '准星对准目标，按住左键连发，把鼠标往弹道的反方向拉，让子弹留在第一发的位置。',
        '弹道是从已解析的 demo 里统计的平均后坐力（每发 recoil = −2 × Δpunch），样本不够的尾部截掉了，所以连发比弹匣短。',
        '误差算法和枪法页的压枪分析一致：子弹 = 视角 + 后坐力，控枪 = 抵消掉的后坐力比例。',
      ];
  }
  return [];
});

// ── 流程 ──
function openSetup(m: TrainingMode) {
  mode.value = m;
  loadOpts(m);
  stage.value = 'setup';
  router.replace({ query: { mode: m } });
  loadHistory();
}

function backToList() {
  stage.value = 'list';
  router.replace({ query: {} });
  loadSummary();
}

function run() {
  saveOpts();
  runningMode.value = createMode(mode.value, { ...opts });
  startedAt.value = new Date().toISOString();
  finish.value = null;
  saveState.value = 'idle';
  runKey.value++;
  stage.value = 'run';
}

async function onDone(f: EngineFinish) {
  finish.value = f;
  stage.value = 'result';
  const s = sens.current.value?.value ?? 1;
  const input: TrainingResultInput = {
    mode: mode.value,
    variant: f.result.variant ?? null,
    startedAt: startedAt.value,
    durationMs: f.durationMs,
    score: f.result.score,
    metrics: f.result.metrics,
    settings: {
      sensitivity: s,
      degPerCount: degPerCount(s),
      hfovDeg: Math.round(f.hfovDeg * 10) / 10,
      pointerLock: f.pointerLock,
      avgFps: f.avgFps == null ? null : Math.round(f.avgFps),
      ...optionsSnapshot(mode.value, opts),
    },
    trials: f.result.trials,
  };
  if (f.result.score == null) {
    saveState.value = 'skipped';
    await loadHistory();
    return;
  }
  saveState.value = 'saving';
  try {
    await tauriInvoke<string>('save_training_result', { result: input });
    saveState.value = 'saved';
  } catch (e) {
    saveState.value = 'error';
    saveError.value = String(e);
  }
  await loadHistory();
}

async function loadHistory() {
  try {
    const variant = mode.value === 'spray' ? opts.sprayWeapon : null;
    history.value = await tauriInvoke<TrainingSessionDto[]>('list_training_results', { mode: mode.value, range: range.value, variant });
  } catch {
    history.value = [];
  }
}

async function loadSummary() {
  const out: typeof summary.value = {};
  await Promise.all(
    TRAINING_MODES.map(async (m) => {
      try {
        const rows = await tauriInvoke<TrainingSessionDto[]>('list_training_results', { mode: m.mode, range: 'all' });
        const scores = rows.map((r) => r.score).filter((v): v is number => v != null);
        if (!rows.length) return;
        out[m.mode] = {
          last: rows[rows.length - 1]!.score,
          best: scores.length ? (m.higherIsBetter ? Math.max(...scores) : Math.min(...scores)) : null,
          count: rows.length,
        };
      } catch { /* ignore */ }
    }),
  );
  summary.value = out;
}

watch(range, loadHistory);
watch(() => opts.sprayWeapon, () => { if (stage.value === 'setup' && mode.value === 'spray') loadHistory(); });

const trendValues = computed(() => history.value.map((r) => ({ at: r.startedAt, score: r.score })));

function fmtScore(v: number | null, m: TrainingModeMeta) {
  if (v == null) return '—';
  return `${m.scoreUnit === 'ms' ? Math.round(v) : v.toFixed(1)}${m.scoreUnit === '%' ? '%' : ` ${m.scoreUnit}`}`;
}

// ── 结果页 ──
interface Tile { label: string; value: number | string | null; unit?: string; hint?: string; tone?: 'neutral' | 'accent'; delta?: number | null; higherIsBetter?: boolean }

/** 和上一次（不含本次）的主成绩比较 */
const prevScore = computed(() => {
  const rows = history.value.filter((r) => r.score != null);
  if (saveState.value === 'saved') rows.pop();
  return rows.length ? rows[rows.length - 1]!.score : null;
});

const tiles = computed<Tile[]>(() => {
  const f = finish.value;
  const m = meta.value;
  if (!f || !m) return [];
  const x = f.result.metrics;
  const delta = f.result.score != null && prevScore.value != null ? Math.round((f.result.score - prevScore.value) * 10) / 10 : null;
  const main: Tile = { label: m.scoreLabel, value: f.result.score, unit: m.scoreUnit, tone: 'accent', delta, higherIsBetter: m.higherIsBetter, hint: delta != null ? '对比上次' : undefined };
  switch (mode.value) {
    case 'reaction':
      return [main, { label: '平均', value: x.mean, unit: 'ms' }, { label: '最好', value: x.best, unit: 'ms' }, { label: '犯规', value: x.fouls, unit: '次' }];
    case 'flick':
      return [main, { label: '平均用时', value: x.avgTime, unit: 'ms', hint: '只算命中的目标' }, { label: '精度', value: x.accuracy, unit: '%', hint: `${x.hits} / ${x.shots} 枪` }, { label: '漏掉', value: Number(x.targets) - Number(x.hits), unit: '个' }];
    case 'micro':
      return [main, { label: '平均用时', value: x.avgTime, unit: 'ms' }, { label: '命中率', value: x.hitRate, unit: '%', hint: `${x.hits} / ${x.targets} 个目标` }, { label: '未命中偏离', value: x.missDist, unit: '°', hint: '离目标边缘' }];
    case 'tracking':
      return [main, { label: '平均偏离', value: x.avgErrDeg, unit: '°', hint: '准星到目标中心' }];
    case 'strafe':
      return [main, { label: '平均时机', value: x.avgTiming, unit: 'ms', hint: '进入阈值到开枪' }, { label: '开枪时平均速度', value: x.avgSpeed, unit: 'u/s', hint: `阈值 ${x.threshold}` }, { label: '有效开枪', value: x.shots, unit: '枪' }];
    case 'spray':
      return [main, { label: '平均误差', value: x.meanErr, unit: '°', hint: '第 2 发起' }, { label: '开始偏出', value: x.breakShot == null ? '无' : `第 ${x.breakShot} 发`, hint: '误差 > 2°（中位数）' }, { label: '全程稳住', value: `${x.unbroken} / ${x.sprays}`, hint: '误差始终 ≤ 2° 的连发' }];
  }
  return [main];
});

const resultDesc = computed(() => {
  const f = finish.value;
  if (!f) return '';
  const parts = [`用时 ${(f.durationMs / 1000).toFixed(1)} 秒`];
  if (f.avgFps) parts.push(`平均 ${Math.round(f.avgFps)} FPS`);
  if (mode.value === 'spray') parts.push(SPRAY_PATTERNS[opts.sprayWeapon]?.label ?? '');
  return parts.join(' · ');
});

const detailTitle = computed(() => ({
  reaction: '每次反应时间（ms）',
  flick: '每个目标用时（ms，红色为漏掉）',
  micro: '每个目标用时（ms，红色为漏掉）',
  tracking: '每秒在目标上的比例（%）',
  strafe: '每一枪的时机（ms，红色为移动中开枪）',
  spray: '弹着点（所有连发叠加）',
} as Record<TrainingMode, string>)[mode.value]);

const detailBars = computed(() => (finish.value?.result.trials ?? []).map((t) => ({ value: t.value ?? null, ok: t.ok ?? null })));
const detailUnit = computed(() => (mode.value === 'tracking' ? '%' : 'ms'));
const detailMax = computed(() => (mode.value === 'tracking' ? 100 : null));
const sprayDetail = computed(() => (mode.value === 'spray' ? (finish.value?.result.detail as { bulletsV: number[]; bulletsH: number[]; recoil: [number, number][]; perShotErr: (number | null)[] } | undefined) : undefined));

onMounted(() => {
  sens.ensure();
  const q = route.query.mode as TrainingMode | undefined;
  if (q && TRAINING_MODES.some((m) => m.mode === q)) openSetup(q);
  else loadSummary();
});
</script>

<style scoped>
.training-page { max-width: 1080px; display: flex; flex-direction: column; gap: var(--space-4); }

.sens-card { display: flex; flex-direction: column; gap: var(--space-2); }
.sens-main { display: flex; justify-content: space-between; align-items: center; gap: var(--space-4); flex-wrap: wrap; }
.sens-title { font-size: var(--text-md); font-weight: 600; color: var(--text); }
.sens-desc { font-size: var(--text-xs); color: var(--text-3); margin-top: 2px; }
.sens-value { display: flex; gap: var(--space-2); align-items: center; }
.sens-input { width: 90px; font-size: var(--text-lg); font-weight: 600; text-align: right; }
.sens-meta { display: flex; gap: var(--space-3); align-items: center; font-size: var(--text-xs); min-width: 0; }
.sens-meta .truncate { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 520px; }
.sens-candidates { display: flex; gap: var(--space-2); align-items: center; flex-wrap: wrap; }
.sens-error code { font-family: var(--font-mono); font-size: var(--text-xs); }

.mode-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: var(--space-3); margin-top: calc(-1 * var(--space-1)); }
.mode-card {
  display: flex; flex-direction: column; gap: var(--space-2);
  padding: var(--space-4) var(--space-5);
  text-align: left;
  background: var(--surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  color: var(--text);
  cursor: pointer;
  transition: border-color var(--duration), background var(--duration);
}
.mode-card:hover { border-color: var(--border-strong); background: var(--surface-raised); }
.mode-head { display: flex; justify-content: space-between; align-items: center; }
.mode-name { font-size: var(--text-lg); font-weight: 600; }
.mode-icon { color: var(--text-3); display: flex; }
.mode-desc { font-size: var(--text-sm); color: var(--text-2); line-height: 1.5; min-height: 38px; }
.mode-stats { display: flex; gap: var(--space-4); font-size: var(--text-xs); color: var(--text-3); padding-top: var(--space-2); border-top: 1px solid var(--border-subtle); }
.mode-stats b { color: var(--text); font-weight: 600; }

.crumbs { margin-bottom: calc(-1 * var(--space-2)); }
.setup-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: var(--space-3); }
.setup-notes { margin: var(--space-4) 0 0; padding-left: 18px; font-size: var(--text-sm); color: var(--text-2); line-height: 1.7; }
.setup-actions { display: flex; justify-content: flex-end; align-items: center; gap: var(--space-4); margin-top: var(--space-4); }
.setup-card .callout { margin-top: var(--space-3); }

.tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: var(--space-3); }
.result-note { margin-top: var(--space-3); }
.result-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(420px, 1fr)); gap: var(--space-3); }
.sub-title { margin: var(--space-4) 0 var(--space-2); font-size: var(--text-xs); color: var(--text-3); }

:deep(.range-switch) { display: flex; gap: 4px; }
:deep(.range-switch .range-on) { border-color: var(--accent-line); color: var(--text); background: var(--surface-active); }
</style>
