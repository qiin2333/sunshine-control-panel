<template>
  <div class="tool-container" :class="{ 'embedded': embedded }">
    <div v-if="!embedded" class="tool-header">
      <h2>调整 DPI</h2>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="tool-content">
      <div class="dpi-display">
        <span class="dpi-value" v-if="!loading">{{ dpiValue }}%</span>
        <span class="dpi-value loading" v-else>加载中...</span>
      </div>

      <div class="slider-container">
        <input
          type="range"
          v-model.number="dpiValue"
          min="100"
          max="300"
          step="25"
          class="dpi-slider"
        />
      </div>

      <div class="presets">
        <button 
          v-for="preset in presets" 
          :key="preset"
          @click="dpiValue = preset"
          :class="{ active: dpiValue === preset }"
          class="preset-btn"
        >
          {{ preset }}%
        </button>
      </div>

      <div class="actions">
        <button @click="applyDpi" class="apply-btn" :disabled="applying">
          {{ applying ? '应用中...' : '应用' }}
        </button>
      </div>

      <div v-if="message" :class="['message', messageType]">
        {{ message }}
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

defineProps({
  embedded: {
    type: Boolean,
    default: false
  }
})

defineEmits(['close']);

const dpiValue = ref(100);
const applying = ref(false);
const message = ref('');
const messageType = ref('');
const loading = ref(true);

const presets = [100, 125, 150, 175, 200, 225, 250, 300];

const loadCurrentDpi = async () => {
  try {
    const currentDpi = await invoke('get_current_dpi');
    dpiValue.value = currentDpi;
    console.log('当前 DPI:', currentDpi);
  } catch (error) {
    console.error('获取当前 DPI 失败:', error);
  } finally {
    loading.value = false;
  }
};

const applyDpi = async () => {
  applying.value = true;
  message.value = '';
  
  try {
    await invoke('set_desktop_dpi', { dpi: dpiValue.value });
    message.value = `✅ DPI 已设置为 ${dpiValue.value}%`;
    messageType.value = 'success';
    
    setTimeout(async () => {
      try {
        const currentDpi = await invoke('get_current_dpi');
        if (currentDpi !== dpiValue.value) {
          message.value = `⚠️ DPI 已设置，当前显示为 ${currentDpi}%（可能需要重启应用）`;
          messageType.value = 'warning';
        }
      } catch (error) {
        console.error('重新获取 DPI 失败:', error);
      }
    }, 1000);
    
  } catch (error) {
    message.value = `❌ 设置失败: ${error}`;
    messageType.value = 'error';
  } finally {
    applying.value = false;
    
    setTimeout(() => {
      message.value = '';
    }, 5000);
  }
};

onMounted(() => {
  loadCurrentDpi();
});
</script>

<style lang="less" scoped>
.tool-container {
  width: 100%;
  color: white;
  
  &.embedded {
    width: 100%;
    
    .tool-header {
      display: none;
    }
    
    .tool-content {
      padding: 0;
    }
  }
}

.tool-header {
  padding: 16px 24px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  position: relative;
}

.tool-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  text-align: center;
}

.close-btn {
  position: absolute;
  top: 12px;
  right: 16px;
  width: 32px;
  height: 32px;
  border: none;
  background: rgba(255, 255, 255, 0.2);
  color: white;
  font-size: 28px;
  line-height: 1;
  border-radius: 50%;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: rotate(90deg);
}

.tool-content {
  padding: 20px 30px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.dpi-display {
  text-align: center;
}

.dpi-value {
  font-size: 48px;
  font-weight: 700;
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
  display: inline-block;
  animation: pulse 2s ease-in-out infinite;
}

.dpi-value.loading {
  font-size: 24px;
  opacity: 0.8;
}

@keyframes pulse {
  0%, 100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.03);
  }
}

.slider-container {
  padding: 0 5px;
}

.dpi-slider {
  width: 100%;
  height: 6px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.3);
  outline: none;
  -webkit-appearance: none;
  cursor: pointer;
}

.dpi-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: white;
  cursor: pointer;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
  transition: transform 0.2s;
}

.dpi-slider::-webkit-slider-thumb:hover {
  transform: scale(1.15);
}

.dpi-slider::-moz-range-thumb {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: white;
  cursor: pointer;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
  border: none;
  transition: transform 0.2s;
}

.dpi-slider::-moz-range-thumb:hover {
  transform: scale(1.15);
}

.presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  justify-content: center;
}

.preset-btn {
  padding: 6px 12px;
  border: 1px solid rgba(255, 255, 255, 0.3);
  background: rgba(255, 255, 255, 0.1);
  color: white;
  border-radius: 15px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  transition: all 0.2s;
}

.preset-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.5);
}

.preset-btn.active {
  background: white;
  color: #4a9eff;
  border-color: white;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.actions {
  text-align: center;
}

.apply-btn {
  padding: 10px 32px;
  background: white;
  color: #4a9eff;
  border: none;
  border-radius: 25px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 3px 8px rgba(0, 0, 0, 0.2);
}

.apply-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.apply-btn:active:not(:disabled) {
  transform: translateY(0);
}

.apply-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.message {
  text-align: center;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 12px;
  animation: slideIn 0.3s ease;
}

@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.message.success {
  background: rgba(76, 175, 80, 0.9);
}

.message.warning {
  background: rgba(255, 152, 0, 0.9);
}

.message.error {
  background: rgba(244, 67, 54, 0.9);
}

.loading-state {
  padding: 60px;
  text-align: center;
  color: white;
}

.loading-state p {
  margin-top: 16px;
  font-size: 16px;
  opacity: 0.9;
}
</style>

