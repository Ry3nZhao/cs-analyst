<template>
  <div class="app-shell">
    <!-- ── Sidebar ── -->
    <aside class="sidebar">
      <!-- Logo -->
      <div class="logo">
        <div class="logo-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <circle cx="12" cy="12" r="3"/>
            <path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>
            <path d="M4.93 4.93l2.12 2.12M16.95 16.95l2.12 2.12M4.93 19.07l2.12-2.12M16.95 7.05l2.12-2.12"/>
          </svg>
        </div>
        <div class="logo-text">
          <span class="logo-title">CS Analyst</span>
          <span class="logo-sub">Demo 分析</span>
        </div>
      </div>

      <!-- Nav -->
      <nav class="nav">
        <NuxtLink
          v-for="item in nav"
          :key="item.to"
          :to="item.to"
          class="nav-item"
          :class="{ active: route.path === item.to || route.path.startsWith(item.to + '/') }"
        >
          <component :is="item.icon" class="nav-icon" />
          <span>{{ item.label }}</span>
        </NuxtLink>
      </nav>

      <!-- Status -->
      <div class="sidebar-footer">
        <div class="status-row">
          <span class="status-dot" :class="status.sidecarRunning ? 'on' : 'off'" />
          <span class="status-label">Sidecar</span>
          <span class="status-value">{{ status.sidecarRunning ? '运行中' : '未运行' }}</span>
        </div>
        <div class="status-row">
          <span class="status-dot" :class="status.orchestratorRunning ? 'on' : 'off'" />
          <span class="status-label">下载器</span>
          <span class="status-value">{{ status.orchestratorRunning ? '运行中' : '未运行' }}</span>
        </div>
        <div class="data-root" :title="status.dataRoot">{{ status.dataRoot }}</div>
      </div>
    </aside>

    <!-- ── Content ── -->
    <div class="content-wrap">
      <!-- Header -->
      <header class="topbar">
        <div class="topbar-left">
          <span class="page-title">{{ title }}</span>
          <span v-if="status.lastDiscoverMinutesAgo != null" class="discover-ago num">
            {{ status.lastDiscoverMinutesAgo }}m 前发现
          </span>
          <div
            v-if="status.backgroundTask"
            class="task-pill"
            :class="`task-${status.backgroundTask.status}`"
          >
            <span class="task-label">{{ status.backgroundTask.label }}</span>
            <span class="task-current">{{ status.backgroundTask.current }}</span>
            <span class="task-progress progress">
              <span
                class="task-progress-fill progress-fill"
                :style="{ width: `${Math.round(status.backgroundTask.progress * 100)}%` }"
              />
            </span>
            <span class="task-percent num">{{ Math.round(status.backgroundTask.progress * 100) }}%</span>
          </div>
        </div>
        <div class="topbar-right" />
      </header>

      <!-- Page slot -->
      <main class="page-content">
        <slot />
      </main>
    </div>

    <USlideover v-model:open="logOpen" side="right">
      <template #content="{ close }">
        <div class="log-drawer">
          <div class="log-header">
            <span class="log-title">后台任务日志</span>
            <button class="btn-ghost btn-sm btn-icon" type="button" aria-label="关闭" @click="close">✕</button>
          </div>
          <div class="log-list">
            <div v-for="(line, i) in logs" :key="`${i}-${line}`" class="log-line">{{ line }}</div>
            <div v-if="logs.length === 0" class="log-empty">暂无日志</div>
          </div>
        </div>
      </template>
    </USlideover>
  </div>
</template>

<script setup lang="ts">
import { listenJobsUpdated } from '~/composables/useTauri';

const route = useRoute();
const { status, logs, logOpen, refreshStatus, listenBackgroundWork } = useAppStatus();

const titles: Record<string, string> = {
  '/': '个人主页',
  '/matches': '对局',
  '/players': '选手',
  '/aim': '枪法与压枪',
  '/training': '训练',
  '/highlights': '集锦',
  '/accounts': '账户管理',
  '/settings': '系统设置',
};
const title = computed(() => titles[route.path] ?? 'CS Analyst');

