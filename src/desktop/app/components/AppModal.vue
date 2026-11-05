<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="app-modal-overlay"
      @mousedown.self="close"
    >
      <div
        class="app-modal"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="title ? 'app-modal-title' : undefined"
        @mousedown.stop
      >
        <header v-if="title || $slots.header" class="app-modal-header">
          <slot name="header">
            <h2 id="app-modal-title" class="app-modal-title">{{ title }}</h2>
          </slot>
          <button type="button" class="btn-ghost btn-sm btn-icon" aria-label="关闭" @click="close">✕</button>
        </header>
        <div class="app-modal-body">
          <slot />
        </div>
        <footer v-if="$slots.footer" class="app-modal-footer">
          <slot name="footer" :close="close" />
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
defineProps<{ open: boolean; title?: string }>();
const emit = defineEmits<{ 'update:open': [value: boolean] }>();

function close() {
  emit('update:open', false);
}
</script>

<style scoped>
.app-modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-4);
  background: rgb(0 0 0 / 0.6);
}

.app-modal {
  width: min(100%, 480px);
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
}

.app-modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}
.app-modal-title { margin: 0; font-size: var(--text-lg); font-weight: 600; color: var(--text); }

.app-modal-body { flex: 1; overflow-y: auto; padding: var(--space-4); }

.app-modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--border-subtle);
}
</style>
