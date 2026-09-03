<template>
  <div class="webstream-settings-wrapper">
    <div class="webstream-header">
      <h2>
        <el-icon class="header-icon"><Connection /></el-icon>
        {{ t.webStream.title }}
      </h2>
    </div>

    <div class="webstream-content">
      <div class="webstream-form">
        <!-- 描述 -->
        <p class="section-desc">
          {{ t.webStream.desc }}
          <el-link type="primary" href="https://github.com/MrCreativ3001/moonlight-web-stream" target="_blank">
            Moonlight Web
          </el-link>
        </p>

        <!-- 状态区域 -->
        <div class="status-section">
          <div class="status-row">
            <div class="status-info">
              <span class="status-dot" :class="status.running ? 'dot-running' : 'dot-stopped'" />
              <span class="status-label">{{ status.running ? t.webStream.statusRunning : (status.installed ? t.webStream.statusStopped : t.webStream.statusNotInstalled) }}</span>
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
                {{ t.webStream.install }}
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
                  {{ t.webStream.start }}
                </el-button>
                <el-button
                  v-else
                  type="danger"
                  :loading="stopping"
                  @click="handleStop"
                  round
                >
                  <el-icon><VideoPause /></el-icon>
                  {{ t.webStream.stop }}
                </el-button>
                <el-button @click="handleCheckUpdate" round>
                  <el-icon><Refresh /></el-icon>
                  {{ t.webStream.checkUpdate }}
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
            <span>{{ t.webStream.accessLink }}</span>
          </div>
          <div class="access-url-row">
            <el-input :model-value="status.access_url" readonly class="url-input">
              <template #prepend>URL</template>
            </el-input>
            <el-button type="primary" @click="copyUrl" round>
              <el-icon><CopyDocument /></el-icon>
              {{ t.webStream.copy }}
            </el-button>
            <el-button @click="openInBrowser" round>
              <el-icon><Position /></el-icon>
              {{ t.webStream.open }}
            </el-button>
          </div>
          <p class="form-tip">
            {{ t.webStream.accessTip.replace('{port}', status.port) }}
          </p>
        </div>

        <!-- 配置区域 -->
        <template v-if="status.installed">
          <div class="section-divider">
            <span class="section-divider-text">
              <el-icon><Setting /></el-icon>
              <span>{{ t.webStream.serviceConfig }}</span>
            </span>
          </div>

          <el-form :model="config" label-width="140px" class="config-form">
            <el-form-item :label="t.webStream.bindAddress">
              <el-input v-model="config.web_server.bind_address" placeholder="0.0.0.0:8080" />
              <span class="form-tip" v-html="t.webStream.bindAddressTip.replace('{allIp}', '<code>0.0.0.0:port</code>').replace('{localhost}', '<code>127.0.0.1:port</code>')"></span>
            </el-form-item>

            <el-form-item :label="t.webStream.httpsCert">
              <div class="setting-content">
                <el-switch v-model="httpsEnabled" />
                <span class="form-tip">{{ httpsEnabled ? t.webStream.httpsEnabled : t.webStream.httpsDisabled }}</span>
              </div>
            </el-form-item>

            <template v-if="httpsEnabled">
              <el-form-item :label="t.webStream.privateKey">
                <el-input v-model="certKeyPath" placeholder="./server/key.pem" />
              </el-form-item>
              <el-form-item :label="t.webStream.certFile">
                <el-input v-model="certPemPath" placeholder="./server/cert.pem" />
              </el-form-item>
              <el-form-item>
                <el-button @click="handleGenerateCert" :loading="generatingCert" round>
                  <el-icon><Key /></el-icon>
                  {{ t.webStream.generateCert }}
                </el-button>
                <span class="form-tip">{{ t.webStream.generateCertTip }}</span>
              </el-form-item>
            </template>

            <div class="section-divider">
              <span class="section-divider-text">{{ t.webStream.defaultStreamSettings }}</span>
            </div>

            <div class="two-column-layout">
              <el-form-item :label="t.webStream.videoCodec">
                <el-select v-model="defaultVideoCodec" style="width: 100%">
                  <el-option :label="t.webStream.h264Label" value="h264" />
                  <el-option label="H.265 / HEVC" value="h265" />
                  <el-option label="AV1" value="av1" />
                  <el-option :label="t.webStream.autoLabel" value="auto" />
                </el-select>
              </el-form-item>

              <el-form-item :label="t.webStream.defaultFps">
                <el-select v-model="defaultFps" style="width: 100%">
                  <el-option :label="30" :value="30" />
                  <el-option :label="60" :value="60" />
                  <el-option :label="120" :value="120" />
                </el-select>
              </el-form-item>

              <el-form-item :label="t.webStream.bitrate">
                <el-input-number v-model="defaultBitrate" :min="1000" :max="150000" :step="1000" style="width: 100%" />
              </el-form-item>

              <el-form-item :label="t.webStream.transportMode">
                <el-select v-model="defaultTransport" style="width: 100%">
                  <el-option :label="t.webStream.transportAuto" value="auto" />
                  <el-option label="WebRTC" value="webrtc" />
                  <el-option label="WebSocket" value="websocket" />
                </el-select>
              </el-form-item>
            </div>

            <div class="form-actions">
              <el-form-item>
                <el-button type="primary" @click="handleSaveConfig" :loading="saving" round>
                  {{ t.webStream.saveConfig }}
                </el-button>
                <el-button @click="loadConfig" round>
                  {{ t.webStream.reload }}
                </el-button>
                <span v-if="configDirty" class="unsaved-hint">{{ t.webStream.unsavedChanges }}</span>
              </el-form-item>
            </div>
          </el-form>
        </template>

        <!-- 安装路径 -->
        <div v-if="status.installed" class="install-path">
          <el-icon><Folder /></el-icon>
          <span>{{ t.webStream.installPath.replace('{path}', status.install_path) }}</span>
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
  Link, CopyDocument, Position, Setting, Folder, Key,
} from '@element-plus/icons-vue'
import { moonlightWeb } from '../tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

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
const generatingCert = ref(false)
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
    console.warn('Cannot listen to download progress event:', e)
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
    console.error('Failed to get status:', e)
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
    console.error('Failed to load config:', e)
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
    ElMessage.error(t.value.webStream.startFailed.replace('{error}', e))
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
    ElMessage.error(t.value.webStream.stopFailed.replace('{error}', e))
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
      ElMessage.error(t.value.webStream.noWindowsDownload)
      return
    }

    await ElMessageBox.confirm(
      t.value.webStream.downloadConfirm.replace('{version}', release.version).replace('{filename}', release.download_name),
      t.value.webStream.installTitle,
      { confirmButtonText: t.value.webStream.downloadInstall, cancelButtonText: t.value.webStream.cancelBtn }
    )

    await moonlightWeb.download(release.download_url, release.version)
    ElMessage.success(t.value.webStream.installComplete)
    await refreshStatus()
    if (status.installed) {
      await loadConfig()
    }
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error(t.value.webStream.installFailed.replace('{error}', e))
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
      ElMessage.info(t.value.webStream.alreadyLatest)
    } else {
      await ElMessageBox.confirm(
        t.value.webStream.newVersionFound.replace('{version}', release.version).replace('{current}', status.version),
        t.value.webStream.updateAvailable,
        { confirmButtonText: t.value.webStream.downloadUpdate, cancelButtonText: t.value.webStream.laterBtn }
      )

      downloading.value = true
      downloadProgress.value = 0

      // 停止服务后再更新
      if (status.running) {
        await moonlightWeb.stop()
      }
      await moonlightWeb.download(release.download_url, release.version)
      ElMessage.success(t.value.webStream.updateComplete)
      await refreshStatus()
    }
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error(t.value.webStream.checkUpdateFailed.replace('{error}', e))
    }
  } finally {
    downloading.value = false
  }
}

