<template>
  <div class="toggle-switch" :class="{ on: modelValue }" @click="$emit('update:modelValue', !modelValue)">
    <div class="toggle-thumb"></div>
  </div>
</template>

<script setup>
defineProps({
  modelValue: { type: Boolean, default: false },
})
defineEmits(['update:modelValue'])
</script>

<style lang="less" scoped>
.toggle-switch {
  width: 38px;
  height: 20px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.18);
  cursor: pointer;
  transition: background 0.25s;
  position: relative;
  flex-shrink: 0;

  &.on {
    background: #4ade80;
  }

  .toggle-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
    transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  }

  &.on .toggle-thumb {
    transform: translateX(18px);
  }
}
</style>
