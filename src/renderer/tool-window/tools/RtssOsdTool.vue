<template>
  <div class="tool-container" :class="{ 'embedded': embedded }">
    <div v-if="!embedded" class="tool-header">
      <h2>{{ t.rtssTool.title }}</h2>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="tool-content">
      <RtssStatusBar
        :status="status"
        :loading="statusLoading"
        @refresh="loadStatus"
        @message="showMessage"
      />

      <template v-if="status.running">
        <div class="two-col-grid">
          <RtssFpsSection
            v-if="status.cli_path"
            @message="showMessage"
          />

          <RtssOsdEditor
            :has-cli="!!status.cli_path"
            @message="showMessage"
          />

          <RtssOsdSettings
            v-if="status.cli_path"
            @message="showMessage"
          />

          <RtssMonitoring
            @message="showMessage"
          />

          <HwInfoMonitor
            @message="showMessage"
          />
        </div>
      </template>

      <!-- 消息提示 -->
      <div v-if="message" :class="['message', messageType]">
        {{ message }}
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../../desktop/i18n/index.js'

import RtssStatusBar from './rtss/RtssStatusBar.vue'
import RtssFpsSection from './rtss/RtssFpsSection.vue'
import RtssOsdEditor from './rtss/RtssOsdEditor.vue'
import RtssOsdSettings from './rtss/RtssOsdSettings.vue'
import RtssMonitoring from './rtss/RtssMonitoring.vue'
import HwInfoMonitor from './rtss/HwInfoMonitor.vue'

const { t } = useI18n()

defineProps({
  embedded: { type: Boolean, default: false },
})
defineEmits(['close'])

const statusLoading = ref(true)
const status = ref({
  running: false,
  version: '',
  osd_slot_count: 0,
  app_count: 0,
  cli_path: '',
  hooks_dll_path: '',
})

const message = ref('')
const messageType = ref('')

async function loadStatus() {
  statusLoading.value = true
  try {
    status.value = await invoke('get_rtss_status')
  } catch (e) {
    console.error('获取 RTSS 状态失败:', e)
    status.value.running = false
  } finally {
    statusLoading.value = false
  }
}

function showMessage(msg, type) {
  message.value = msg
  messageType.value = type
  setTimeout(() => { message.value = '' }, 4000)
}

onMounted(() => {
  loadStatus()
})
</script>

<style lang="less" scoped>
.tool-container {
  width: 100%;
  color: white;
  font-size: 14px;

  &.embedded {
    .tool-header { display: none; }
    .tool-content { padding: 0; }
  }
}

.tool-header {
  padding: 10px 16px;
  background: rgba(0, 0, 0, 0.15);
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  position: relative;

  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    text-align: center;
    letter-spacing: 0.5px;
  }
}

.close-btn {
  position: absolute;
  top: 6px;
  right: 10px;
  width: 26px;
  height: 26px;
  border: none;
  background: rgba(255, 255, 255, 0.15);
  color: white;
  font-size: 18px;
  line-height: 1;
  border-radius: 50%;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;

  &:hover {
    background: rgba(255, 255, 255, 0.25);
    transform: rotate(90deg);
  }
}

.tool-content {
  padding: 20px 24px 28px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  max-width: 880px;
  margin: 0 auto;
  width: 100%;
}

.two-col-grid {
  column-count: 2;
  column-gap: 10px;

  > * {
    break-inside: avoid;
    margin-bottom: 10px;
  }
}

.message {
  padding: 8px 14px;
  border-radius: 10px;
  font-size: 12px;
  font-weight: 500;
  text-align: center;
  animation: fadeIn 0.2s;

  &.success {
    background: rgba(74, 222, 128, 0.15);
    border: 1px solid rgba(74, 222, 128, 0.3);
    color: #4ade80;
  }

  &.error {
    background: rgba(248, 113, 113, 0.15);
    border: 1px solid rgba(248, 113, 113, 0.3);
    color: #f87171;
  }
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: none; }
}
</style>
