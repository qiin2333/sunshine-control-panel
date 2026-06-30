<template>
  <div class="tool-container" :class="{ 'embedded': embedded }">
    <div v-if="!embedded" class="tool-header">
      <h2>{{ t.bitrateTool.title }}</h2>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="tool-content">
      <!-- 客户端选择 -->
      <div class="section">
        <label class="section-label">{{ t.bitrateTool.selectClient }}</label>
        <select
          v-model="selectedClient"
          class="client-select"
          :disabled="isLoading || applying"
          @change="onClientChange"
        >
          <option value="">{{ t.bitrateTool.selectClientPlaceholder }}</option>
          <option v-for="session in activeSessions" :key="session.client_name" :value="session.client_name">
            {{ session.client_name }} ({{ session.width }}x{{ session.height }}@{{ session.fps }}fps)
          </option>
        </select>
        <button
          class="refresh-btn"
          :class="{ refreshing }"
          @click="loadSessions(true)"
          :disabled="isLoading || applying"
          :title="t.bitrateTool.refresh"
        >
          <RefreshRight class="refresh-icon" :class="{ spinning: refreshing }" />
        </button>
      </div>

      <!-- 加载状态（仅首次加载时显示） -->
      <div v-if="isLoading" class="loading-state">
        <p>{{ t.bitrateTool.loading }}</p>
      </div>

      <!-- 无会话提示 -->
      <template v-else-if="activeSessions.length === 0">
        <div class="empty-state">
          <div class="icon"><Connection /></div>
          <p>{{ t.bitrateTool.noStream }}</p>
          <p class="subtitle">{{ t.bitrateTool.noStreamSub }}</p>
          <p v-if="allSessions.length > 0" class="subtitle warning-text">
            {{ t.bitrateTool.sessionsIdle.replace('{count}', allSessions.length) }}
          </p>
        </div>
      </template>

      <!-- 码率调整界面 -->
      <template v-else-if="selectedClient">
        <div class="bitrate-controls">
          <!-- 当前码率显示 -->
          <div class="bitrate-display">
            <span class="bitrate-value">{{ formatBitrate(bitrateValue) }}</span>
            <span class="bitrate-label">{{ t.bitrateTool.targetBitrate }}</span>
            <div v-if="currentBitrate" class="current-bitrate">
              <span class="current-bitrate-label">{{ t.bitrateTool.currentBitrate }}</span>
              <span class="current-bitrate-value">{{ formatBitrate(currentBitrate) }}</span>
            </div>
          </div>

          <!-- 码率滑块 -->
          <div class="slider-container">
            <input
              type="range"
              v-model.number="bitrateValue"
              :min="BITRATE_LIMITS.MIN"
              :max="BITRATE_LIMITS.MAX"
              :step="BITRATE_LIMITS.STEP"
              class="bitrate-slider"
              :disabled="applying"
            />
            <div class="slider-labels">
              <span>{{ formatBitrate(BITRATE_LIMITS.MIN) }}</span>
              <span>{{ formatBitrate(BITRATE_LIMITS.MAX) }}</span>
            </div>
          </div>

          <!-- 预设按钮 -->
          <div class="presets">
            <button
              v-for="preset in BITRATE_PRESETS"
              :key="preset"
              @click="bitrateValue = preset"
              :class="{ active: bitrateValue === preset }"
              class="preset-btn"
              :disabled="applying"
            >
              {{ formatBitrate(preset) }}
            </button>
          </div>

          <!-- 自定义输入 -->
          <div class="custom-input">
            <input
              type="number"
              v-model.number="bitrateValue"
              :min="BITRATE_LIMITS.MIN"
              :max="BITRATE_LIMITS.MAX"
              :step="BITRATE_LIMITS.STEP"
              class="bitrate-input"
              :disabled="applying"
              :placeholder="t.bitrateTool.inputPlaceholder"
            />
            <span class="input-label">Kbps</span>
          </div>

          <!-- 应用按钮 -->
          <div class="actions">
            <button @click="applyBitrate" class="apply-btn" :disabled="applying || !selectedClient">
              {{ applying ? t.bitrateTool.applying : t.bitrateTool.apply }}
            </button>
          </div>
        </div>
      </template>

      <!-- 未选择客户端提示 -->
      <div v-else class="empty-state">
        <div class="icon"><Top /></div>
        <p>{{ t.bitrateTool.selectFirst }}</p>
      </div>

      <!-- 消息提示 -->
      <transition name="message-fade">
        <div v-if="message" :class="['message', messageType]">
          {{ message }}
        </div>
      </transition>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { sunshine } from '../../tauri-adapter.js'
