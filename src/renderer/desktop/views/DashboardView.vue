<template>
  <div class="dashboard-view">
    <!-- 欢迎横幅 -->
    <div class="welcome-banner fade-in">
      <div class="banner-content">
        <h1 class="banner-title">
          <span class="gradient-text">Foundation</span> Desktop
        </h1>
        <p class="banner-subtitle">{{ t.dashboard.subtitle }}</p>
      </div>
      <div class="banner-decoration">
        <div class="decoration-circle"></div>
        <div class="decoration-circle delay"></div>
      </div>
    </div>

    <!-- 状态卡片 -->
    <div class="desktop-grid cols-4">
      <div class="desktop-card stat-card fade-in delay-1">
        <div class="stat-icon online">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 12h-4l-3 9L9 3l-3 9H2"/>
          </svg>
        </div>
        <div class="stat-info">
          <span class="stat-label">{{ t.dashboard.serviceStatus }}</span>
          <span class="stat-value-text" :class="serviceStatus.class">{{ serviceStatus.text }}</span>
        </div>
      </div>

      <div class="desktop-card stat-card fade-in delay-2">
        <div class="stat-icon cyan">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="3" width="20" height="14" rx="2"/>
            <line x1="8" y1="21" x2="16" y2="21"/>
            <line x1="12" y1="17" x2="12" y2="21"/>
          </svg>
        </div>
        <div class="stat-info">
          <span class="stat-label">{{ t.dashboard.pairedDevices }}</span>
          <span class="stat-value">{{ pairedDevices }}</span>
        </div>
      </div>

      <div class="desktop-card stat-card fade-in delay-3">
        <div class="stat-icon magenta">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="5 3 19 12 5 21 5 3"/>
          </svg>
        </div>
        <div class="stat-info">
          <span class="stat-label">{{ t.dashboard.activeSessions }}</span>
          <span class="stat-value">{{ activeSessions }}</span>
        </div>
      </div>

      <div class="desktop-card stat-card fade-in delay-4">
        <div class="stat-icon yellow">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="12 6 12 12 16 14"/>
          </svg>
        </div>
        <div class="stat-info">
          <span class="stat-label">{{ t.dashboard.uptime }}</span>
          <span class="stat-value-text">{{ uptime }}</span>
        </div>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="section-title fade-in">
      <span class="title-icon">⚡</span>
      {{ t.dashboard.quickActions }}
    </div>

    <div class="desktop-grid cols-3">
      <div class="desktop-card action-card fade-in" tabindex="0" @click="openWebUI" @keydown.enter="openWebUI">
        <div class="action-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <line x1="2" y1="12" x2="22" y2="12"/>
            <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
          </svg>
        </div>
        <div class="action-info">
          <span class="action-title">{{ t.dashboard.webConsole }}</span>
          <span class="action-desc">{{ t.dashboard.webConsoleDesc }}</span>
        </div>
        <div class="action-arrow">→</div>
      </div>

      <div class="desktop-card action-card fade-in" tabindex="0" @click="restartService" @keydown.enter="restartService">
        <div class="action-icon warning">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/>
          </svg>
        </div>
        <div class="action-info">
          <span class="action-title">{{ t.dashboard.restartService }}</span>
          <span class="action-desc">{{ t.dashboard.restartServiceDesc }}</span>
        </div>
        <div class="action-arrow">→</div>
      </div>

      <div class="desktop-card action-card fade-in" tabindex="0" @click="openLogs" @keydown.enter="openLogs">
        <div class="action-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
            <line x1="16" y1="13" x2="8" y2="13"/>
            <line x1="16" y1="17" x2="8" y2="17"/>
            <polyline points="10 9 9 9 8 9"/>
          </svg>
        </div>
        <div class="action-info">
          <span class="action-title">{{ t.dashboard.viewLogs }}</span>
          <span class="action-desc">{{ t.dashboard.viewLogsDesc }}</span>
        </div>
        <div class="action-arrow">→</div>
      </div>
    </div>

    <!-- 系统信息 -->
    <div class="section-title fade-in">
      <span class="title-icon">💻</span>
      {{ t.dashboard.systemInfo }}
    </div>

    <div class="desktop-card system-info-card fade-in">
      <div class="info-grid">
        <div class="info-item">
          <span class="info-label">{{ t.dashboard.sunshineVersion }}</span>
          <span class="info-value">{{ systemInfo.sunshineVersion }}</span>
        </div>
        <div class="info-item">
          <span class="info-label">{{ t.dashboard.os }}</span>
          <span class="info-value">{{ systemInfo.os }}</span>
        </div>
        <div class="info-item">
          <span class="info-label">{{ t.dashboard.gpu }}</span>
          <span class="info-value">{{ systemInfo.gpu }}</span>
        </div>
        <div class="info-item">
          <span class="info-label">{{ t.dashboard.encoder }}</span>
          <span class="info-value">{{ systemInfo.encoder }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

// Tauri 命令 - 使用 ref 存储
const invoke = ref(null)
const proxyUrl = ref('http://localhost:48081')

// 状态数据
const serviceStatus = ref({ text: '', class: 'connecting' })
const pairedDevices = ref(0)
const activeSessions = ref(0)
const uptime = ref('--:--:--')

const systemInfo = ref({
  sunshineVersion: '...',
  os: 'Windows',
  gpu: '...',
  encoder: '...'
})

// 计时器
let uptimeInterval = null
let startTime = Date.now()

function updateUptime() {
  const elapsed = Math.floor((Date.now() - startTime) / 1000)
  const hours = Math.floor(elapsed / 3600).toString().padStart(2, '0')
  const minutes = Math.floor((elapsed % 3600) / 60).toString().padStart(2, '0')
  const seconds = (elapsed % 60).toString().padStart(2, '0')
  uptime.value = `${hours}:${minutes}:${seconds}`
}

async function loadSystemInfo() {
  if (!invoke.value) return

  try {
    // 获取 Sunshine 版本
    const version = await invoke.value('get_sunshine_version')
    systemInfo.value.sunshineVersion = version || t.value.dashboard.status.unknown

    // 获取 GPU 信息（get_gpus 返回 Vec<String>，即 GPU 名称列表）
    const gpus = await invoke.value('get_gpus')
    if (gpus && gpus.length > 0) {
      systemInfo.value.gpu = gpus[0]
      // 根据 GPU 名称推断编码器
      const gpuLower = gpus[0].toLowerCase()
      systemInfo.value.encoder = gpuLower.includes('nvidia') || gpuLower.includes('geforce') ? 'NVENC' :
                                  gpuLower.includes('amd') || gpuLower.includes('radeon') ? 'AMF' :
                                  gpuLower.includes('intel') ? 'QuickSync' : 'Software'
    }

    // 获取活动会话
    const sessions = await invoke.value('get_active_sessions')
    activeSessions.value = sessions?.length || 0

    // 从 API 获取已配对设备数量
    try {
      const resp = await fetch(`${proxyUrl.value}/api/clients/list`)
      const data = await resp.json()
      if (data.status?.toString() === 'true' && data.named_certs) {
        pairedDevices.value = data.named_certs.length
      }
    } catch (_) { /* proxy not available */ }

    serviceStatus.value = { text: t.value.dashboard.status.online, class: 'online' }
  } catch (e) {
    console.error('Failed to load system info:', e)
    serviceStatus.value = { text: t.value.dashboard.status.offline, class: 'offline' }
  }
}

// 操作函数
async function openWebUI() {
  if (invoke.value) {
    try {
      const url = await invoke.value('get_sunshine_url')
      await invoke.value('open_external_url', { url })
    } catch (e) {
      console.error('Failed to open web UI:', e)
    }
  }
}

async function restartService() {
  if (invoke.value) {
    try {
      await invoke.value('restart_sunshine_service')
      serviceStatus.value = { text: t.value.dashboard.status.restarting, class: 'connecting' }
      setTimeout(() => {
        serviceStatus.value = { text: t.value.dashboard.status.online, class: 'online' }
      }, 3000)
    } catch (e) {
      console.error('Failed to restart service:', e)
    }
  }
}

async function openLogs() {
  if (invoke.value) {
    try {
      await invoke.value('open_tool_window', { toolName: 'logs' })
    } catch (e) {
      console.error('Failed to open logs window:', e)
    }
  }
}

onMounted(async () => {
  // 动态导入 Tauri API
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
    const url = await invoke.value('get_proxy_url_command')
    if (url) proxyUrl.value = url
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }

  updateUptime()
  uptimeInterval = setInterval(updateUptime, 5000)
  loadSystemInfo()
})

