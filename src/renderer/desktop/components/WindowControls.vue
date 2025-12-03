<template>
  <div class="window-controls">
    <button 
      class="control-btn minimize-btn" 
      @click="handleMinimize" 
      title="最小化"
      :disabled="disabled"
    >
      <svg viewBox="0 0 10 1">
        <rect width="10" height="1" fill="currentColor"/>
      </svg>
    </button>
    <button 
      class="control-btn maximize-btn" 
      @click="handleToggleMaximize" 
      :title="isMaximized ? '还原' : '最大化'"
      :disabled="disabled"
    >
      <svg v-if="!isMaximized" viewBox="0 0 10 10">
        <rect x="0" y="0" width="10" height="10" stroke="currentColor" stroke-width="1" fill="none"/>
      </svg>
      <svg v-else viewBox="0 0 10 10">
        <rect x="2" y="0" width="8" height="8" stroke="currentColor" stroke-width="1" fill="none"/>
        <rect x="0" y="2" width="8" height="8" stroke="currentColor" stroke-width="1" fill="var(--bg-primary, #0f0f23)"/>
      </svg>
    </button>
    <button 
      class="control-btn close-btn" 
      @click="handleClose" 
      title="关闭"
      :disabled="disabled"
    >
      <svg viewBox="0 0 10 10">
        <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1.2"/>
        <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1.2"/>
      </svg>
    </button>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { useWindowControls } from '../composables/useWindowControls'

const props = defineProps({
  disabled: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['minimize', 'maximize', 'close'])

const { isMaximized, minimize, maximize, close, toggleMaximize } = useWindowControls()

async function handleMinimize() {
  if (props.disabled) return
  await minimize()
  emit('minimize')
}

async function handleToggleMaximize() {
  if (props.disabled) return
  await toggleMaximize()
  emit('maximize')
}

async function handleClose() {
  if (props.disabled) return
  await close()
  emit('close')
}
</script>

<style lang="less" scoped>
.window-controls {
  display: flex;
  -webkit-app-region: no-drag;
}

.control-btn {
  width: 46px;
  height: 32px;
  border: none;
  background: transparent;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;

  &:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 1);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  &.close-btn:hover:not(:disabled) {
    background: #e81123;
    color: white;
  }

  svg {
    width: 10px;
    height: 10px;
  }
}
</style>

