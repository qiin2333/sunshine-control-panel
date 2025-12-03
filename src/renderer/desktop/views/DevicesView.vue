<template>
  <div class="devices-view">
    <div class="page-header fade-in">
      <h1 class="page-title">设备管理</h1>
      <p class="page-subtitle">管理已配对的 Moonlight 客户端设备</p>
    </div>

    <div class="devices-list">
      <div 
        v-for="(device, index) in devices" 
        :key="device.id"
        class="desktop-card device-card fade-in"
        :style="{ animationDelay: `${index * 0.1}s` }"
      >
        <div class="device-icon" :class="device.type">
          <component :is="getDeviceIcon(device.type)" />
        </div>
        <div class="device-info">
          <div class="device-name">{{ device.name }}</div>
          <div class="device-meta">
            <span class="device-type">{{ device.typeLabel }}</span>
            <span class="device-separator">•</span>
            <span class="device-last-seen">{{ device.lastSeen }}</span>
          </div>
        </div>
        <div class="device-status">
          <span class="status-indicator" :class="device.status"></span>
          <span class="status-text">{{ device.statusText }}</span>
        </div>
        <div class="device-actions">
          <button class="desktop-btn" @click="unpairDevice(device)">
            取消配对
          </button>
        </div>
      </div>

      <div v-if="devices.length === 0" class="empty-state fade-in">
        <div class="empty-icon">📱</div>
        <h3>暂无已配对设备</h3>
        <p>在 Moonlight 客户端中输入配对码来添加设备</p>
      </div>
    </div>

    <!-- 配对信息卡片 -->
    <div class="desktop-card pairing-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">🔗</span>
          配对新设备
        </div>
      </div>
      <div class="card-content">
        <div class="pairing-steps">
          <div class="step">
            <div class="step-number">1</div>
            <div class="step-content">
              <div class="step-title">打开 Moonlight</div>
              <div class="step-desc">在你的设备上启动 Moonlight 客户端</div>
            </div>
          </div>
          <div class="step">
            <div class="step-number">2</div>
            <div class="step-content">
              <div class="step-title">添加电脑</div>
              <div class="step-desc">点击添加按钮，输入此电脑的 IP 地址</div>
            </div>
          </div>
          <div class="step">
            <div class="step-number">3</div>
            <div class="step-content">
              <div class="step-title">输入 PIN 码</div>
              <div class="step-desc">在 Moonlight 中输入 Web 界面显示的配对码</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'

// 模拟设备数据
const devices = ref([
  {
    id: 1,
    name: 'iPhone 15 Pro',
    type: 'phone',
    typeLabel: 'iOS',
    lastSeen: '刚刚',
    status: 'online',
    statusText: '在线'
  },
  {
    id: 2,
    name: 'Steam Deck',
    type: 'handheld',
    typeLabel: 'Linux',
    lastSeen: '2小时前',
    status: 'offline',
    statusText: '离线'
  },
  {
    id: 3,
    name: 'Android TV',
    type: 'tv',
    typeLabel: 'Android TV',
    lastSeen: '昨天',
    status: 'offline',
    statusText: '离线'
  }
])

function getDeviceIcon(type) {
  // 返回对应图标组件（这里用占位符）
  return 'div'
}

function unpairDevice(device) {
  if (confirm(`确定要取消与 "${device.name}" 的配对吗？`)) {
    devices.value = devices.value.filter(d => d.id !== device.id)
  }
}
</script>

<style lang="less" scoped>
.devices-view {
  max-width: 1000px;
  margin: 0 auto;
}

.page-header {
  margin-bottom: 32px;

  .page-title {
    font-size: 32px;
    font-weight: 700;
    color: white;
    margin: 0 0 8px 0;
  }

  .page-subtitle {
    font-size: 16px;
    color: rgba(255, 255, 255, 0.5);
    margin: 0;
  }
}

.devices-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-bottom: 32px;
}

.device-card {
  display: flex;
  align-items: center;
  gap: 20px;

  .device-icon {
    width: 56px;
    height: 56px;
    border-radius: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 28px;
    background: rgba(0, 255, 245, 0.1);

    &.phone::before { content: '📱'; }
    &.handheld::before { content: '🎮'; }
    &.tv::before { content: '📺'; }
    &.pc::before { content: '💻'; }
  }

  .device-info {
    flex: 1;

    .device-name {
      font-size: 16px;
      font-weight: 600;
      color: white;
      margin-bottom: 4px;
    }

    .device-meta {
      font-size: 13px;
      color: rgba(255, 255, 255, 0.5);
      display: flex;
      align-items: center;
      gap: 8px;
    }
  }

  .device-status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: 20px;
    background: rgba(0, 0, 0, 0.2);

    .status-text {
      font-size: 13px;
      color: rgba(255, 255, 255, 0.7);
    }
  }
}

.empty-state {
  text-align: center;
  padding: 60px 20px;
  color: rgba(255, 255, 255, 0.5);

  .empty-icon {
    font-size: 64px;
    margin-bottom: 16px;
  }

  h3 {
    font-size: 20px;
    color: white;
    margin: 0 0 8px 0;
  }

  p {
    margin: 0;
  }
}

.pairing-card {
  .pairing-steps {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .step {
    display: flex;
    align-items: flex-start;
    gap: 16px;

    .step-number {
      width: 32px;
      height: 32px;
      border-radius: 50%;
      background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);
      color: #0f0f23;
      font-weight: 700;
      display: flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }

    .step-content {
      .step-title {
        font-size: 16px;
        font-weight: 600;
        color: white;
        margin-bottom: 4px;
      }

      .step-desc {
        font-size: 14px;
        color: rgba(255, 255, 255, 0.5);
      }
    }
  }
}
</style>

