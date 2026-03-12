<template>
  <div class="webstream-settings-wrapper">
    <div class="webstream-header">
      <h2>
        <el-icon class="header-icon"><Connection /></el-icon>
        Web 串流服务
      </h2>
    </div>

    <div class="webstream-content">
      <div class="webstream-form">
        <!-- 描述 -->
        <p class="section-desc">
          通过浏览器远程串流，无需安装客户端。基于
          <el-link type="primary" href="https://github.com/MrCreativ3001/moonlight-web-stream" target="_blank">
            Moonlight Web
          </el-link>
        </p>

        <!-- 状态区域 -->
        <div class="status-section">
          <div class="status-row">
            <div class="status-info">
              <span class="status-dot" :class="status.running ? 'dot-running' : 'dot-stopped'" />
              <span class="status-label">{{ status.running ? '运行中' : (status.installed ? '已停止' : '未安装') }}</span>
              <span v-if="status.version" class="status-version">v{{ status.version }}</span>
            </div>
            <div class="status-actions">
              <el-button
                v-if="!status.installed"
                type="primary"
                :loading="downloading"
                @click="handleInstall"
                round
              >
                <el-icon><Download /></el-icon>
                安装
              </el-button>
              <template v-else>
                <el-button
                  v-if="!status.running"
                  type="primary"
                  :loading="starting"
                  @click="handleStart"
                  round
                >
                  <el-icon><VideoPlay /></el-icon>
                  启动
                </el-button>
                <el-button
                  v-else
                  type="danger"
                  :loading="stopping"
                  @click="handleStop"
                  round
                >
                  <el-icon><VideoPause /></el-icon>
                  停止
                </el-button>
                <el-button @click="handleCheckUpdate" round>
                  <el-icon><Refresh /></el-icon>
                  检查更新
                </el-button>
              </template>
            </div>
          </div>

          <!-- 下载进度 -->
          <el-progress
            v-if="downloading"
            :percentage="downloadProgress"
            :stroke-width="6"
            class="download-progress"
          />
        </div>

        <!-- 访问链接 -->
        <div v-if="status.running" class="access-section">
          <div class="section-title">
            <el-icon><Link /></el-icon>
            <span>访问链接</span>
          </div>
          <div class="access-url-row">
            <el-input :model-value="status.access_url" readonly class="url-input">
              <template #prepend>URL</template>
            </el-input>
            <el-button type="primary" @click="copyUrl" round>
              <el-icon><CopyDocument /></el-icon>
              复制
            </el-button>
            <el-button @click="openInBrowser" round>
              <el-icon><Position /></el-icon>
              打开
            </el-button>
          </div>
          <p class="form-tip">
            将此链接发送给远程用户，在浏览器中打开即可串流。外网需放行端口 {{ status.port }}。
          </p>
        </div>

        <!-- 配置区域 -->
        <template v-if="status.installed">
          <el-divider content-position="left">
            <el-icon><Setting /></el-icon>
            <span style="margin-left: 6px;">服务配置</span>
          </el-divider>

          <el-form :model="config" label-width="140px" class="config-form">
            <el-form-item label="绑定地址">
              <el-input v-model="config.web_server.bind_address" placeholder="0.0.0.0:8080" />
              <span class="form-tip"><code>0.0.0.0:端口</code> 监听所有网卡，<code>127.0.0.1:端口</code> 仅本机</span>
            </el-form-item>

            <el-form-item label="HTTPS 证书">
              <div class="setting-content">
                <el-switch v-model="httpsEnabled" />
                <span class="form-tip">{{ httpsEnabled ? '已启用（需要证书文件）' : '未启用（HTTP 明文）' }}</span>
              </div>
            </el-form-item>

            <template v-if="httpsEnabled">
              <el-form-item label="私钥文件">
                <el-input v-model="certKeyPath" placeholder="./server/key.pem" />
              </el-form-item>
              <el-form-item label="证书文件">
                <el-input v-model="certPemPath" placeholder="./server/cert.pem" />
              </el-form-item>
            </template>

            <el-divider content-position="left">默认串流设置</el-divider>

            <div class="two-column-layout">
              <el-form-item label="视频编码">
                <el-select v-model="defaultVideoCodec" style="width: 100%">
                  <el-option label="H.264 (兼容最好)" value="h264" />
                  <el-option label="H.265 / HEVC" value="h265" />
                  <el-option label="AV1" value="av1" />
                  <el-option label="自动" value="auto" />
                </el-select>
              </el-form-item>

              <el-form-item label="默认帧率">
                <el-select v-model="defaultFps" style="width: 100%">
                  <el-option :label="30" :value="30" />
                  <el-option :label="60" :value="60" />
                  <el-option :label="120" :value="120" />
                </el-select>
              </el-form-item>

              <el-form-item label="码率 (Kbps)">
                <el-input-number v-model="defaultBitrate" :min="1000" :max="150000" :step="1000" style="width: 100%" />
              </el-form-item>

              <el-form-item label="传输模式">
                <el-select v-model="defaultTransport" style="width: 100%">
                  <el-option label="自动 (WebRTC 优先)" value="auto" />
                  <el-option label="WebRTC" value="webrtc" />
                  <el-option label="WebSocket" value="websocket" />
                </el-select>
              </el-form-item>
            </div>

            <div class="form-actions">
              <el-form-item>
                <el-button type="primary" @click="handleSaveConfig" :loading="saving" round>
                  保存配置
                </el-button>
                <el-button @click="loadConfig" round>
                  重新加载
                </el-button>
                <span v-if="configDirty" class="unsaved-hint">⚠ 有未保存的更改</span>
              </el-form-item>
            </div>
          </el-form>
        </template>

        <!-- 安装路径 -->
        <div v-if="status.installed" class="install-path">
          <el-icon><Folder /></el-icon>
          <span>安装路径: {{ status.install_path }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Connection, Download, VideoPlay, VideoPause, Refresh,
  Link, CopyDocument, Position, Setting, Folder,
} from '@element-plus/icons-vue'
import { moonlightWeb } from '../tauri-adapter.js'

