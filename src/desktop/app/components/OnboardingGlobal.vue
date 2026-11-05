<template>
  <!-- 拖拽导入的遮罩 -->
  <Transition name="fade">
    <div v-if="drop.active" class="drop-overlay" aria-live="polite">
      <div class="drop-box" :class="{ invalid: drop.demoCount === 0 }">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3v12M7 10l5 5 5-5" />
          <path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />
        </svg>
        <div class="drop-title">
          <template v-if="drop.demoCount > 0">松开导入 <span class="num">{{ drop.demoCount }}</span> 个 demo</template>
          <template v-else>只能导入 .dem 文件</template>
        </div>
        <div v-if="drop.otherCount > 0 && drop.demoCount > 0" class="drop-hint">
          另有 <span class="num">{{ drop.otherCount }}</span> 个其他文件会被忽略
        </div>
      </div>
    </div>
  </Transition>

  <!-- 从向导跳到其他页面（例如绑定账号）时，提供回到向导的入口 -->
  <NuxtLink v-if="showBack" to="/welcome" class="back-pill">
    <span class="back-dot" />
    返回首次启动向导
  </NuxtLink>
</template>

<script setup lang="ts">
const drop = useDropState();
const route = useRoute();
const wizardActive = useState('onboarding-wizard-active', () => false);
const showBack = computed(() => wizardActive.value && route.path !== '/welcome');
</script>

<style scoped>
.drop-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--bg) 78%, transparent);
  backdrop-filter: blur(2px);
  pointer-events: none;
}
.drop-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  min-width: 320px;
  padding: var(--space-8) var(--space-6);
  border: 1.5px dashed var(--accent-line);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
}
.drop-box svg { color: var(--accent); }
.drop-box.invalid { border-color: var(--border-strong); }
.drop-box.invalid svg { color: var(--text-3); }
.drop-title { font-size: var(--text-lg); font-weight: 600; }
.drop-hint { font-size: var(--text-xs); color: var(--text-3); }

.back-pill {
  position: fixed;
  right: var(--space-5);
  bottom: var(--space-5);
  z-index: 90;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--control-h);
  padding: 0 var(--space-3);
  border: 1px solid var(--accent-line);
  border-radius: var(--radius-full);
  background: var(--surface-raised);
  color: var(--text);
  font-size: var(--text-sm);
  text-decoration: none;
}
.back-pill:hover { background: var(--surface-hover); }
.back-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); }

.fade-enter-active, .fade-leave-active { transition: opacity 0.12s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