onUnmounted(() => {
  if (uptimeInterval) {
    clearInterval(uptimeInterval)
  }
})
</script>

<style lang="less" scoped>
.dashboard-view {
  max-width: 1600px;
  margin: 0 auto;
}

.welcome-banner {
  background: linear-gradient(135deg, rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1) 0%, rgba(255, 0, 255, 0.1) 100%);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 24px;
  padding: 56px;
  margin-bottom: 40px;
  position: relative;
  overflow: hidden;

  .banner-content {
    position: relative;
    z-index: 1;
  }

  .banner-title {
    font-size: 56px;
    font-weight: 700;
    margin: 0 0 12px 0;
    color: var(--fd-text-primary, #fff);

    .gradient-text {
      background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
    }
  }

  .banner-subtitle {
    font-size: 22px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.7);
    margin: 0;
  }

  .banner-decoration {
    position: absolute;
    right: 40px;
    top: 50%;
    transform: translateY(-50%);

    .decoration-circle {
      width: 200px;
      height: 200px;
      border: 2px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
      border-radius: 50%;
      position: absolute;
      right: 0;
      animation: pulse-ring 3s infinite;

      &.delay {
        animation-delay: 1.5s;
      }
    }
  }
}

@keyframes pulse-ring {
  0% {
    transform: scale(0.8);
    opacity: 1;
  }
  100% {
    transform: scale(1.5);
    opacity: 0;
  }
}

