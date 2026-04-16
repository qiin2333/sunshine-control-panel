<template>
  <!-- 状态指示 -->
  <div class="status-bar" :class="{ online: status.running, offline: !status.running }">
    <span class="status-dot"></span>
    <span v-if="loading">{{ t.rtssTool.checking }}</span>
    <span v-else-if="status.running">
      RTSS v{{ status.version }} · {{ t.rtssTool.running }}
    </span>
    <span v-else>{{ t.rtssTool.notRunning }}</span>
    <button class="refresh-btn" @click="$emit('refresh')" :disabled="loading" :title="t.rtssTool.refresh">↻</button>
  </div>

  <!-- rtss-cli 检测 -->
  <div v-if="status.running && !status.cli_path" class="warning-box">
    <div class="warning-text">⚠️ {{ t.rtssTool.cliNotFound }}</div>
    <button class="download-btn" @click="downloadCli" :disabled="downloading">
      {{ downloading ? t.rtssTool.downloading : t.rtssTool.autoDownload }}
    </button>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../../../desktop/i18n/index.js'

const { t } = useI18n()

const props = defineProps({
  status: { type: Object, required: true },
  loading: { type: Boolean, default: false },
})
const emit = defineEmits(['refresh', 'message'])

const downloading = ref(false)

async function downloadCli() {
  downloading.value = true
  try {
    const path = await invoke('rtss_download_cli')
    emit('message', t.value.rtssTool.cliDownloaded, 'success')
    // 刷新状态以检测新下载的 cli
    emit('refresh')
  } catch (e) {
    emit('message', String(e), 'error')
  } finally {
    downloading.value = false
  }
}

function openCliDownload() {
  try {
    invoke('open_external_url', { url: 'https://github.com/xanderfrangos/rtss-cli/releases' })
  } catch (e) {
    console.error(e)
  }
}
</script>

<style lang="less" scoped>
.status-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-radius: 12px;
  font-size: 12px;
  font-weight: 500;
  background: rgba(0, 0, 0, 0.18);
  border: 1px solid rgba(255, 255, 255, 0.1);

  &.online .status-dot { background: #4ade80; box-shadow: 0 0 8px rgba(74, 222, 128, 0.6); }
  &.offline .status-dot { background: #f87171; box-shadow: 0 0 8px rgba(248, 113, 113, 0.6); }
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.refresh-btn {
  margin-left: auto;
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.7);
  font-size: 16px;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 6px;
  transition: all 0.2s;

  &:hover { color: white; background: rgba(255, 255, 255, 0.1); }
  &:disabled { opacity: 0.4; cursor: not-allowed; }
}

.warning-box {
  padding: 8px 14px;
  background: rgba(251, 191, 36, 0.12);
  border: 1px solid rgba(251, 191, 36, 0.35);
  border-radius: 10px;
  font-size: 12px;
  line-height: 1.4;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.warning-text {
  flex: 1;
  color: rgba(255, 255, 255, 0.9);
}

.download-btn {
  padding: 5px 12px;
  border: none;
  border-radius: 8px;
  background: linear-gradient(135deg, #f59e0b, #d97706);
  color: white;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;

  &:hover { opacity: 0.9; transform: translateY(-1px); }
  &:disabled { opacity: 0.5; cursor: not-allowed; transform: none; }
}
</style>
