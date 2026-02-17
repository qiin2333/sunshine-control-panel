<template>
  <el-dialog
    v-model="visible"
    :title="dialogTitle"
    width="720px"
    :close-on-click-modal="isLatest"
    :close-on-press-escape="isLatest || !isInstalling"
    :show-close="!isInstalling"
  >
    <div class="update-dialog-content">

      <!-- 更新说明 -->
      <div v-if="updateInfo?.release_notes" class="release-notes">
        <div class="notes-content" v-html="parsedReleaseNotes"></div>
      </div>

      <!-- 下载进度 (仅新版本) -->
      <div v-if="!isLatest && isDownloadInProgress" class="download-progress">
        <el-progress :percentage="downloadProgress" :stroke-width="8" />
        <p class="progress-text">正在下载... {{ downloadProgress }}%</p>
      </div>

      <!-- 安装提示 (仅新版本) -->
      <div v-if="!isLatest && isInstalling" class="install-notice">
        <el-alert type="warning" :closable="false" show-icon>
          <template #title>
            <p>正在准备安装更新，系统将自动关闭服务并启动安装程序</p>
          </template>
        </el-alert>
      </div>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <template v-if="isLatest">
          <el-button @click="handleOpenBrowser">在浏览器中查看</el-button>
          <el-button type="primary" @click="handleCancel">关闭</el-button>
        </template>
        <template v-else>
          <template v-if="showDownloadButtons">
            <el-button @click="handleOpenBrowser">在浏览器中打开</el-button>
            <el-button @click="handleSkipVersion">忽略此版本</el-button>
            <el-button @click="handleCancel">稍后提醒</el-button>
            <el-button type="primary" :loading="isDownloading" @click="handleDownload">
              <el-icon><Download /></el-icon>
              下载并安装
            </el-button>
          </template>
          <el-button v-if="isInstalling" type="primary" disabled>正在安装...</el-button>
        </template>
      </div>
    </template>
  </el-dialog>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Download, Link, CircleCheckFilled } from '@element-plus/icons-vue'
import MarkdownIt from 'markdown-it'

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
const isInstalling = ref(false)

const isLatest = computed(() => !!props.updateInfo?.is_latest)

const dialogTitle = computed(() => {
  const ver = props.updateInfo?.version || ''
  if (isLatest.value) {
    return `已经是最新版本——此版本更新内容：${ver}`
  }
  return `发现新版本：${ver} （当前版本: ${props.currentVersion}）`
})

const md = new MarkdownIt({ html: true, breaks: true, linkify: true })

const parsedReleaseNotes = computed(() =>
  props.updateInfo?.release_notes ? md.render(props.updateInfo.release_notes) : ''
)

const isDownloadInProgress = computed(() => downloadProgress.value > 0 && downloadProgress.value < 100)

const showDownloadButtons = computed(() => !isInstalling.value && downloadProgress.value === 0)

const getTauriApis = async () => {
  const [{ invoke }, { listen }] = await Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')])
  return { invoke, listen }
}

const handleDownload = async () => {
  const downloadUrl = props.updateInfo?.download_url
  if (!downloadUrl) {
    ElMessage.warning('未找到下载链接')
    return
  }

  isDownloading.value = true
  downloadProgress.value = 0

  try {
    const { invoke, listen } = await getTauriApis()

    const progressUnlisten = await listen('download-progress', (event) => {
      if (event.payload.progress !== undefined) {
        downloadProgress.value = event.payload.progress
      }
    })

    const filename = props.updateInfo.download_name || `sunshine-update-${props.updateInfo.version}.msi`
    const result = await invoke('download_update', { url: downloadUrl, filename })

    await progressUnlisten()

    if (result.success) {
      downloadProgress.value = 100
      ElMessage.success('下载完成，准备安装...')
      await new Promise((resolve) => setTimeout(resolve, 1000))
      await handleInstall(result.file_path)
    } else {
      ElMessage.error(result.message || '下载失败')
    }
  } catch (error) {
    ElMessage.error(`下载失败: ${error}`)
  } finally {
    isDownloading.value = false
  }
}

const handleInstall = async (filePath) => {
  try {
    await ElMessageBox.confirm(
      '安装更新将关闭 Sunshine 服务和 GUI 窗口，然后启动安装程序。\n安装完成后请重新启动应用。是否继续？',
      '准备安装更新',
      { confirmButtonText: '确定安装', cancelButtonText: '取消', type: 'warning' }
    )

    isInstalling.value = true
    const { invoke } = await getTauriApis()
    await invoke('install_update', { filePath })

    ElMessage.success('安装程序已启动')
    setTimeout(() => {
      visible.value = false
    }, 2000)
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`安装失败: ${error}`)
      isInstalling.value = false
    }
  }
}

const handleOpenBrowser = async () => {
  const releasePage = props.updateInfo?.release_page
  if (!releasePage) {
    ElMessage.warning('未找到发布页面链接')
    return
  }

  try {
    const { invoke } = await getTauriApis()
    await invoke('open_external_url', { url: releasePage })
  } catch (error) {
    ElMessage.error(`打开浏览器失败: ${error}`)
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
  downloadProgress.value = 0
  isInstalling.value = false
  isDownloading.value = false
}

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
  margin-bottom: 20px;

  .progress-text {
    text-align: center;
    color: #909399;
    font-size: 14px;
    margin-top: 8px;
  }
}

.install-notice {
  margin-bottom: 20px;
}

.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
}
</style>
