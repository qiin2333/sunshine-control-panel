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
        <h4>更新内容：</h4>
        <div class="notes-content">{{ formatReleaseNotes(updateInfo.release_notes) }}</div>
      </div>

      <!-- 下载进度 -->
      <div v-if="downloadProgress > 0 && downloadProgress < 100" class="download-progress">
        <el-progress
          :percentage="downloadProgress"
          :status="downloadError ? 'exception' : undefined"
          :stroke-width="8"
        />
        <p class="progress-text">{{ downloadStatusText }}</p>
      </div>

      <!-- 安装提示 -->
      <div v-if="isInstalling" class="install-notice">
        <el-alert
          type="warning"
          :closable="false"
          show-icon
        >
          <template #title>
            <div class="install-alert-content">
              <p>正在准备安装更新...</p>
              <p class="install-tip">系统将自动关闭 Sunshine 服务和 GUI 窗口，然后启动安装程序</p>
              <p class="install-tip">安装完成后，请重新启动应用</p>
            </div>
          </template>
        </el-alert>
      </div>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <el-button
          v-if="!isInstalling && downloadProgress === 0"
          @click="handleDownload"
          type="primary"
          :loading="isDownloading"
        >
          <el-icon><Download /></el-icon>
          下载并安装
        </el-button>
        <el-button
          v-if="!isInstalling && downloadProgress === 0"
          @click="handleOpenBrowser"
        >
          <el-icon><Link /></el-icon>
          在浏览器中打开
        </el-button>
        <el-button
          v-if="!isInstalling"
          @click="handleCancel"
        >
          稍后提醒
        </el-button>
        <el-button
          v-if="isInstalling"
          type="primary"
          disabled
        >
          正在安装...
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Download, Link } from '@element-plus/icons-vue'

const props = defineProps({
  modelValue: {
    type: Boolean,
    default: false
  },
  updateInfo: {
    type: Object,
    default: null
  },
  currentVersion: {
    type: String,
    default: '0.0.0'
  }
})

const emit = defineEmits(['update:modelValue', 'close'])

const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val)
})

const isDownloading = ref(false)
const downloadProgress = ref(0)
const downloadError = ref(false)
const isInstalling = ref(false)

const downloadStatusText = computed(() => {
  if (downloadError.value) {
    return '下载失败，请重试'
  }
  if (downloadProgress.value === 0) {
    return ''
  }
  if (downloadProgress.value === 100) {
    return '下载完成，准备安装...'
  }
  return `正在下载... ${downloadProgress.value}%`
})

const formatReleaseNotes = (notes) => {
  if (!notes) return ''
  // 限制显示长度，避免对话框过大
  if (notes.length > 500) {
    return notes.substring(0, 500) + '...'
  }
  return notes
}

const handleDownload = async () => {
  if (!props.updateInfo?.download_url) {
    ElMessage.warning('未找到下载链接')
    return
  }

  try {
    isDownloading.value = true
    downloadProgress.value = 0
    downloadError.value = false

    const { invoke } = await import('@tauri-apps/api/core')
    const { listen } = await import('@tauri-apps/api/event')
    
    // 监听下载进度事件
    const progressUnlisten = await listen('download-progress', (event) => {
      const data = event.payload
      if (data.progress !== undefined) {
        downloadProgress.value = data.progress
        console.log(`📊 下载进度: ${data.progress}% (${data.downloaded}/${data.total})`)
      }
    })
    
    try {
      // 调用后端下载更新（会实时发送进度事件）
      const result = await invoke('download_update', {
        url: props.updateInfo.download_url,
        filename: props.updateInfo.download_name || `sunshine-update-${props.updateInfo.version}.msi`
      })

      // 取消监听进度事件
      await progressUnlisten()

      if (result.success) {
        downloadProgress.value = 100
        ElMessage.success('下载完成，准备安装...')
        
        // 等待一下让用户看到下载完成
        await new Promise(resolve => setTimeout(resolve, 1000))
        
        // 开始安装流程
        await handleInstall(result.file_path)
      } else {
        downloadError.value = true
        ElMessage.error(result.message || '下载失败')
      }
    } catch (error) {
      // 确保取消监听
      await progressUnlisten()
      downloadError.value = true
      ElMessage.error('下载失败: ' + error)
    }
  } catch (error) {
    console.error('下载更新失败:', error)
    downloadError.value = true
    ElMessage.error('下载失败: ' + error)
  } finally {
    isDownloading.value = false
  }
}

const handleInstall = async (filePath) => {
  try {
    // 确认安装更新
    await ElMessageBox.confirm(
      '安装更新将执行以下操作：\n\n' +
      '1. 自动关闭 Sunshine 服务\n' +
      '2. 关闭 GUI 窗口\n' +
      '3. 启动安装程序\n\n' +
      '安装完成后，请重新启动应用。\n\n是否继续？',
      '准备安装更新',
      {
        confirmButtonText: '确定安装',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )

    isInstalling.value = true

    const { invoke } = await import('@tauri-apps/api/core')
    
    // 调用后端安装更新（会自动关闭Sunshine和GUI）
    await invoke('install_update', {
      filePath: filePath
    })

    // 如果到这里说明安装程序已启动，关闭对话框
    // 注意：GUI会在3秒后自动退出
    ElMessage.success('安装程序已启动，GUI窗口将在几秒后自动关闭')
    
    // 延迟关闭对话框，让用户看到提示
    setTimeout(() => {
    visible.value = false
    }, 2000)
  } catch (error) {
    if (error !== 'cancel') {
      console.error('安装更新失败:', error)
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
    console.error('打开浏览器失败:', error)
    ElMessage.error('打开浏览器失败: ' + error)
  }
}

const handleCancel = () => {
  visible.value = false
  emit('close')
}

// 监听下载进度事件
watch(() => props.modelValue, (newVal) => {
  if (newVal) {
    // 重置状态
    downloadProgress.value = 0
    downloadError.value = false
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
  margin-bottom: 24px;
  
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
    white-space: pre-wrap;
    word-break: break-word;
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
  
  .install-alert-content {
    p {
      margin: 4px 0;
      
      &.install-tip {
        font-size: 12px;
        color: #909399;
        margin-top: 8px;
      }
    }
  }
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}
</style>

