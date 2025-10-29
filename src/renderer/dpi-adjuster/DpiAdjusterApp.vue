<template>
  <div id="dpi-adjuster">
    <div class="header">
      <h2>调整 DPI</h2>
    </div>

    <div class="content">
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

const dpiValue = ref(100);
const applying = ref(false);
const message = ref('');
const messageType = ref('');
const loading = ref(true);

const presets = [100, 125, 150, 175, 200, 225, 250, 300];

// 获取当前系统 DPI
const loadCurrentDpi = async () => {
  try {
    const currentDpi = await invoke('get_current_dpi');
    dpiValue.value = currentDpi;
    console.log('当前 DPI:', currentDpi);
  } catch (error) {
    console.error('获取当前 DPI 失败:', error);
    // 如果获取失败，保持默认值 100
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
  } catch (error) {
    message.value = `❌ 设置失败: ${error}`;
    messageType.value = 'error';
    console.error('设置 DPI 失败:', error);
  } finally {
    applying.value = false;
    
    setTimeout(() => {
      message.value = '';
    }, 3000);
  }
};

// 组件挂载时加载当前 DPI
onMounted(() => {
  loadCurrentDpi();
});
</script>

<style scoped>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

#dpi-adjuster {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  overflow: hidden;
}

.header {
  padding: 12px 20px;
  text-align: center;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
}

.header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 15px 25px;
  gap: 12px;
  overflow: hidden;
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
  appearance: none;
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
  color: #667eea;
  border-color: white;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.actions {
  text-align: center;
}

.apply-btn {
  padding: 10px 32px;
  background: white;
  color: #667eea;
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

.message.error {
  background: rgba(244, 67, 54, 0.9);
}
</style>

