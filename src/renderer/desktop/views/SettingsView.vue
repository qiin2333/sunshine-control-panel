<template>
  <div class="settings-view">
    <div class="page-header fade-in">
      <h1 class="page-title">设置</h1>
      <p class="page-subtitle">应用程序偏好设置</p>
    </div>

    <!-- 外观设置 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">🎨</span>
          外观
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">主题</div>
            <div class="setting-desc">选择应用程序的颜色主题</div>
          </div>
          <div class="setting-control">
            <select v-model="settings.theme" class="select-control">
              <option value="dark">深色 (赛博朋克)</option>
              <option value="light">浅色</option>
              <option value="system">跟随系统</option>
            </select>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">主题色</div>
            <div class="setting-desc">自定义强调色</div>
          </div>
          <div class="setting-control color-options">
            <div 
              v-for="color in accentColors" 
              :key="color.value"
              class="color-option"
              :class="{ active: settings.accentColor === color.value }"
              :style="{ background: color.gradient }"
              @click="settings.accentColor = color.value"
              :title="color.name"
            />
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">动画效果</div>
            <div class="setting-desc">启用界面动画和过渡效果</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.animations" />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <!-- 启动设置 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">🚀</span>
          启动
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">开机自启动</div>
            <div class="setting-desc">系统启动时自动运行应用程序</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.autoStart" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">启动时最小化</div>
            <div class="setting-desc">启动后最小化到系统托盘</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.startMinimized" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">自动启动 Sunshine 服务</div>
            <div class="setting-desc">应用启动时自动启动 Sunshine</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.autoStartSunshine" />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <!-- 通知设置 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">🔔</span>
          通知
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">桌面通知</div>
            <div class="setting-desc">显示系统桌面通知</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.notifications" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">连接通知</div>
            <div class="setting-desc">客户端连接/断开时通知</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.connectionNotify" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">更新通知</div>
            <div class="setting-desc">有新版本时通知</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.updateNotify" />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <!-- 高级设置 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">⚙️</span>
          高级
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">开发者模式</div>
            <div class="setting-desc">显示调试信息和开发者工具</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.devMode" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">日志级别</div>
            <div class="setting-desc">设置日志记录的详细程度</div>
          </div>
          <div class="setting-control">
            <select v-model="settings.logLevel" class="select-control">
              <option value="error">仅错误</option>
              <option value="warn">警告及以上</option>
              <option value="info">信息及以上</option>
              <option value="debug">调试（全部）</option>
            </select>
          </div>
        </div>
      </div>
    </div>

    <!-- 关于 -->
    <div class="desktop-card about-card fade-in">
      <div class="about-content">
        <div class="about-logo">☀️</div>
        <div class="about-info">
          <div class="about-name">Sunshine Desktop</div>
          <div class="about-version">版本 0.2.5</div>
          <div class="about-links">
            <a href="#" @click.prevent="openLink('github')">GitHub</a>
            <span>•</span>
            <a href="#" @click.prevent="openLink('docs')">文档</a>
            <span>•</span>
            <a href="#" @click.prevent="openLink('discord')">Discord</a>
          </div>
        </div>
      </div>
      <button class="desktop-btn" @click="checkUpdate">检查更新</button>
    </div>

    <!-- 保存按钮 -->
    <div class="actions-bar fade-in">
      <button class="desktop-btn" @click="resetSettings">恢复默认</button>
      <button class="desktop-btn primary" @click="saveSettings">保存设置</button>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'

const settings = ref({
  theme: 'dark',
  accentColor: 'cyan',
  animations: true,
  autoStart: false,
  startMinimized: false,
  autoStartSunshine: true,
  notifications: true,
  connectionNotify: true,
  updateNotify: true,
  devMode: false,
  logLevel: 'info',
})

