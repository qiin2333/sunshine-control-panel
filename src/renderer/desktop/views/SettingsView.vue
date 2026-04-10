<template>
  <div class="settings-view">
    <div class="page-header fade-in">
      <h1 class="page-title">设置</h1>
      <p class="page-subtitle">应用程序偏好设置</p>
    </div>

    <!-- 外观设置 —— 统一由主题编辑器管理 -->
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
            <div class="setting-name">主题编辑器</div>
            <div class="setting-desc">自定义主题色、壁纸、预设和外观效果</div>
          </div>
          <div class="setting-control">
            <button class="desktop-btn" @click="$emit('openThemeEditor')">打开编辑器</button>
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

    <!-- 启动助手 — 全局工具路径配置 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">⚡</span>
          启动助手
        </div>
      </div>
      <div class="card-content">
        <p class="section-desc">
          在此设置常用工具路径，之后在应用右键菜单中一键启用即可。
        </p>
        <div
          v-for="tmpl in helperTemplates"
          :key="tmpl.id"
          class="setting-item tool-path-item"
        >
          <div class="setting-info">
            <div class="setting-name">{{ tmpl.icon }} {{ tmpl.name }}</div>
            <div class="setting-desc">{{ tmpl.description }}</div>
          </div>
          <div class="setting-control tool-path-control">
            <div
              v-for="param in tmpl.params.filter(p => p.key === 'path')"
              :key="param.key"
              class="tool-path-row"
            >
              <input
                type="text"
                class="path-input"
                :placeholder="param.placeholder"
                :value="getGlobalToolPath(tmpl.id, param.key)"
                @input="setGlobalToolPath(tmpl.id, param.key, $event.target.value)"
              />
              <button
                v-if="hasTauri"
                class="browse-btn-small"
                @click="browseToolPath(tmpl.id, param.key)"
              >📂</button>
            </div>
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

    <!-- 桌宠 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">🐾</span>
          桌宠
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">桌面观察</div>
            <div class="setting-desc">米塔会定时偷看你的桌面并发表评论（需要启用 AI 并配置支持视觉的模型如 GPT-4o）</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="petEnabled" @change="onPetToggle" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item" v-if="petEnabled">
          <div class="setting-info">
            <div class="setting-name">观察间隔（秒）</div>
            <div class="setting-desc">每隔多少秒截取桌面并生成评论（最小 15 秒）</div>
          </div>
          <div class="setting-control">
            <select v-model="petIntervalSec" class="select-control" @change="onPetIntervalChange">
              <option :value="15">15 秒</option>
              <option :value="30">30 秒</option>
              <option :value="60">60 秒</option>
              <option :value="120">2 分钟</option>
              <option :value="300">5 分钟</option>
            </select>
          </div>
        </div>

        <div class="setting-item" v-if="petEnabled">
          <div class="setting-info">
            <div class="setting-name">立即触发</div>
            <div class="setting-desc">让米塔现在就看看你的桌面</div>
          </div>
          <div class="setting-control">
            <button class="desktop-btn" :disabled="isObserving" @click="poke">
              {{ isObserving ? '观察中...' : '戳一下' }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 关于 -->
    <div class="desktop-card about-card fade-in">
      <div class="about-content">
        <div class="about-logo">☀️</div>
        <div class="about-info">
          <div class="about-name">Foundation Desktop</div>
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
      <button class="desktop-btn" :disabled="checking" @click="checkUpdate">
        {{ checking ? '检查中...' : '检查更新' }}
      </button>
    </div>

    <!-- 保存按钮 -->
    <div class="actions-bar fade-in">
      <button class="desktop-btn" @click="resetSettings">恢复默认</button>
      <button class="desktop-btn primary" @click="saveSettings">保存设置</button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useLaunchHelpers } from '../composables/useLaunchHelpers'
import { useDesktopPet } from '../../composables/useDesktopPet.js'

const invoke = ref(null)
const hasTauri = ref(false)

// 桌宠设置
const {
  petEnabled,
  isObserving,
  observeInterval,
  startObserving,
  stopObserving,
  setIntervalSeconds,
  poke,
} = useDesktopPet()

const petIntervalSec = ref(Math.round(observeInterval.value / 1000))

function onPetToggle() {
  if (petEnabled.value) {
    startObserving()
  } else {
    stopObserving()
  }
}

function onPetIntervalChange() {
  setIntervalSeconds(petIntervalSec.value)
}

const {
  templates: allTemplates,
  getGlobalPath: getGlobalToolPath,
  setGlobalPath: setGlobalToolPath,
} = useLaunchHelpers()

