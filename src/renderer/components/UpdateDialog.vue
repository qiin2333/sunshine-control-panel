<template>
  <el-dialog
    v-model="visible"
    title="发现新版本"
    width="600px"
    :close-on-click-modal="false"
    :close-on-press-escape="false"
    :show-close="!isInstalling"
  >
    <div class="update-dialog-content">
      <!-- 版本信息 -->
      <div class="version-info">
        <div class="version-badge">
          <el-icon :size="24"><Download /></el-icon>
          <span class="version-text">{{ updateInfo?.version }}</span>
        </div>
        <p class="current-version">当前版本: {{ currentVersion }}</p>
      </div>

      <!-- 更新说明 -->
      <div v-if="updateInfo?.release_notes" class="release-notes">
        <!-- <h4>更新内容：</h4> -->
        <div class="notes-content" v-html="parsedReleaseNotes"></div>
      </div>

      <!-- 下载进度 -->
      <div v-if="downloadProgress > 0 && downloadProgress < 100" class="download-progress">
        <el-progress :percentage="downloadProgress" :stroke-width="8" />
        <p class="progress-text">正在下载... {{ downloadProgress }}%</p>
      </div>

      <!-- 安装提示 -->
      <div v-if="isInstalling" class="install-notice">
        <el-alert type="warning" :closable="false" show-icon>
          <template #title>
            <p>正在准备安装更新，系统将自动关闭服务并启动安装程序</p>
          </template>
        </el-alert>
      </div>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <template v-if="!isInstalling && downloadProgress === 0">
          <el-button type="primary" :loading="isDownloading" @click="handleDownload">
            <el-icon><Download /></el-icon>
            下载并安装
          </el-button>
          <el-button @click="handleOpenBrowser">
            <el-icon><Link /></el-icon>
            在浏览器中打开
          </el-button>
        </template>
        <el-button v-if="!isInstalling" @click="handleCancel">稍后提醒</el-button>
        <el-button v-if="isInstalling" type="primary" disabled>正在安装...</el-button>
      </div>
    </template>
  </el-dialog>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Download, Link } from '@element-plus/icons-vue'
import MarkdownIt from 'markdown-it'

const props = defineProps({
  modelValue: { type: Boolean, default: false },
  updateInfo: { type: Object, default: null },
  currentVersion: { type: String, default: '0.0.0' }
})

const emit = defineEmits(['update:modelValue', 'close'])

const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val)
})

const isDownloading = ref(false)
const downloadProgress = ref(0)
const isInstalling = ref(false)

const md = new MarkdownIt({ html: true, breaks: true, linkify: true })

const parsedReleaseNotes = computed(() => {
  return props.updateInfo?.release_notes ? md.render(props.updateInfo.release_notes) : ''
})

const handleDownload = async () => {
  if (!props.updateInfo?.download_url) {
    ElMessage.warning('未找到下载链接')
    return
  }

  isDownloading.value = true
  downloadProgress.value = 0

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const { listen } = await import('@tauri-apps/api/event')
    
    const progressUnlisten = await listen('download-progress', (event) => {
      if (event.payload.progress !== undefined) {
        downloadProgress.value = event.payload.progress
      }
    })
    
    const result = await invoke('download_update', {
      url: props.updateInfo.download_url,
      filename: props.updateInfo.download_name || `sunshine-update-${props.updateInfo.version}.msi`
    })

    await progressUnlisten()

    if (result.success) {
      downloadProgress.value = 100
      ElMessage.success('下载完成，准备安装...')
      await new Promise(resolve => setTimeout(resolve, 1000))
      await handleInstall(result.file_path)
    } else {
      ElMessage.error(result.message || '下载失败')
    }
  } catch (error) {
    ElMessage.error('下载失败: ' + error)
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
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('install_update', { filePath })
    
    ElMessage.success('安装程序已启动')
    setTimeout(() => { visible.value = false }, 2000)
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('安装失败: ' + error)
      isInstalling.value = false
    }
  }
}

const handleOpenBrowser = async () => {
  if (!props.updateInfo?.release_page) {
    ElMessage.warning('未找到发布页面链接')
    return
  }
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('open_external_url', { url: props.updateInfo.release_page })
  } catch (error) {
    ElMessage.error('打开浏览器失败: ' + error)
  }
}

const handleCancel = () => {
  visible.value = false
  emit('close')
}

watch(() => props.modelValue, (newVal) => {
  if (newVal) {
    downloadProgress.value = 0
    isInstalling.value = false
    isDownloading.value = false
  }
})
</script>

<style scoped lang="less">
.update-dialog-content {
  padding: 20px 0;
}

.version-info {
  text-align: center;
  margin-bottom: 24px;
}

.version-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 12px 24px;
  background: linear-gradient(135deg, #4a9eff 0%, #7ab8ff 100%);
  border-radius: 12px;
  color: white;
  margin-bottom: 12px;
  
  .version-text {
    font-size: 20px;
    font-weight: 600;
  }
}

.current-version {
  color: #909399;
  font-size: 14px;
  margin: 0;
}

.release-notes {
  h4 {
    color: #303133;
    font-size: 16px;
    margin-bottom: 12px;
  }
  
  .notes-content {
    max-height: 200px;
    overflow-y: auto;
    padding: 12px;
    background: #f5f7fa;
    border-radius: 8px;
    color: #606266;
    font-size: 14px;
    line-height: 1.6;
    
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
    }
    
    &::-webkit-scrollbar-thumb:hover {
      background: #909399;
    }
    
    :deep(h2), :deep(h3) { margin: 12px 0 8px; font-weight: 600; color: #303133; }
    :deep(p) { margin: 8px 0; }
    :deep(ul), :deep(ol) { margin: 8px 0; padding-left: 24px; }
    :deep(li) { margin: 4px 0; }
    :deep(code) { padding: 2px 6px; background: #e4e7ed; border-radius: 3px; }
    :deep(a) { color: #409eff; text-decoration: none; }
    :deep(a:hover) { text-decoration: underline; }
  }
}

.download-progress {
  margin-bottom: 24px;
  
  .progress-text {
    text-align: center;
    color: #909399;
    font-size: 14px;
    margin-top: 8px;
  }
}

.install-notice {
  margin-bottom: 24px;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}
</style>
