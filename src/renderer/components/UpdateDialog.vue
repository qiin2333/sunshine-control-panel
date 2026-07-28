<template>
  <el-dialog
    v-model="visible"
    :title="dialogTitle"
    width="min(720px, 92vw)"
    :close-on-click-modal="isLatest"
    :close-on-press-escape="isLatest || (!isInstalling && !isDownloading)"
    :show-close="!isInstalling && !isDownloading"
  >
    <div class="update-dialog-content">

      <!-- 更新说明 -->
      <div v-if="updateInfo?.release_notes" class="release-notes">
        <div class="notes-content" v-html="parsedReleaseNotes"></div>
      </div>

      <!-- 下载进度 (仅新版本) -->
      <div v-if="!isLatest && isDownloading" class="download-progress">
        <div class="download-status">
          <span v-if="downloadPhase !== 'complete'" class="download-spinner"></span>
          <span v-else class="download-complete-mark">✓</span>
          <p class="progress-text">{{ downloadStatusText }}</p>
          <span
            v-if="downloadPhase === 'downloading' && downloadSource"
            class="download-source-chip"
          >
            {{ downloadSource }}
          </span>
        </div>
        <el-progress
          v-if="downloadProgress > 0"
          :percentage="downloadProgress"
          :stroke-width="8"
          :show-text="false"
          :status="downloadPhase === 'complete' ? 'success' : undefined"
        />
      </div>

      <!-- 安装提示 (仅新版本) -->
      <div v-if="!isLatest && isInstalling" class="install-progress-panel">
        <div class="install-status">
          <span class="install-spinner" :class="{ failed: installStage === 'failed' }"></span>
          <div>
            <p class="install-current">{{ currentInstallMessage }}</p>
            <p v-if="installDetail" class="install-detail">{{ installDetail }}</p>
          </div>
        </div>

        <ol class="install-steps">
          <li
            v-for="(step, index) in installSteps"
            :key="step.key"
            :class="getInstallStepStatus(index)"
          >
            <span class="step-dot"></span>
            <span class="step-label">{{ step.label }}</span>
          </li>
        </ol>

        <p class="install-note">{{ t.updateDialog.installDoNotRepeat }}</p>
        <p v-if="installWaitHint" class="install-wait-hint">{{ installWaitHint }}</p>
      </div>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <template v-if="isLatest">
          <el-button @click="handleOpenBrowser">{{ t.updateDialog.viewInBrowser }}</el-button>
          <el-button type="primary" @click="handleCancel">{{ t.updateDialog.close }}</el-button>
        </template>
        <template v-else>
          <template v-if="showDownloadButtons">
            <el-button @click="handleOpenBrowser">{{ t.updateDialog.openInBrowser }}</el-button>
            <el-button @click="handleSkipVersion">{{ t.updateDialog.skipVersion }}</el-button>
            <el-button @click="handleCancel">{{ t.updateDialog.remindLater }}</el-button>
            <el-button type="primary" :loading="isDownloading" @click="handleDownload">
              <el-icon><Download /></el-icon>
              {{ t.updateDialog.downloadAndInstall }}
            </el-button>
          </template>
          <el-button
            v-if="isDownloading"
            type="primary"
            :loading="downloadPhase !== 'complete'"
            disabled
          >
            {{
              downloadPhase === 'complete'
                ? t.updateDialog.downloadComplete
                : t.updateDialog.downloading
            }}
          </el-button>
          <el-button v-if="isInstalling" type="primary" disabled>{{ t.updateDialog.installing }}</el-button>
        </template>
      </div>
    </template>
  </el-dialog>
</template>

<script setup>
import { ref, computed, watch, onBeforeUnmount } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Download } from '@element-plus/icons-vue'
import MarkdownIt from 'markdown-it'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

const DOWNLOAD_PHASE_MESSAGE_KEYS = Object.freeze({
  connecting: 'downloadConnecting',
  retrying: 'downloadRetrying',
  verifying: 'downloadVerifying',
  complete: 'downloadComplete',
})

const DOWNLOAD_ERROR_MESSAGE_KEYS = Object.freeze({
  setup_failed: 'downloadSetupFailed',
  file_preparation_failed: 'downloadFilePreparationFailed',
  file_finalization_failed: 'downloadFileFinalizationFailed',
  sources_exhausted: 'downloadSourcesFailed',
})

const props = defineProps({
  modelValue: { type: Boolean, default: false },
  updateInfo: { type: Object, default: null },
  currentVersion: { type: String, default: '0.0.0' },
})

const emit = defineEmits(['update:modelValue', 'close', 'skip-version'])

const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
})

