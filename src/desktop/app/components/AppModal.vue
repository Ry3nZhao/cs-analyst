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
          <button type="button" class="app-modal-close" aria-label="关闭" @click="close">×</button>
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
  background: rgba(0, 0, 0, 0.55);
  padding: 16px;
}

.app-modal {
  width: min(100%, 480px);
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 10px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
  overflow: hidden;
}

.app-modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--c-border);
}

.app-modal-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: #fff;
}

.app-modal-close {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--c-text-3);
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}
.app-modal-close:hover {
  background: var(--c-card);
  color: var(--c-text);
}

.app-modal-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.app-modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--c-border);
}
</style>
