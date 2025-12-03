<template>
  <div class="dashboard-view">
    <!-- 欢迎横幅 -->
    <div class="welcome-banner fade-in">
      <div class="banner-content">
        <h1 class="banner-title">
          <span class="gradient-text">Sunshine</span> Desktop
        </h1>
        <p class="banner-subtitle">高性能游戏串流解决方案</p>
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
          <span class="stat-label">服务状态</span>
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
          <span class="stat-label">已配对设备</span>
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
          <span class="stat-label">活动会话</span>
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
          <span class="stat-label">运行时间</span>
          <span class="stat-value-text">{{ uptime }}</span>
        </div>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="section-title fade-in">
      <span class="title-icon">⚡</span>
      快捷操作
    </div>

    <div class="desktop-grid cols-3">
      <div class="desktop-card action-card fade-in" @click="openWebUI">
        <div class="action-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <line x1="2" y1="12" x2="22" y2="12"/>
            <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
          </svg>
        </div>
        <div class="action-info">
          <span class="action-title">Web 控制台</span>
          <span class="action-desc">打开 Sunshine Web 管理界面</span>
        </div>
        <div class="action-arrow">→</div>
      </div>

      <div class="desktop-card action-card fade-in" @click="restartService">
        <div class="action-icon warning">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/>
          </svg>
        </div>
        <div class="action-info">
          <span class="action-title">重启服务</span>
          <span class="action-desc">重新启动 Sunshine 服务</span>
        </div>
        <div class="action-arrow">→</div>
      </div>

      <div class="desktop-card action-card fade-in" @click="openLogs">
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
          <span class="action-title">查看日志</span>
          <span class="action-desc">打开日志控制台</span>
        </div>
        <div class="action-arrow">→</div>
      </div>
    </div>

    <!-- 系统信息 -->
    <div class="section-title fade-in">
      <span class="title-icon">💻</span>
      系统信息
    </div>

    <div class="desktop-card system-info-card fade-in">
      <div class="info-grid">
        <div class="info-item">
          <span class="info-label">Sunshine 版本</span>
          <span class="info-value">{{ systemInfo.sunshineVersion }}</span>
        </div>
        <div class="info-item">
          <span class="info-label">操作系统</span>
          <span class="info-value">{{ systemInfo.os }}</span>
        </div>
        <div class="info-item">
          <span class="info-label">显卡</span>
          <span class="info-value">{{ systemInfo.gpu }}</span>
        </div>
        <div class="info-item">
          <span class="info-label">编码器</span>
          <span class="info-value">{{ systemInfo.encoder }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'

// Tauri 命令 - 使用 ref 存储
const invoke = ref(null)

// 状态数据
const serviceStatus = ref({ text: '检测中...', class: 'connecting' })
const pairedDevices = ref(0)
const activeSessions = ref(0)
const uptime = ref('--:--:--')

const systemInfo = ref({
  sunshineVersion: '加载中...',
  os: 'Windows',
  gpu: '加载中...',
  encoder: '加载中...'
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
    systemInfo.value.sunshineVersion = version || '未知'

    // 获取 GPU 信息
    const gpus = await invoke.value('get_gpus')
    if (gpus && gpus.length > 0) {
      systemInfo.value.gpu = gpus[0].name || '未知'
      systemInfo.value.encoder = gpus[0].vendor === 'NVIDIA' ? 'NVENC' : 
                                  gpus[0].vendor === 'AMD' ? 'AMF' : 'Software'
    }

    // 获取活动会话
    const sessions = await invoke.value('get_active_sessions')
    activeSessions.value = sessions?.length || 0

    // 模拟服务状态
    serviceStatus.value = { text: '运行中', class: 'online' }
    pairedDevices.value = 2 // 这里应该从实际 API 获取
  } catch (e) {
    console.error('Failed to load system info:', e)
    serviceStatus.value = { text: '离线', class: 'offline' }
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
      serviceStatus.value = { text: '重启中...', class: 'connecting' }
      setTimeout(() => {
        serviceStatus.value = { text: '运行中', class: 'online' }
      }, 3000)
    } catch (e) {
      console.error('Failed to restart service:', e)
    }
  }
}

function openLogs() {
  // TODO: 打开日志窗口
  console.log('Open logs')
}

onMounted(async () => {
  // 动态导入 Tauri API
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }

  updateUptime()
  uptimeInterval = setInterval(updateUptime, 1000)
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
  max-width: 1400px;
  margin: 0 auto;
}

.welcome-banner {
  background: linear-gradient(135deg, rgba(0, 255, 245, 0.1) 0%, rgba(255, 0, 255, 0.1) 100%);
  border: 1px solid rgba(0, 255, 245, 0.2);
  border-radius: 20px;
  padding: 40px;
  margin-bottom: 32px;
  position: relative;
  overflow: hidden;

  .banner-content {
    position: relative;
    z-index: 1;
  }

  .banner-title {
    font-size: 48px;
    font-weight: 700;
    margin: 0 0 8px 0;
    color: white;

    .gradient-text {
      background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
    }
  }

  .banner-subtitle {
    font-size: 18px;
    color: rgba(255, 255, 255, 0.7);
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
      border: 2px solid rgba(0, 255, 245, 0.3);
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
  gap: 16px;

  .stat-icon {
    width: 56px;
    height: 56px;
    border-radius: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 255, 136, 0.1);
    color: #00ff88;

    &.online { background: rgba(0, 255, 136, 0.1); color: #00ff88; }
    &.cyan { background: rgba(0, 255, 245, 0.1); color: #00fff5; }
    &.magenta { background: rgba(255, 0, 255, 0.1); color: #ff00ff; }
    &.yellow { background: rgba(255, 215, 0, 0.1); color: #ffd700; }

    svg {
      width: 28px;
      height: 28px;
    }
  }

  .stat-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stat-label {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
  }

  .stat-value {
    font-size: 28px;
    font-weight: 700;
    background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .stat-value-text {
    font-size: 16px;
    font-weight: 600;

    &.online { color: #00ff88; }
    &.offline { color: #ff6b35; }
    &.connecting { color: #ffd700; }
  }
}

.section-title {
  font-size: 20px;
  font-weight: 600;
  color: white;
  margin: 32px 0 16px 0;
  display: flex;
  align-items: center;
  gap: 8px;

  .title-icon {
    font-size: 24px;
  }
}

.action-card {
  display: flex;
  align-items: center;
  gap: 16px;
  cursor: pointer;
  transition: all 0.3s ease;

  &:hover {
    transform: translateY(-4px);

    .action-arrow {
      transform: translateX(4px);
      color: #00fff5;
    }
  }

  .action-icon {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 255, 245, 0.1);
    color: #00fff5;

    &.warning {
      background: rgba(255, 215, 0, 0.1);
      color: #ffd700;
    }

    svg {
      width: 24px;
      height: 24px;
    }
  }

  .action-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .action-title {
    font-size: 16px;
    font-weight: 600;
    color: white;
  }

  .action-desc {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
  }

  .action-arrow {
    font-size: 20px;
    color: rgba(255, 255, 255, 0.3);
    transition: all 0.3s ease;
  }
}

.system-info-card {
  .info-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 24px;

    @media (max-width: 1000px) {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .info-item {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .info-label {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .info-value {
    font-size: 15px;
    color: white;
    font-weight: 500;
  }
}
</style>