const isDownloading = ref(false)
const downloadProgress = ref(0)
const downloadPhase = ref('idle')
const downloadSource = ref('')
const isInstalling = ref(false)
const installStage = ref('idle')
const installDetail = ref('')
const installWaitHint = ref('')
const installLastEventAt = ref(0)
const installFailedRank = ref(null)
let installProgressUnlisten = null
let installWaitTimer = null

const isLatest = computed(() => !!props.updateInfo?.is_latest)

const dialogTitle = computed(() => {
  const ver = props.updateInfo?.version || ''
  if (isLatest.value) {
    return t.value.updateDialog.titleLatest.replace('{version}', ver)
  }
  return t.value.updateDialog.titleNew.replace('{version}', ver).replace('{current}', props.currentVersion)
})

const md = new MarkdownIt({ html: true, breaks: true, linkify: true })

// 所有链接添加 target="_blank" rel="noopener"，让 Tauri 在系统浏览器中打开
const defaultLinkRender = md.renderer.rules.link_open || ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options))
md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
  tokens[idx].attrSet('target', '_blank')
  tokens[idx].attrSet('rel', 'noopener')
  return defaultLinkRender(tokens, idx, options, env, self)
}

const parsedReleaseNotes = computed(() =>
  props.updateInfo?.release_notes ? md.render(props.updateInfo.release_notes) : ''
)

const showDownloadButtons = computed(
  () => !isDownloading.value && !isInstalling.value && downloadProgress.value === 0
)

const downloadStatusText = computed(() => {
  if (downloadPhase.value === 'downloading') {
    return t.value.updateDialog.downloadProgress.replace('{progress}', downloadProgress.value)
  }
  const messageKey = DOWNLOAD_PHASE_MESSAGE_KEYS[downloadPhase.value]
  if (messageKey) return t.value.updateDialog[messageKey]
  return t.value.updateDialog.downloading
})

const getDownloadErrorMessage = (error) => {
  const code = error && typeof error === 'object' ? error.code : ''
  const messageKey = DOWNLOAD_ERROR_MESSAGE_KEYS[code] || 'downloadFailed'

  console.error('Download update failed', error)
  return t.value.updateDialog[messageKey]
}

const applyDownloadProgress = (payload = {}) => {
  if (payload.progress !== undefined) downloadProgress.value = payload.progress
  if (payload.phase) downloadPhase.value = payload.phase
  downloadSource.value = payload.source || ''
}

const resetDownloadState = () => {
  isDownloading.value = false
  downloadProgress.value = 0
  downloadPhase.value = 'idle'
  downloadSource.value = ''
}

const installSteps = computed(() => [
  { key: 'downloaded', label: t.value.updateDialog.installStepDownloaded },
  { key: 'building-command', label: t.value.updateDialog.installStepPreparingHelper },
  { key: 'launching-installer', label: t.value.updateDialog.installStepLaunching },
  { key: 'installer-started', label: t.value.updateDialog.installStepBackground },
  { key: 'app-exiting', label: t.value.updateDialog.installStepExiting },
])

const installStageRanks = {
  idle: -1,
  preparing: 0,
  downloaded: 0,
  'building-command': 1,
  'launching-installer': 2,
  'installer-started': 3,
  'app-exiting': 4,
  failed: -1,
}

const currentInstallRank = computed(() => installStageRanks[installStage.value] ?? 0)

const currentInstallMessage = computed(() => {
  const messages = {
    idle: t.value.updateDialog.preparingInstall,
    preparing: t.value.updateDialog.installPreparing,
    downloaded: t.value.updateDialog.installPreparing,
    'building-command': t.value.updateDialog.installPreparingHelper,
    'launching-installer': t.value.updateDialog.installLaunching,
    'installer-started': t.value.updateDialog.installStartedBackground,
    'app-exiting': t.value.updateDialog.installExiting,
    failed: t.value.updateDialog.installFailed,
  }
  return messages[installStage.value] || t.value.updateDialog.preparingInstall
})

const getInstallStepStatus = (index) => {
  if (installStage.value === 'failed') {
    const failedRank = installFailedRank.value ?? Math.max(currentInstallRank.value, 0)
    if (index < failedRank) return 'done'
    if (index === failedRank) return 'failed'
    return 'pending'
  }
  if (index < currentInstallRank.value) return 'done'
  if (index === currentInstallRank.value) return 'active'
  return 'pending'
}

const clearInstallProgressListener = () => {
  if (installProgressUnlisten) {
    installProgressUnlisten()
    installProgressUnlisten = null
  }
}

const clearInstallWaitTimer = () => {
  if (installWaitTimer) {
    clearInterval(installWaitTimer)
    installWaitTimer = null
  }
}