const emit = defineEmits(['close'])

// 状态
const status = reactive({
  installed: false,
  running: false,
  install_path: '',
  version: '',
  access_url: '',
  port: 8080,
})

const config = reactive({
  web_server: { bind_address: '0.0.0.0:8080' },
  webrtc: null,
  default_settings: null,
})

// UI 状态
const starting = ref(false)
const stopping = ref(false)
const downloading = ref(false)
const downloadProgress = ref(0)
const saving = ref(false)
const configDirty = ref(false)
const httpsEnabled = ref(false)
const certKeyPath = ref('./server/key.pem')
const certPemPath = ref('./server/cert.pem')
const defaultVideoCodec = ref('h264')
const defaultBitrate = ref(20000)
const defaultFps = ref(60)
const defaultTransport = ref('auto')

// 状态轮询
let pollTimer = null

// ========== 生命周期 ==========

onMounted(async () => {
  await refreshStatus()
  if (status.installed) {
    await loadConfig()
  }
  startPolling()

  // 监听下载进度事件
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen('moonlight-web-download-progress', (event) => {
      downloadProgress.value = event.payload.progress || 0
    })
    // 组件销毁时取消监听
    onUnmounted(() => {
      unlisten()
    })
  } catch (e) {
    console.warn('无法监听下载进度事件:', e)
  }
})

onUnmounted(() => {
  stopPolling()
})

// 监听配置变化标记脏状态
watch([() => config.web_server.bind_address, httpsEnabled, certKeyPath, certPemPath, defaultVideoCodec, defaultBitrate, defaultFps, defaultTransport], () => {
  configDirty.value = true
})

// ========== 方法 ==========

