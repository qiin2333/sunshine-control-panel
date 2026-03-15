<template>
  <div class="pill-group">
    <button 
      v-for="opt in options" 
      :key="opt.value"
      class="pill-btn"
      :class="{ active: modelValue === opt.value }"
      @click="$emit('update:modelValue', opt.value)"
    >{{ opt.label }}</button>
  </div>
</template>

<script setup>
defineProps({
  modelValue: { type: [Number, String], required: true },
  options: { type: Array, required: true },
})
defineEmits(['update:modelValue'])
</script>

<style lang="less" scoped>
.pill-group {
  display: flex;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-radius: 8px;
  overflow: hidden;
  flex-shrink: 0;
}

.pill-btn {
  padding: 7px 14px;
  border: none;
  background: transparent;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;

  & + .pill-btn {
    border-left: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
  }

  &:hover {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.8);
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.04);
  }

  &.active {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.12);
    color: var(--fd-accent, #00fff5);
    font-weight: 500;
  }
}
</style>