const markInstallProgressSeen = () => {
  installLastEventAt.value = Date.now()
  installWaitHint.value = ''
}

const startInstallWaitTimer = () => {
  clearInstallWaitTimer()
  markInstallProgressSeen()
  installWaitTimer = setInterval(() => {
    if (!isInstalling.value || installStage.value === 'failed') return
    const elapsedMs = Date.now() - installLastEventAt.value
    if (elapsedMs >= 60000) {
      installWaitHint.value = t.value.updateDialog.installLongWaitHint
    } else if (elapsedMs >= 15000) {
      installWaitHint.value = t.value.updateDialog.installWaitHint
    } else {
      installWaitHint.value = ''
    }
  }, 1000)
}

const resetInstallProgress = () => {
  installStage.value = 'idle'
  installDetail.value = ''
  installWaitHint.value = ''
  installLastEventAt.value = 0
  installFailedRank.value = null
  clearInstallWaitTimer()
}

const getTauriApis = async () => {
  const [{ invoke }, { listen }] = await Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')])
  return { invoke, listen }
}

const handleDownload = async () => {
  const downloadUrl = props.updateInfo?.download_url
  if (!downloadUrl) {
    ElMessage.warning(t.value.updateDialog.noDownloadUrl)
    return
  }

  resetDownloadState()
  isDownloading.value = true
  let progressUnlisten = null

  try {
    const { invoke, listen } = await getTauriApis()

    progressUnlisten = await listen('download-progress', (event) => {
      applyDownloadProgress(event.payload)
    })

    const filename = props.updateInfo.download_name || `sunshine-update-${props.updateInfo.version}.msi`
    const result = await invoke('download_update', {
      url: downloadUrl,
      filename,
      expectedSize: props.updateInfo.download_size || null,
    })
    await progressUnlisten()
    progressUnlisten = null

    if (result.success) {
      downloadProgress.value = 100
      downloadPhase.value = 'complete'
      ElMessage.success(t.value.updateDialog.downloadComplete)
      await new Promise((resolve) => setTimeout(resolve, 1000))
      isDownloading.value = false
      await handleInstall(result.file_path)
    } else {
      console.error('Download update returned an unsuccessful result', result)
      ElMessage.error(t.value.updateDialog.downloadFailed)
    }
  } catch (error) {
    resetDownloadState()
    ElMessage.error(getDownloadErrorMessage(error))
  } finally {
    if (progressUnlisten) {
      await progressUnlisten()
    }
    isDownloading.value = false
  }
}

const handleInstall = async (filePath) => {
  try {
    await ElMessageBox.confirm(
      t.value.updateDialog.installConfirm,
      t.value.updateDialog.installTitle,
      { confirmButtonText: t.value.updateDialog.installConfirmBtn, cancelButtonText: t.value.updateDialog.cancelBtn, type: 'warning' }
    )

    isInstalling.value = true
    resetInstallProgress()
    installStage.value = 'downloaded'
    startInstallWaitTimer()

    const { invoke, listen } = await getTauriApis()
    clearInstallProgressListener()
    installProgressUnlisten = await listen('install-progress', (event) => {
      const payload = event.payload || {}
      if (payload.stage) {
        if (payload.stage === 'failed') {
          installFailedRank.value = Math.max(currentInstallRank.value, 0)
        } else {
          installFailedRank.value = null
        }
        installStage.value = payload.stage
      }
      installDetail.value = payload.detail || ''
      markInstallProgressSeen()
      if (payload.terminal) clearInstallWaitTimer()
    })

    await invoke('install_update', { filePath, targetVersion: props.updateInfo?.version || null })

    ElMessage.success(t.value.updateDialog.installStarted)
  } catch (error) {
    if (error !== 'cancel') {
      installFailedRank.value = Math.max(currentInstallRank.value, 0)
      installStage.value = 'failed'
      installDetail.value = String(error)
      clearInstallWaitTimer()
      clearInstallProgressListener()
      ElMessage.error(t.value.updateDialog.installError.replace('{error}', error))
      isInstalling.value = false
    } else {
      downloadProgress.value = 0
    }
  }
}

const handleOpenBrowser = async () => {
  const releasePage = props.updateInfo?.release_page
  if (!releasePage) {
    ElMessage.warning(t.value.updateDialog.noReleasePage)
    return
  }

  try {
    const { invoke } = await getTauriApis()
    await invoke('open_external_url', { url: releasePage })
  } catch (error) {
    ElMessage.error(t.value.updateDialog.openBrowserError.replace('{error}', error))
  }
}

const handleCancel = () => {
  visible.value = false
  emit('close')
}

