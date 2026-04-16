<template>
  <SectionPanel :title="t.rtssTool.fpsLimiter">
    <template #icon><Aim /></template>
    <template #actions>
      <ToggleSwitch :modelValue="limiterEnabled" @update:modelValue="onToggleLimiter" />
    </template>

    <div class="fps-display">
      <span class="fps-value">{{ fpsValue }}</span>
      <span class="fps-label">FPS</span>
    </div>

    <div class="slider-container">
      <input
        type="range"
        v-model.number="fpsValue"
        :min="15"
        :max="360"
        :step="1"
        class="fps-slider"
        :disabled="applying"
      />
      <div class="slider-labels">
        <span>15</span>
        <span>60</span>
        <span>120</span>
        <span>240</span>
        <span>360</span>
      </div>
    </div>

    <div class="presets">
      <button
        v-for="preset in presets"
        :key="preset"
        @click="fpsValue = preset"
        :class="{ active: fpsValue === preset }"
        class="preset-btn"
      >
        {{ preset }}
      </button>
    </div>

    <div class="actions">
      <button @click="applyLimit" class="apply-btn" :disabled="applying">
        {{ applying ? t.rtssTool.applying : t.rtssTool.applyFps }}
      </button>
      <button @click="removeLimit" class="reset-btn" :disabled="applying">
        {{ t.rtssTool.removeFps }}
      </button>
    </div>
  </SectionPanel>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { Aim } from '@element-plus/icons-vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../../../desktop/i18n/index.js'
import SectionPanel from '../../components/SectionPanel.vue'
import ToggleSwitch from '../../components/ToggleSwitch.vue'

const { t } = useI18n()
const emit = defineEmits(['message'])

const fpsValue = ref(60)
const applying = ref(false)
const limiterEnabled = ref(true)
const presets = [30, 60, 90, 120, 144, 165, 240]

async function loadState() {
  try {
    const currentFps = await invoke('rtss_get_framerate_limit', { profile: null })
    if (currentFps > 0) fpsValue.value = currentFps
    const state = await invoke('rtss_get_limiter_status')
    limiterEnabled.value = state === 1
  } catch (e) {
    console.warn('获取帧率限制状态失败:', e)
  }
}

async function applyLimit() {
  applying.value = true
  try {
    await invoke('rtss_set_framerate_limit', { fps: fpsValue.value, profile: null })
    emit('message', t.value.rtssTool.fpsApplied.replace('{fps}', fpsValue.value), 'success')
  } catch (e) {
    emit('message', String(e), 'error')
  } finally {
    applying.value = false
  }
}

async function removeLimit() {
  applying.value = true
  try {
    await invoke('rtss_set_framerate_limit', { fps: 0, profile: null })
    fpsValue.value = 0
    emit('message', t.value.rtssTool.fpsRemoved, 'success')
  } catch (e) {
    emit('message', String(e), 'error')
  } finally {
    applying.value = false
  }
}

async function onToggleLimiter() {
  try {
    const result = await invoke('rtss_toggle_limiter')
    limiterEnabled.value = result === '1'
    emit('message',
      limiterEnabled.value ? t.value.rtssTool.limiterEnabled : t.value.rtssTool.limiterDisabled,
      'success'
    )
  } catch (e) {
    emit('message', String(e), 'error')
  }
}

onMounted(() => loadState())

defineExpose({ loadState })
</script>

<style lang="less" scoped>
.fps-display {
  text-align: center;
  padding: 2px 0 0;
}

.fps-value {
  font-size: 36px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  background: linear-gradient(135deg, #93c5fd, #c4b5fd);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.fps-label {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.6);
  margin-left: 4px;
}

.slider-container {
  padding: 0 4px;
}

.fps-slider {
  width: 100%;
  height: 8px;
  -webkit-appearance: none;
  background: rgba(255, 255, 255, 0.18);
  border-radius: 8px;
  outline: none;
  cursor: pointer;

  &::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    cursor: pointer;
    transition: transform 0.15s;

    &:hover { transform: scale(1.15); }
  }

  &:disabled { opacity: 0.4; cursor: not-allowed; }
}

.slider-labels {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
  margin-top: 4px;
  padding: 0 2px;
}

.presets {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  justify-content: center;
}

.preset-btn {
  padding: 4px 12px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { background: rgba(255, 255, 255, 0.14); color: white; }
  &.active { background: rgba(147, 197, 253, 0.25); border-color: #93c5fd; color: #c4b5fd; }
}

.actions {
  display: flex;
  gap: 8px;
  justify-content: center;
}

.apply-btn {
  padding: 6px 20px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, #60a5fa, #818cf8);
  color: white;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { opacity: 0.9; transform: translateY(-1px); }
  &:disabled { opacity: 0.4; cursor: not-allowed; transform: none; }
}

.reset-btn {
  padding: 6px 16px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { background: rgba(255, 255, 255, 0.14); color: white; }
  &:disabled { opacity: 0.4; cursor: not-allowed; }
}
</style>