async function refreshStatus() {
  try {
    const s = await moonlightWeb.getStatus()
    Object.assign(status, s)
  } catch (e) {
    console.error('获取状态失败:', e)
  }
}

async function loadConfig() {
  try {
    const c = await moonlightWeb.getConfig()
    Object.assign(config, c)

    // 确保 bind_address 不为空
    if (!config.web_server?.bind_address) {
      config.web_server.bind_address = '0.0.0.0:8080'
    }

    // 解析 HTTPS 状态
    httpsEnabled.value = !!config.web_server?.certificate
    if (config.web_server?.certificate) {
      certKeyPath.value = config.web_server.certificate.private_key_pem || './server/key.pem'
      certPemPath.value = config.web_server.certificate.certificate_pem || './server/cert.pem'
    }

    // 解析默认设置
    const ds = config.default_settings || {}
    defaultVideoCodec.value = ds.videoCodec || 'h264'
    defaultBitrate.value = ds.bitrate || 20000
    defaultFps.value = ds.fps || 60
    defaultTransport.value = ds.dataTransport || 'auto'

    configDirty.value = false
  } catch (e) {
    console.error('加载配置失败:', e)
  }
}

function startPolling() {
  pollTimer = setInterval(refreshStatus, 5000)
}

function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

async function handleStart() {
  starting.value = true
  try {
    const msg = await moonlightWeb.start()
    ElMessage.success(msg)
    await refreshStatus()
  } catch (e) {
    ElMessage.error('启动失败: ' + e)
  } finally {
    starting.value = false
  }
}

async function handleStop() {
  stopping.value = true
  try {
    const msg = await moonlightWeb.stop()
    ElMessage.success(msg)
    await refreshStatus()
  } catch (e) {
    ElMessage.error('停止失败: ' + e)
  } finally {
    stopping.value = false
  }
}

async function handleInstall() {
  downloading.value = true
  downloadProgress.value = 0
  try {
    // 先检查最新版
    const release = await moonlightWeb.checkRelease()
    if (!release.download_url) {
      ElMessage.error('未找到适用于 Windows x86_64 的下载文件')
      return
    }

    await ElMessageBox.confirm(
      `即将下载 Moonlight Web ${release.version}\n文件: ${release.download_name}`,
      '确认安装',
      { confirmButtonText: '下载安装', cancelButtonText: '取消' }
    )

    await moonlightWeb.download(release.download_url, release.version)
    ElMessage.success('安装完成!')
    await refreshStatus()
    if (status.installed) {
      await loadConfig()
    }
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error('安装失败: ' + e)
    }
  } finally {
    downloading.value = false
  }
}

async function handleCheckUpdate() {
  try {
    const release = await moonlightWeb.checkRelease()
    const currentVersion = status.version?.replace(/^v/, '') || '0.0.0'
    const latestVersion = release.version?.replace(/^v/, '') || '0.0.0'

    if (latestVersion === currentVersion) {
      ElMessage.info('已是最新版本')
    } else {
      await ElMessageBox.confirm(
        `发现新版本: ${release.version}\n当前版本: ${status.version}\n\n是否下载更新？`,
        '更新可用',
        { confirmButtonText: '下载更新', cancelButtonText: '稍后' }
      )

      downloading.value = true
      downloadProgress.value = 0

      // 停止服务后再更新
      if (status.running) {
        await moonlightWeb.stop()
      }
      await moonlightWeb.download(release.download_url, release.version)
      ElMessage.success('更新完成!')
      await refreshStatus()
    }
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error('检查更新失败: ' + e)
    }
  } finally {
    downloading.value = false
  }
}