const accentColors = [
  { value: 'cyan', name: '青色', gradient: 'linear-gradient(135deg, #00fff5 0%, #00d4aa 100%)' },
  { value: 'magenta', name: '品红', gradient: 'linear-gradient(135deg, #ff00ff 0%, #cc00cc 100%)' },
  { value: 'green', name: '绿色', gradient: 'linear-gradient(135deg, #00ff88 0%, #00cc6a 100%)' },
  { value: 'yellow', name: '金色', gradient: 'linear-gradient(135deg, #ffd700 0%, #ffaa00 100%)' },
  { value: 'orange', name: '橙色', gradient: 'linear-gradient(135deg, #ff6b35 0%, #ff4500 100%)' },
  { value: 'blue', name: '蓝色', gradient: 'linear-gradient(135deg, #6495ed 0%, #4169e1 100%)' },
]

function resetSettings() {
  settings.value = {
    theme: 'dark',
    accentColor: 'cyan',
    animations: true,
    autoStart: false,
    startMinimized: false,
    autoStartSunshine: true,
    notifications: true,
    connectionNotify: true,
    updateNotify: true,
    devMode: false,
    logLevel: 'info',
  }
}

function saveSettings() {
  // TODO: 保存设置
  console.log('Saving settings:', settings.value)
}

function openLink(type) {
  const urls = {
    github: 'https://github.com/LizardByte/Sunshine',
    docs: 'https://docs.lizardbyte.dev/projects/sunshine/',
    discord: 'https://discord.gg/lizardbyte',
  }
  window.open(urls[type], '_blank')
}

function checkUpdate() {
  // TODO: 检查更新
  console.log('Checking for updates...')
}
</script>

<style lang="less" scoped>
.settings-view {
  max-width: 800px;
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

.desktop-card {
  margin-bottom: 24px;
}

.setting-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 0;
  border-bottom: 1px solid rgba(0, 255, 245, 0.1);

  &:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  &:first-child {
    padding-top: 0;
  }

  .setting-info {
    .setting-name {
      font-size: 15px;
      font-weight: 500;
      color: white;
      margin-bottom: 4px;
    }

    .setting-desc {
      font-size: 13px;
      color: rgba(255, 255, 255, 0.5);
    }
  }
}

.select-control {
  padding: 8px 32px 8px 12px;
  border: 1px solid rgba(0, 255, 245, 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: white;
  font-size: 14px;
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2300fff5' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;

  &:focus {
    outline: none;
    border-color: #00fff5;
  }

  option {
    background: #1a1a2e;
    color: white;
  }
}

.color-options {
  display: flex;
  gap: 8px;

  .color-option {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    cursor: pointer;
    border: 2px solid transparent;
    transition: all 0.2s ease;

    &:hover {
      transform: scale(1.1);
    }

    &.active {
      border-color: white;
      box-shadow: 0 0 12px rgba(255, 255, 255, 0.3);
    }
  }
}

// 开关样式
.switch {
  position: relative;
  display: inline-block;
  width: 48px;
  height: 26px;

  input {
    opacity: 0;
    width: 0;
    height: 0;

    &:checked + .slider {
      background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);

      &::before {
        transform: translateX(22px);
      }
    }
  }

  .slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 26px;
    transition: 0.3s;

    &::before {
      position: absolute;
      content: "";
      height: 20px;
      width: 20px;
      left: 3px;
      bottom: 3px;
      background: white;
      border-radius: 50%;
      transition: 0.3s;
    }
  }
}

.about-card {
  display: flex;
  align-items: center;
  justify-content: space-between;

  .about-content {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .about-logo {
    font-size: 48px;
  }

  .about-info {
    .about-name {
      font-size: 18px;
      font-weight: 600;
      color: white;
    }

    .about-version {
      font-size: 14px;
      color: rgba(255, 255, 255, 0.5);
      margin-bottom: 4px;
    }

    .about-links {
      font-size: 13px;
      display: flex;
      gap: 8px;

      a {
        color: #00fff5;
        text-decoration: none;

        &:hover {
          text-decoration: underline;
        }
      }

      span {
        color: rgba(255, 255, 255, 0.3);
      }
    }
  }
}

.actions-bar {
  display: flex;
  justify-content: flex-end;
  gap: 16px;
  margin-top: 32px;
  padding-top: 24px;
  border-top: 1px solid rgba(0, 255, 245, 0.1);
}
</style>