const helperTemplates = computed(() =>
  allTemplates.value.filter(t => t.id !== 'custom')
)

async function browseToolPath(templateId, paramKey) {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const path = await open({
      filters: [{ name: '可执行文件', extensions: ['exe', 'bat', 'cmd', 'lnk'] }],
    })
    if (path) {
      setGlobalToolPath(templateId, paramKey, path)
    }
  } catch (e) {
    console.warn('File dialog not available:', e)
  }
}

const SETTINGS_KEY = 'sunshine-desktop-settings'

const defaultSettings = {
  autoStart: false,
  startMinimized: false,
  autoStartSunshine: true,
  notifications: true,
  connectionNotify: true,
  updateNotify: true,
  devMode: false,
  logLevel: 'info',
}

const settings = ref({ ...defaultSettings })

defineEmits(['openThemeEditor'])

const updateStatus = ref(null)
const checking = ref(false)

function loadSettings() {
  try {
    const saved = localStorage.getItem(SETTINGS_KEY)
    if (saved) {
      settings.value = { ...defaultSettings, ...JSON.parse(saved) }
    }
  } catch (e) {
    console.error('Failed to load settings:', e)
  }
}

function resetSettings() {
  settings.value = { ...defaultSettings }
  localStorage.removeItem(SETTINGS_KEY)
}

function saveSettings() {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings.value))
  } catch (e) {
    console.error('Failed to save settings:', e)
  }
}

function openLink(type) {
  const urls = {
    github: 'https://github.com/LizardByte/Sunshine',
    docs: 'https://docs.lizardbyte.dev/projects/sunshine/',
    discord: 'https://discord.gg/lizardbyte',
  }
  if (invoke.value) {
    invoke.value('open_external_url', { url: urls[type] }).catch(() => {
      window.open(urls[type], '_blank')
    })
  } else {
    window.open(urls[type], '_blank')
  }
}

async function checkUpdate() {
  if (!invoke.value) return
  checking.value = true
  updateStatus.value = null
  try {
    const update = await invoke.value('check_for_updates')
    if (update) {
      updateStatus.value = { type: 'success', message: `发现新版本: ${update.version}` }
    } else {
      updateStatus.value = { type: 'info', message: '当前已是最新版本' }
    }
  } catch (e) {
    updateStatus.value = { type: 'error', message: '检查更新失败' }
  } finally {
    checking.value = false
  }
}

onMounted(async () => {
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
    hasTauri.value = true
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }
  loadSettings()
})
</script>

<style lang="less" scoped>
.settings-view {
  max-width: 1000px;
  margin: 0 auto;
}

.page-header {
  margin-bottom: 40px;

  .page-title {
    font-size: 40px;
    font-weight: 700;
    color: var(--fd-text-primary, #fff);
    margin: 0 0 10px 0;
  }

  .page-subtitle {
    font-size: 18px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    margin: 0;
  }
}

.desktop-card {
  margin-bottom: 28px;
}

.setting-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 0;
  border-bottom: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);

  &:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  &:first-child {
    padding-top: 0;
  }

  .setting-info {
    .setting-name {
      font-size: 18px;
      font-weight: 500;
      color: var(--fd-text-primary, #fff);
      margin-bottom: 4px;
    }

    .setting-desc {
      font-size: 14px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    }
  }
}

.select-control {
  padding: 8px 32px 8px 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2300fff5' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;

  &:focus {
    outline: none;
    border-color: var(--fd-accent, #00fff5);
  }

  option {
    background: var(--fd-bg-secondary, #1a1a2e);
    color: var(--fd-text-primary, #fff);
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
      background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);

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
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
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
      color: var(--fd-text-primary, #fff);
    }

    .about-version {
      font-size: 14px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
      margin-bottom: 4px;
    }

    .about-links {
      font-size: 13px;
      display: flex;
      gap: 8px;

      a {
        color: var(--fd-accent, #00fff5);
        text-decoration: none;

        &:hover {
          text-decoration: underline;
        }
      }

      span {
        color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
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
  border-top: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
}

.section-desc {
  font-size: 14px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  margin: 0 0 16px 0;
}

.tool-path-item {
  flex-direction: column;
  align-items: flex-start !important;
  gap: 10px;

  .setting-control {
    width: 100%;
  }
}

.tool-path-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.path-input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 13px;
  font-family: 'Consolas', 'Monaco', monospace;

  &::placeholder {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25);
  }

  &:focus {
    outline: none;
    border-color: var(--fd-accent, #00fff5);
  }
}

.browse-btn-small {
  padding: 7px 10px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  flex-shrink: 0;

  &:hover {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    border-color: var(--fd-accent, #00fff5);
  }
}
</style>