// Nav icon components
function navIcon(d: string) {
  return defineComponent({
    render: () =>
      h('svg', { width: 15, height: 15, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [
        h('path', { d }),
      ]),
  });
}

const nav = [
  { to: '/', label: '主页', icon: navIcon('M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z') },
  {
    to: '/matches',
    label: '对局',
    icon: defineComponent({
      render: () =>
        h('svg', { width: 15, height: 15, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [
          h('path', { d: 'M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4' }),
          h('polyline', { points: '7 10 12 15 17 10' }),
          h('line', { x1: 12, y1: 15, x2: 12, y2: 3 }),
        ]),
    }),
  },
  {
    to: '/players',
    label: '选手',
    icon: defineComponent({
      render: () =>
        h('svg', { width: 15, height: 15, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [
          h('circle', { cx: 12, cy: 8, r: 4 }),
          h('path', { d: 'M4 21v-1a6 6 0 0 1 6-6h4a6 6 0 0 1 6 6v1' }),
        ]),
    }),
  },
  {
    to: '/aim',
    label: '枪法',
    icon: defineComponent({
      render: () =>
        h('svg', { width: 15, height: 15, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [
          h('circle', { cx: 12, cy: 12, r: 8 }),
          h('path', { d: 'M12 2v4M12 18v4M2 12h4M18 12h4' }),
          h('circle', { cx: 12, cy: 12, r: 1 }),
        ]),
    }),
  },
  {
    to: '/training',
    label: '训练',
    icon: navIcon('M12 2v4M12 18v4M2 12h4M18 12h4M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z'),
  },
  {
    to: '/highlights',
    label: '集锦',
    icon: navIcon('M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z'),
  },
  {
    to: '/accounts',
    label: '账户管理',
    icon: defineComponent({
      render: () =>
        h('svg', { width: 15, height: 15, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [
          h('path', { d: 'M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2' }),
          h('circle', { cx: 9, cy: 7, r: 4 }),
          h('path', { d: 'M23 21v-2a4 4 0 0 0-3-3.87' }),
          h('path', { d: 'M16 3.13a4 4 0 0 1 0 7.75' }),
        ]),
    }),
  },
  {
    to: '/settings',
    label: '系统设置',
    icon: defineComponent({
      render: () =>
        h('svg', { width: 15, height: 15, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, [
          h('circle', { cx: 12, cy: 12, r: 3 }),
          h('path', { d: 'M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z' }),
        ]),
    }),
  },
];

onMounted(() => {
  refreshStatus();
  const u = listenJobsUpdated(() => refreshStatus());
  const uTask = listenBackgroundWork();
  onUnmounted(() => {
    u();
    uTask();
  });
});
</script>

<style scoped>
.app-shell {
  display: flex;
  height: 100vh;
  background: var(--bg);
  color: var(--text);
  overflow: hidden;
}

/* ── 侧边栏 ── */
.sidebar {
  width: 196px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-sunken);
  border-right: 1px solid var(--border-subtle);
  overflow: hidden;
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 52px;
  padding: 0 var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  box-sizing: border-box;
}
.logo-icon {
  width: 26px;
  height: 26px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  background: var(--gray-0);
  color: var(--text-inverse);
}
.logo-text { display: flex; flex-direction: column; line-height: 1.2; }
.logo-title { font-size: var(--text-md); font-weight: 600; color: var(--text); }
.logo-sub { font-size: var(--text-xs); color: var(--text-3); }

.nav {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-3) var(--space-2) 0;
}
.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 32px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-md);
  color: var(--text-3);
  font-size: var(--text-md);
  text-decoration: none;
  transition: background var(--duration), color var(--duration);
}
.nav-item:hover { background: var(--surface-hover); color: var(--text); }
.nav-item.active { background: var(--surface-active); color: var(--text); font-weight: 500; }
.nav-item.active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 8px;
  bottom: 8px;
  width: 2px;
  border-radius: 0 2px 2px 0;
  background: var(--accent);
}
.nav-icon { flex-shrink: 0; }

.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--border-subtle);
}
.status-row { display: flex; align-items: center; gap: var(--space-2); height: 20px; }
.status-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
.status-dot.on  { background: var(--success); }
.status-dot.off { background: var(--text-disabled); }
.status-label { font-size: var(--text-xs); color: var(--text-3); }
.status-value { margin-left: auto; font-size: var(--text-xs); color: var(--text-disabled); }
.data-root {
  margin-top: var(--space-1);
  font-size: 10px;
  color: var(--text-disabled);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── 内容区 ── */
.content-wrap { flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden; }

.topbar {
  height: 52px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg);
  box-sizing: border-box;
}
.topbar-left { display: flex; align-items: center; gap: var(--space-3); min-width: 0; }
.page-title { font-size: var(--text-lg); font-weight: 600; color: var(--text); white-space: nowrap; }
.discover-ago { font-size: var(--text-xs); color: var(--text-3); white-space: nowrap; }

.task-pill {
  height: 26px;
  min-width: 260px;
  max-width: 440px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) 84px auto;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-2);
  border-radius: var(--radius-md);
  border: 1px solid var(--accent-line);
  background: var(--surface);
}
.task-failed { border-color: var(--danger-line); background: var(--danger-soft); }
.task-failed .task-progress-fill { background: var(--danger); }
.task-completed { border-color: var(--success-line); }
.task-completed .task-progress-fill { background: var(--success); }
.task-label { font-size: var(--text-xs); font-weight: 600; color: var(--text); white-space: nowrap; }
.task-current {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-xs);
  color: var(--text-3);
}
.task-progress-fill { display: block; }
.task-percent { width: 32px; font-size: var(--text-xs); text-align: right; color: var(--text-2); }
.topbar-right { display: flex; align-items: center; gap: var(--space-2); }

.page-content { flex: 1; overflow-y: auto; padding: var(--space-5); }

/* ── 日志抽屉 ── */
.log-drawer {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: var(--space-4) var(--space-5);
  background: var(--surface);
  box-sizing: border-box;
}
.log-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
}
.log-title { font-size: var(--text-lg); font-weight: 600; color: var(--text); }
.log-list { flex: 1; min-height: 0; overflow: auto; padding-top: var(--space-2); font-family: var(--font-mono); }
.log-line {
  padding: var(--space-1) 0;
  font-size: var(--text-xs);
  line-height: 1.5;
  color: var(--text-2);
  border-bottom: 1px solid var(--border-subtle);
  white-space: pre-wrap;
  word-break: break-word;
}
.log-empty { padding: var(--space-6) 0; text-align: center; font-size: var(--text-sm); color: var(--text-disabled); }
</style>