async function handleSaveConfig() {
  saving.value = true
  try {
    // 基于已加载的完整配置进行更新，保留所有上游字段
    const saveConfig = JSON.parse(JSON.stringify(config))

    // 更新用户可编辑的字段
    saveConfig.web_server.bind_address = config.web_server.bind_address

    if (httpsEnabled.value) {
      saveConfig.web_server.certificate = {
        private_key_pem: certKeyPath.value,
        certificate_pem: certPemPath.value,
      }
    } else {
      delete saveConfig.web_server.certificate
    }

    saveConfig.default_settings = {
      ...(saveConfig.default_settings || {}),
      videoCodec: defaultVideoCodec.value,
      bitrate: defaultBitrate.value,
      fps: defaultFps.value,
      dataTransport: defaultTransport.value,
    }

    await moonlightWeb.saveConfig(saveConfig)
    ElMessage.success('配置已保存。如服务正在运行，需重启生效。')
    configDirty.value = false
  } catch (e) {
    ElMessage.error('保存配置失败: ' + e)
  } finally {
    saving.value = false
  }
}

function copyUrl() {
  navigator.clipboard.writeText(status.access_url).then(() => {
    ElMessage.success('已复制到剪贴板')
  }).catch(() => {
    ElMessage.warning('复制失败，请手动复制')
  })
}

async function openInBrowser() {
  try {
    const { openExternalUrl } = await import('../tauri-adapter.js')
    await openExternalUrl(status.access_url)
  } catch (e) {
    window.open(status.access_url, '_blank')
  }
}
</script>

<style scoped lang="less">
@import '../styles/theme.less';

.webstream-settings-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

// ========== 深色模式 ==========
[data-bs-theme='dark'] {
  .webstream-header {
    border-bottom: 1px solid rgba(230, 213, 184, 0.15);
    background: linear-gradient(135deg, rgba(212, 165, 165, 0.1), rgba(230, 213, 184, 0.05));

    h2 {
      color: #e6d5b8;
      text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);

      .header-icon {
        color: @morandi-red;
      }
    }
  }

  .webstream-form {
    background: linear-gradient(135deg, rgba(61, 50, 53, 0.4), rgba(74, 63, 66, 0.3));
    border: 1px solid rgba(212, 165, 165, 0.2);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 2px 8px rgba(212, 165, 165, 0.1);

    :deep(.el-form-item__label) {
      color: #e6d5b8;
    }

    :deep(.el-input__inner),
    :deep(.el-input-number__decrease),
    :deep(.el-input-number__increase) {
      background: rgba(230, 213, 184, 0.1);
      border-color: rgba(230, 213, 184, 0.2);
      color: #e6d5b8;

      &:hover {
        border-color: rgba(230, 213, 184, 0.4);
      }

      &:focus {
        border-color: @morandi-red;
      }
    }

    :deep(.el-select__wrapper) {
      background: rgba(230, 213, 184, 0.1);
      border-color: rgba(230, 213, 184, 0.2);

      &:hover {
        border-color: rgba(230, 213, 184, 0.4);
      }

      &.is-focused {
        border-color: @morandi-red;
      }
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @morandi-red;
    }

    :deep(.el-divider__text) {
      background: transparent;
      color: rgba(230, 213, 184, 0.7);
    }

    :deep(.el-divider) {
      border-color: rgba(230, 213, 184, 0.15);
    }
  }

  .section-desc {
    color: rgba(230, 213, 184, 0.7);
  }

  .form-tip {
    color: rgba(230, 213, 184, 0.6);
  }

  .status-section {
    background: rgba(230, 213, 184, 0.05);
    border-color: rgba(212, 165, 165, 0.2);
  }

  .access-section {
    background: rgba(230, 213, 184, 0.05);
    border-color: rgba(212, 165, 165, 0.2);

    .section-title {
      color: #e6d5b8;
    }
  }

  .status-label {
    color: #e6d5b8;
  }

  .status-version {
    color: rgba(230, 213, 184, 0.6);
  }

  .install-path {
    color: rgba(230, 213, 184, 0.5);
  }

  .unsaved-hint {
    color: @morandi-yellow;
  }

  .form-actions .el-button:not(.el-button--primary):not(.el-button--danger) {
    background: rgba(212, 165, 165, 0.2);
    border-color: rgba(212, 165, 165, 0.3);
    color: #e6d5b8;

    &:hover {
      background: rgba(212, 165, 165, 0.3);
      border-color: @morandi-red;
    }
  }

  .webstream-content {
    &::-webkit-scrollbar-track {
      background: rgba(230, 213, 184, 0.05);
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(212, 165, 165, 0.3);

      &:hover {
        background: rgba(212, 165, 165, 0.5);
      }
    }
  }
}