.stat-card {
  display: flex;
  align-items: center;
  gap: 20px;

  .stat-icon {
    width: 64px;
    height: 64px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.1);
    color: var(--fd-status-success, #00ff88);

    &.online { background: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.1); color: var(--fd-status-success, #00ff88); }
    &.cyan { background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1); color: var(--fd-accent, #00fff5); }
    &.magenta { background: rgba(var(--fd-accent-secondary-rgb, 255, 0, 255), 0.1); color: var(--fd-accent-secondary, #ff00ff); }
    &.yellow { background: rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.1); color: var(--fd-status-warning, #ffd700); }

    svg {
      width: 32px;
      height: 32px;
    }
  }

  .stat-info {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .stat-label {
    font-size: 15px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  }

  .stat-value {
    font-size: 42px;
    font-weight: 700;
    background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .stat-value-text {
    font-size: 20px;
    font-weight: 600;

    &.online { color: var(--fd-status-success, #00ff88); }
    &.offline { color: var(--fd-status-danger, #ff6b35); }
    &.connecting { color: var(--fd-status-warning, #ffd700); }
  }
}

.section-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--fd-text-primary, #fff);
  margin: 40px 0 20px 0;
  display: flex;
  align-items: center;
  gap: 10px;

  .title-icon {
    font-size: 28px;
  }
}

.action-card {
  display: flex;
  align-items: center;
  gap: 20px;
  cursor: pointer;
  transition: all 0.3s ease;

  &:hover {
    transform: translateY(-2px);

    .action-arrow {
      transform: translateX(6px);
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
    }
  }

  .action-icon {
    width: 60px;
    height: 60px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    color: var(--fd-accent, #00fff5);

    &.warning {
      background: rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.1);
      color: var(--fd-status-warning, #ffd700);
    }

    svg {
      width: 28px;
      height: 28px;
    }
  }

  .action-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .action-title {
    font-size: 20px;
    font-weight: 600;
    color: var(--fd-text-primary, #fff);
  }

  .action-desc {
    font-size: 15px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  }

  .action-arrow {
    font-size: 24px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
    transition: all 0.3s ease;
  }
}

.system-info-card {
  .info-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 32px;

    @media (max-width: 1000px) {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .info-item {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .info-label {
    font-size: 14px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .info-value {
    font-size: 18px;
    color: var(--fd-text-primary, #fff);
    font-weight: 500;
  }
}
</style>

