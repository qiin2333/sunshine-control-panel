<template>
  <Transition name="notice">
    <div
      v-if="notice"
      class="settings-notice fade-in"
      :class="notice.type"
    >
      {{ notice.message }}
    </div>
  </Transition>
</template>

<script setup>
defineProps({
  notice: {
    type: Object,
    default: null,
  },
})
</script>

<style lang="less" scoped>
.settings-notice {
  display: flex;
  align-items: center;
  min-height: 42px;
  margin: 8px 0 20px;
  padding: 10px 14px;
  border-radius: 10px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.72);
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.82);
  font-size: 13px;

  &.success {
    border-color: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.35);
    color: var(--fd-status-success, #00ff88);
  }

  &.warning,
  &.info {
    border-color: rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.3);
  }

  &.error {
    border-color: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.35);
    color: var(--fd-status-danger, #ff6b35);
  }
}

.notice-enter-active,
.notice-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.notice-enter-from,
.notice-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
</style>