// ========== 浅色模式 ==========
[data-bs-theme='light'] {
  .webstream-header {
    border-bottom: 1px solid rgba(74, 158, 255, 0.2);
    background: linear-gradient(135deg, rgba(74, 158, 255, 0.1), rgba(122, 184, 255, 0.05));

    h2 {
      color: #3a7ed5;
      text-shadow: 0 1px 2px rgba(74, 158, 255, 0.2);

      .header-icon {
        color: @gura-blue;
      }
    }
  }

  .webstream-form {
    background: linear-gradient(135deg, rgba(240, 248, 255, 0.8), rgba(230, 242, 255, 0.6));
    border: 1px solid rgba(74, 158, 255, 0.2);
    box-shadow: 0 8px 32px rgba(74, 158, 255, 0.15), 0 2px 8px rgba(74, 158, 255, 0.1);

    :deep(.el-form-item__label) {
      color: #3a7ed5;
    }

    :deep(.el-input__inner),
    :deep(.el-input-number__decrease),
    :deep(.el-input-number__increase) {
      background: rgba(255, 255, 255, 0.8);
      border-color: rgba(74, 158, 255, 0.3);
      color: #3a7ed5;

      &:hover {
        border-color: rgba(74, 158, 255, 0.5);
      }

      &:focus {
        border-color: @gura-blue;
      }
    }

    :deep(.el-select__wrapper) {
      background: rgba(255, 255, 255, 0.8);
      border-color: rgba(74, 158, 255, 0.3);

      &:hover {
        border-color: rgba(74, 158, 255, 0.5);
      }

      &.is-focused {
        border-color: @gura-blue;
      }
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @gura-blue;
    }

    :deep(.el-divider__text) {
      background: transparent;
      color: rgba(58, 126, 213, 0.7);
    }

    :deep(.el-divider) {
      border-color: rgba(74, 158, 255, 0.2);
    }
  }

  .section-desc {
    color: rgba(58, 126, 213, 0.7);
  }

  .form-tip {
    color: rgba(58, 126, 213, 0.6);
  }

  .status-section {
    background: rgba(74, 158, 255, 0.05);
    border-color: rgba(74, 158, 255, 0.2);
  }

  .access-section {
    background: rgba(74, 158, 255, 0.05);
    border-color: rgba(74, 158, 255, 0.2);

    .section-title {
      color: #3a7ed5;
    }
  }

  .status-label {
    color: #3a7ed5;
  }

  .status-version {
    color: rgba(58, 126, 213, 0.6);
  }

  .install-path {
    color: rgba(58, 126, 213, 0.5);
  }

  .unsaved-hint {
    color: #e6a23c;
  }

  .form-actions .el-button:not(.el-button--primary):not(.el-button--danger) {
    background: rgba(74, 158, 255, 0.1);
    border-color: rgba(74, 158, 255, 0.3);
    color: #3a7ed5;

    &:hover {
      background: rgba(74, 158, 255, 0.2);
      border-color: @gura-blue;
    }
  }

  .webstream-content {
    &::-webkit-scrollbar-track {
      background: rgba(74, 158, 255, 0.05);
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(74, 158, 255, 0.3);

      &:hover {
        background: rgba(74, 158, 255, 0.5);
      }
    }
  }
}