async function handleGenerateCert() {
  generatingCert.value = true
  try {
    const result = await moonlightWeb.generateCert()
    certKeyPath.value = result.private_key_pem
    certPemPath.value = result.certificate_pem
    // 生成后自动保存配置
    await handleSaveConfig()
    // 如果服务正在运行，提示重启
    if (status.running) {
      try {
        await ElMessageBox.confirm(
          t.value.webStream.certGenRestart,
          t.value.webStream.certTitle,
          { confirmButtonText: t.value.webStream.restartService, cancelButtonText: t.value.webStream.laterManualRestart }
        )
        await moonlightWeb.stop()
        await new Promise(resolve => setTimeout(resolve, 500))
        await moonlightWeb.start()
        await refreshStatus()
        ElMessage.success(t.value.webStream.serviceRestarted)
      } catch {
        ElMessage.info(t.value.webStream.manualRestartHint)
      }
    } else {
      ElMessage.success(t.value.webStream.certGenSuccess)
    }
  } catch (e) {
    ElMessage.error(t.value.webStream.certGenFailed.replace('{error}', e))
  } finally {
    generatingCert.value = false
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
      saveConfig.web_server.session_cookie_secure = true
    } else {
      delete saveConfig.web_server.certificate
      saveConfig.web_server.session_cookie_secure = false
    }

    saveConfig.default_settings = {
      ...(saveConfig.default_settings || {}),
      videoCodec: defaultVideoCodec.value,
      bitrate: defaultBitrate.value,
      fps: defaultFps.value,
      dataTransport: defaultTransport.value,
    }

    await moonlightWeb.saveConfig(saveConfig)
    ElMessage.success(t.value.webStream.configSaved)
    configDirty.value = false
  } catch (e) {
    ElMessage.error(t.value.webStream.configSaveFailed.replace('{error}', e))
  } finally {
    saving.value = false
  }
}

function copyUrl() {
  navigator.clipboard.writeText(status.access_url).then(() => {
    ElMessage.success(t.value.webStream.copied)
  }).catch(() => {
    ElMessage.warning(t.value.webStream.copyFailed)
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
  }

  .section-divider {
    .section-divider-text {
      color: rgba(230, 213, 184, 0.7);
    }

    &::after {
      background: rgba(230, 213, 184, 0.15);
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
  }

  .section-divider {
    .section-divider-text {
      color: rgba(58, 126, 213, 0.7);
    }

    &::after {
      background: rgba(74, 158, 255, 0.2);
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

.section-divider {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 24px 0 16px;

  .section-divider-text {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    color: var(--el-text-color-secondary);
  }

  &::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--el-border-color-lighter);
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