import { RefreshRight, Connection, Top } from '@element-plus/icons-vue'
import { useI18n } from '../../desktop/i18n/index.js'

const { t } = useI18n()

// 常量定义
const BITRATE_LIMITS = Object.freeze({
  MIN: 1000,
  MAX: 800000,
  STEP: 1000,
})

const BITRATE_PRESETS = Object.freeze([5000, 10000, 20000, 50000, 100000, 200000])
const DEFAULT_BITRATE = 20000
const MESSAGE_TIMEOUT = 5000
const REFRESH_DELAY = 1000

defineProps({
  embedded: {
    type: Boolean,
    default: false
  }
})

defineEmits(['close'])

// 响应式状态
const activeSessions = ref([])
const allSessions = ref([])
const selectedClient = ref('')
const bitrateValue = ref(DEFAULT_BITRATE)
const loading = ref(true)
const refreshing = ref(false)
const applying = ref(false)
const message = ref('')
const messageType = ref('')

// 计算属性
const isLoading = computed(() => loading.value && !refreshing.value)

const selectedSession = computed(() => {
  if (!selectedClient.value) return null
  return activeSessions.value.find((s) => s.client_name === selectedClient.value) ?? null
})

const currentBitrate = computed(() => selectedSession.value?.bitrate ?? null)

// 工具函数
const formatBitrate = (kbps) => (kbps >= 1000 ? `${(kbps / 1000).toFixed(0)} Mbps` : `${kbps} Kbps`)

const isValidBitrate = (value) => value >= BITRATE_LIMITS.MIN && value <= BITRATE_LIMITS.MAX

let messageTimer = null
const showMessage = (msg, type = 'info', timeout = MESSAGE_TIMEOUT) => {
  if (messageTimer) clearTimeout(messageTimer)
  message.value = msg
  messageType.value = type
  messageTimer = setTimeout(() => {
    message.value = ''
    messageTimer = null
  }, timeout)
}

const getSessionBitrate = (clientName) => {
  const session = activeSessions.value.find((s) => s.client_name === clientName)
  return session?.bitrate ?? DEFAULT_BITRATE
}

// 会话管理
const loadSessions = async (isRefresh = false) => {
  if (isRefresh) {
    refreshing.value = true
  } else {
    loading.value = true
  }
  message.value = ''

  const previousClient = selectedClient.value

  try {
    const sessions = await sunshine.getActiveSessions()
    allSessions.value = sessions
    activeSessions.value = sessions.filter((s) => s.state !== 'STOPPED' && s.state !== 'STOPPING')

    if (activeSessions.value.length === 0) {
      selectedClient.value = ''
      return
    }

    // 恢复或选择客户端
    const clientExists = activeSessions.value.some((s) => s.client_name === previousClient)
    selectedClient.value = previousClient && clientExists ? previousClient : activeSessions.value[0].client_name

    // 设置当前客户端码率到滑块
    bitrateValue.value = getSessionBitrate(selectedClient.value)
  } catch (error) {
    console.error('获取活动会话失败:', error)
    showMessage(t.value.bitrateTool.loadFailed.replace('{error}', error), 'error')
  } finally {
    loading.value = false
    refreshing.value = false
  }
}

const onClientChange = () => {
  if (selectedClient.value) {
    bitrateValue.value = getSessionBitrate(selectedClient.value)
  }
}

// 码率调整
const applyBitrate = async () => {
  if (!selectedClient.value) {
    showMessage(t.value.bitrateTool.selectFirstError, 'error', 3000)
    return
  }

  if (!isValidBitrate(bitrateValue.value)) {
    showMessage(
      t.value.bitrateTool.rangeError
        .replace('{min}', formatBitrate(BITRATE_LIMITS.MIN))
        .replace('{max}', formatBitrate(BITRATE_LIMITS.MAX)),
      'error',
      3000
    )
    return
  }

  applying.value = true
  message.value = ''

  try {
    const result = await sunshine.changeBitrate(selectedClient.value, bitrateValue.value)
    showMessage(`✅ ${result}`, 'success')
    setTimeout(() => loadSessions(true), REFRESH_DELAY)
  } catch (error) {
    console.error('码率调整错误:', error)
    const errorMessage = error.toString()
    if (errorMessage.includes('身份验证') || errorMessage.includes('401')) {
      showMessage(t.value.bitrateTool.authFailed, 'error')
    } else {
      showMessage(t.value.bitrateTool.applyFailed.replace('{error}', error), 'error')
    }
  } finally {
    applying.value = false
  }
}

