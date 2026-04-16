<template>
  <div class="section-panel">
    <div class="section-header">
      <span v-if="icon" class="section-icon">{{ icon }}</span>
      <span class="section-title">{{ title }}</span>
      <div class="section-actions">
        <slot name="actions" />
      </div>
    </div>
    <div class="section-body">
      <slot />
    </div>
  </div>
</template>

<script setup>
defineProps({
  icon: { type: String, default: '' },
  title: { type: String, required: true },
})
</script>

<style lang="less" scoped>
.section-panel {
  background: rgba(0, 0, 0, 0.18);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 16px;
  overflow: hidden;
}

.section-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: rgba(255, 255, 255, 0.06);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.section-icon {
  font-size: 15px;
  flex-shrink: 0;
}

.section-title {
  font-size: 13px;
  font-weight: 600;
  flex: 1;
  color: rgba(255, 255, 255, 0.95);
  letter-spacing: 0.3px;
}

.section-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.section-body {
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