const handleSkipVersion = () => {
  const version = props.updateInfo?.version
  if (version) {
    emit('skip-version', version)
  }
  visible.value = false
  emit('close')
}

const resetState = () => {
  resetDownloadState()
  isInstalling.value = false
  resetInstallProgress()
  clearInstallProgressListener()
}

onBeforeUnmount(() => {
  clearInstallWaitTimer()
  clearInstallProgressListener()
})

watch(
  () => props.modelValue,
  (newVal) => {
    if (newVal) resetState()
  }
)
</script>

<style scoped lang="less">
@border-radius: 8px;

.update-dialog-content {
  padding: 0;
}

.release-notes {
  margin-bottom: 0;

  .notes-content {
    max-height: 400px;
    overflow-y: auto;
    padding: 14px 18px;
    background: #f5f7fa;
    border-radius: @border-radius;
    color: #4a4a4a;
    font-size: 14px;
    line-height: 1.7;

    &::-webkit-scrollbar {
      width: 6px;
    }

    &::-webkit-scrollbar-track {
      background: transparent;
      border-radius: 3px;
    }

    &::-webkit-scrollbar-thumb {
      background: #c0c4cc;
      border-radius: 3px;

      &:hover {
        background: #909399;
      }
    }

    :deep(h2),
    :deep(h3) {
      margin: 12px 0 8px;
      font-weight: 600;
      color: #303133;
    }

    :deep(p) {
      margin: 8px 0;
    }

    :deep(ul),
    :deep(ol) {
      margin: 8px 0;
      padding-left: 24px;
    }

    :deep(li) {
      margin: 4px 0;
    }

    :deep(code) {
      padding: 2px 6px;
      background: #e4e7ed;
      border-radius: 3px;
    }

    :deep(a) {
      color: #409eff;
      text-decoration: none;
      word-break: break-all;

      &:hover {
        text-decoration: underline;
      }
    }
  }
}

.download-progress {
  margin-top: 14px;
  margin-bottom: 4px;

  .download-status {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    min-height: 28px;
    margin-bottom: 10px;
  }

  .download-spinner {
    width: 14px;
    height: 14px;
    flex: 0 0 14px;
    border: 2px solid #c6e2ff;
    border-top-color: #409eff;
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }

  .download-complete-mark {
    width: 18px;
    height: 18px;
    color: #67c23a;
    font-size: 18px;
    font-weight: 700;
    line-height: 18px;
  }

  .download-source-chip {
    display: inline-flex;
    align-items: center;
    min-height: 20px;
    padding: 1px 9px;
    border: 1px solid #b3d8ff;
    border-radius: 999px;
    background: #ecf5ff;
    color: #409eff;
    font-size: 12px;
    font-weight: 600;
    line-height: 18px;
    white-space: nowrap;
  }

  .progress-text {
    text-align: center;
    color: #909399;
    font-size: 14px;
    margin: 0;
  }
}

.install-progress-panel {
  margin-top: 16px;
  padding: 16px;
  border: 1px solid #dcdfe6;
  border-radius: @border-radius;
  background: #f8fafc;
}

.install-status {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  margin-bottom: 14px;
}

.install-spinner {
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  margin-top: 2px;
  border: 2px solid #c6e2ff;
  border-top-color: #409eff;
  border-radius: 50%;
  animation: spin 0.9s linear infinite;

  &.failed {
    border-color: #f56c6c;
    animation: none;
  }
}

.install-current {
  margin: 0;
  color: #303133;
  font-size: 14px;
  font-weight: 600;
}

.install-detail,
.install-note,
.install-wait-hint {
  margin: 6px 0 0;
  color: #606266;
  font-size: 13px;
  line-height: 1.5;
}

.install-wait-hint {
  color: #b88230;
}

.install-steps {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(118px, 1fr));
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;

  li {
    display: flex;
    align-items: center;
    min-width: 0;
    gap: 6px;
    color: #909399;
    font-size: 12px;
    line-height: 1.3;
  }

  .step-dot {
    width: 10px;
    height: 10px;
    flex: 0 0 10px;
    border: 2px solid #c0c4cc;
    border-radius: 50%;
    background: #fff;
  }

  .step-label {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  li.done {
    color: #409eff;

    .step-dot {
      border-color: #409eff;
      background: #409eff;
    }
  }

  li.active {
    color: #303133;
    font-weight: 600;

    .step-dot {
      border-color: #409eff;
      background: #ecf5ff;
    }
  }

  li.failed {
    color: #f56c6c;

    .step-dot {
      border-color: #f56c6c;
      background: #fef0f0;
    }
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
}
</style>
