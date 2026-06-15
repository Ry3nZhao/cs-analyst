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
          <span class="logo-title">CS Demo</span>
          <span class="logo-sub">Analyst</span>
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
        </div>
        <div class="status-row">
          <span class="status-dot" :class="status.orchestratorRunning ? 'on' : 'off'" />
          <span class="status-label">下载器</span>
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
          <span v-if="status.lastDiscoverMinutesAgo != null" class="discover-ago">
            {{ status.lastDiscoverMinutesAgo }}m 前发现
          </span>
        </div>
        <div class="topbar-right">
          <button class="btn-accent" @click="discoverNow">
            <IconSearch />
            立即发现
          </button>
          <button
            class="btn-ghost"
            :class="{ 'btn-warn': status.queuePaused }"
            @click="toggleQueue"
          >
            <IconPause v-if="!status.queuePaused" />
            <IconPlay v-else />
            {{ status.queuePaused ? '恢复队列' : '暂停队列' }}
          </button>
        </div>
      </header>

      <!-- Page slot -->
      <main class="page-content">
        <slot />
      </main>
    </div>
  </div>
</template>

<script setup lang="ts">
import { listenJobsUpdated } from '~/composables/useTauri';

const route = useRoute();
const { status, refreshStatus, discoverNow, toggleQueue } = useAppStatus();

const titles: Record<string, string> = {
  '/downloads': '下载队列',
  '/accounts': '账户管理',
  '/settings': '系统设置',
};
const title = computed(() => titles[route.path] ?? 'CS Demo Analyst');

// Inline SVG icon components
const IconSearch = defineComponent({
  render: () =>
    h('svg', { width: 13, height: 13, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2.2 }, [
      h('circle', { cx: 11, cy: 11, r: 8 }),
      h('path', { d: 'm21 21-4.35-4.35' }),
    ]),
});
const IconPause = defineComponent({
  render: () =>
    h('svg', { width: 13, height: 13, viewBox: '0 0 24 24', fill: 'currentColor' }, [
      h('rect', { x: 6, y: 4, width: 4, height: 16, rx: 1 }),
      h('rect', { x: 14, y: 4, width: 4, height: 16, rx: 1 }),
    ]),
});
const IconPlay = defineComponent({
  render: () =>
    h('svg', { width: 13, height: 13, viewBox: '0 0 24 24', fill: 'currentColor' }, [
      h('polygon', { points: '5 3 19 12 5 21 5 3' }),
    ]),
});

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
  {
    to: '/downloads',
    label: '下载队列',
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
  onUnmounted(u);
});
</script>

<style scoped>
.app-shell {
  display: flex;
  height: 100vh;
  background: var(--c-bg);
  color: var(--c-text);
  overflow: hidden;
}

/* ── Sidebar ── */
.sidebar {
  width: 210px;
  flex-shrink: 0;
  background: var(--c-sidebar);
  border-right: 1px solid var(--c-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 20px 16px 16px;
  border-bottom: 1px solid var(--c-border-sub);
}

.logo-icon {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: linear-gradient(135deg, var(--c-accent) 0%, #4f46e5 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  flex-shrink: 0;
}

.logo-text {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
}
.logo-title {
  font-size: 13px;
  font-weight: 600;
  color: #fff;
}
.logo-sub {
  font-size: 10px;
  color: var(--c-text-3);
}

.nav {
  flex: 1;
  padding: 10px 10px 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 9px 12px;
  border-radius: 8px;
  font-size: 13px;
  color: var(--c-text-2);
  text-decoration: none;
  transition: background 0.12s, color 0.12s;
  cursor: pointer;
}
.nav-item:hover {
  background: rgba(255,255,255,0.04);
  color: var(--c-text);
}
.nav-item.active {
  background: var(--c-accent-bg);
  color: var(--c-accent-lt);
  font-weight: 500;
}
.nav-icon {
  flex-shrink: 0;
  opacity: 0.85;
}

.sidebar-footer {
  padding: 14px 16px;
  border-top: 1px solid var(--c-border-sub);
}
.status-row {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 3px 0;
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}
.status-dot.on  { background: var(--c-success); box-shadow: 0 0 6px var(--c-success); }
.status-dot.off { background: var(--c-text-faint); }
.status-label { font-size: 11px; color: var(--c-text-3); }
.data-root {
  margin-top: 8px;
  font-size: 10px;
  color: var(--c-text-faint);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── Content ── */
.content-wrap {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.topbar {
  height: 50px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
  border-bottom: 1px solid var(--c-border-sub);
  background: var(--c-bg);
  flex-shrink: 0;
}
.topbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}
.page-title {
  font-size: 13px;
  font-weight: 600;
  color: #fff;
}
.discover-ago {
  font-size: 11px;
  color: var(--c-text-faint);
}
.topbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-accent, .btn-ghost {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 6px 12px;
  border-radius: 7px;
  font-size: 12px;
  font-weight: 500;
  border: 1px solid transparent;
  cursor: pointer;
  transition: opacity 0.1s;
}
.btn-accent:hover, .btn-ghost:hover { opacity: 0.82; }

.btn-accent {
  background: var(--c-accent-bg);
  color: var(--c-accent-lt);
  border-color: var(--c-accent-bdr);
}
.btn-ghost {
  background: rgba(255,255,255,0.05);
  color: var(--c-text-2);
  border-color: rgba(255,255,255,0.08);
}
.btn-ghost.btn-warn {
  background: rgba(245,158,11,0.1);
  color: #f59e0b;
  border-color: rgba(245,158,11,0.2);
}

.page-content {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
}
</style>