onMounted(loadSessions)
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
      min-height: 236px;
    }
  }
}

.tool-header {
  padding: 16px 24px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  position: relative;

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    text-align: center;
  }
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

  &:hover {
    background: rgba(255, 255, 255, 0.3);
    transform: rotate(90deg);
  }
}

.tool-content {
  padding: 20px 30px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.section {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.section-label {
  font-size: 14px;
  font-weight: 500;
  white-space: nowrap;
  flex-shrink: 0;
}

.client-select {
  flex: 1;
  min-width: 0;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 6px;
  color: white;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  overflow: hidden;
  text-overflow: ellipsis;

  &:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.15);
    border-color: rgba(255, 255, 255, 0.3);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  option {
    background: #2a2a2a;
    color: white;
  }
}

.refresh-btn {
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border: none;
  background: rgba(255, 255, 255, 0.1);
  color: white;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;

  &:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.2);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .spinning {
    animation: spin 1s linear infinite;
  }

  .refresh-icon {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
  }
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.loading-state,
.empty-state {
  text-align: center;
  padding: 28px 20px;
}

.empty-state {
  min-height: 190px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;

  .icon {
    width: 64px;
    height: 64px;
    margin-bottom: 16px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.88);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;

    :deep(svg) {
      width: 34px;
      height: 34px;
      flex-shrink: 0;
    }
  }

  p {
    font-size: 16px;
    color: rgba(255, 255, 255, 0.9);
    opacity: 1;
    margin-bottom: 8px;
  }

  .subtitle {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.62);
    opacity: 1;
  }

  .warning-text {
    margin-top: 8px;
    color: rgba(255, 152, 0, 0.9);
  }
}

.bitrate-controls {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.bitrate-display {
  text-align: center;
  padding: 20px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 8px;
}

.bitrate-value {
  font-size: 48px;
  font-weight: 700;
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
  display: block;
  margin-bottom: 8px;
  animation: pulse 2s ease-in-out infinite;
}

.bitrate-label {
  font-size: 14px;
  opacity: 0.8;
}

@keyframes pulse {
  0%,
  100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.02);
  }
}

.slider-container {
  padding: 0 5px;
}

.bitrate-slider {
  width: 100%;
  height: 6px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.3);
  outline: none;
  -webkit-appearance: none;
  appearance: none;
  cursor: pointer;

  &::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: white;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
    transition: transform 0.2s;

    &:hover {
      transform: scale(1.15);
    }
  }

  &::-moz-range-thumb {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: white;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
    border: none;
    transition: transform 0.2s;

    &:hover {
      transform: scale(1.15);
    }
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.slider-labels {
  display: flex;
  justify-content: space-between;
  margin-top: 4px;
  font-size: 12px;
  opacity: 0.7;
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

  &:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.2);
    border-color: rgba(255, 255, 255, 0.5);
  }

  &.active {
    background: white;
    color: #4a9eff;
    border-color: white;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.custom-input {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 6px;
}

.bitrate-input {
  flex: 1;
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  padding: 6px 10px;
  color: white;
  font-size: 14px;
  outline: none;
  transition: all 0.2s;

  &:focus {
    border-color: rgba(255, 255, 255, 0.4);
    background: rgba(255, 255, 255, 0.05);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.input-label {
  font-size: 14px;
  opacity: 0.8;
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

  &:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  &:active:not(:disabled) {
    transform: translateY(0);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.message {
  text-align: center;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 12px;

  &.success {
    background: rgba(76, 175, 80, 0.9);
  }

  &.warning {
    background: rgba(255, 152, 0, 0.9);
  }

  &.error {
    background: rgba(244, 67, 54, 0.9);
  }
}

.message-fade-enter-active,
.message-fade-leave-active {
  transition: all 0.3s ease;
}

.message-fade-enter-from {
  opacity: 0;
  transform: translateY(-10px);
}

.message-fade-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
</style>