// ========== 通用样式 ==========
.webstream-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 24px 32px;
  transition: all 0.3s ease;

  h2 {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0;
    font-size: 24px;
    font-weight: 600;
    transition: all 0.3s ease;

    .header-icon {
      font-size: 28px;
      transition: all 0.3s ease;
    }
  }
}

.webstream-content {
  flex: 1;
  overflow-y: auto;
  padding: 32px;

  &::-webkit-scrollbar {
    width: 8px;
  }
}

.webstream-form {
  max-width: 800px;
  margin: 0 auto;
  padding: 32px;
  border-radius: 16px;
  backdrop-filter: blur(10px);
  transition: all 0.3s ease;

  :deep(.el-form-item__label) {
    font-weight: 600;
    font-size: 14px;
  }

  :deep(.el-select__wrapper) {
    box-shadow: none;
  }
}

.section-desc {
  margin: 0 0 20px 0;
  font-size: 14px;
  line-height: 1.6;
  transition: all 0.3s ease;
}

.status-section {
  padding: 16px 20px;
  border-radius: 12px;
  border: 1px solid;
  margin-bottom: 24px;
  transition: all 0.3s ease;
}

.status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}

.status-info {
  display: flex;
  align-items: center;
  gap: 10px;
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
  flex-shrink: 0;

  &.dot-running {
    background-color: #67c23a;
    box-shadow: 0 0 8px rgba(103, 194, 58, 0.6);
    animation: pulse 2s infinite;
  }

  &.dot-stopped {
    background-color: #909399;
  }
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.status-label {
  font-size: 15px;
  font-weight: 600;
  transition: all 0.3s ease;
}

.status-version {
  font-size: 12px;
  font-style: italic;
  transition: all 0.3s ease;
}

.status-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.download-progress {
  margin-top: 12px;
}

.access-section {
  padding: 16px 20px;
  border-radius: 12px;
  border: 1px solid;
  margin-bottom: 24px;
  transition: all 0.3s ease;

  .section-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 15px;
    font-weight: 600;
    margin-bottom: 12px;
    transition: all 0.3s ease;
  }
}

.access-url-row {
  display: flex;
  gap: 8px;
  align-items: center;

  .url-input {
    flex: 1;
  }
}

.form-tip {
  font-size: 12px;
  font-style: italic;
  margin-top: 4px;
  transition: all 0.3s ease;

  code {
    font-style: normal;
    padding: 1px 4px;
    border-radius: 3px;
    font-family: monospace;
  }
}

.setting-content {
  display: flex;
  align-items: center;
  gap: 10px;
}

.two-column-layout {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
  margin-bottom: 20px;

  :deep(.el-form-item) {
    margin-bottom: 0;
  }
}

.form-actions {
  margin-top: 32px;
  text-align: center;

  :deep(.el-form-item__content) {
    justify-content: center;
    gap: 16px;
  }
}

// 主按钮主题样式
[data-bs-theme='dark'] .form-actions .el-button.el-button--primary {
  background: linear-gradient(135deg, @morandi-red, @morandi-yellow);
  border: none;
  color: #2d2628;
  box-shadow: 0 4px 16px rgba(212, 165, 165, 0.4);

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(212, 165, 165, 0.6);
  }
}

[data-bs-theme='light'] .form-actions .el-button.el-button--primary {
  background: linear-gradient(135deg, @gura-blue, @gura-light-blue);
  border: none;
  color: white;
  box-shadow: 0 4px 16px rgba(74, 158, 255, 0.4);

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(74, 158, 255, 0.6);
  }
}

.form-actions .el-button {
  min-width: 120px;
  font-weight: 600;
  border-radius: 12px;
  transition: all 0.3s ease;

  &:active {
    transform: translateY(0);
  }
}

.unsaved-hint {
  font-size: 13px;
  font-weight: 500;
  transition: all 0.3s ease;
}

.install-path {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  padding: 16px 0 0;
  transition: all 0.3s ease;
}
</style>
