<template>
  <div class="tool-window-overlay" @click.self="closeWindow">
    <div class="tool-panel" @click.stop>
      <component :is="currentTool" v-if="currentTool" @close="closeWindow" />

      <div v-else class="loading-state">
        <div class="spinner"></div>
        <p>加载中...</p>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, defineAsyncComponent } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'

const currentTool = ref(null)

const closeWindow = async () => {
  try {
    const window = getCurrentWindow()
    await window.close()
  } catch (error) {
    console.error('关闭窗口失败:', error)
  }
}

const getToolType = () => {
  const params = new URLSearchParams(window.location.search)
  return params.get('tool') || 'dpi'
}

// ESC 键关闭
const handleKeyDown = (e) => {
  if (e.key === 'Escape') {
    closeWindow()
  }
}

onMounted(async () => {
  const toolType = getToolType()
  console.log('加载工具:', toolType)

  // 添加键盘事件监听
  window.addEventListener('keydown', handleKeyDown)

  try {
    switch (toolType) {
      case 'dpi':
        currentTool.value = defineAsyncComponent(() => import('./tools/DpiAdjusterTool.vue'))
        break
      case 'bitrate':
        currentTool.value = defineAsyncComponent(() => import('./tools/BitrateTool.vue'))
        break
      default:
        console.error('未知的工具类型:', toolType)
    }
  } catch (error) {
    console.error('加载工具失败:', error)
  }
})

// 清理事件监听
onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown)
})
</script>

<style scoped>
.tool-window-overlay {
  width: 100vw;
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.75);
  backdrop-filter: blur(10px);
  overflow: hidden;
  cursor: pointer;
  animation: overlayIn 0.2s ease;
}

@keyframes overlayIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.tool-panel {
  max-width: 90vw;
  max-height: 90vh;
  background: linear-gradient(135deg, #4a9eff 0%, #7ab8ff 100%);
  border-radius: 20px;
  box-shadow: 0 20px 60px rgba(74, 158, 255, 0.5);
  overflow: hidden;
  cursor: default;
  animation: panelIn 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}

@keyframes panelIn {
  0% {
    opacity: 0;
    transform: scale(0.8) translateY(30px);
  }
  100% {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.loading-state {
  padding: 60px;
  text-align: center;
  color: white;
  width: 420px;
}

.spinner {
  width: 48px;
  height: 48px;
  border: 4px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin: 0 auto 20px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-state p {
  margin-top: 16px;
  font-size: 16px;
  opacity: 0.9;
}
</style>
