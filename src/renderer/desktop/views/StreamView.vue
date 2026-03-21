<template>
  <div class="stream-view">
    <div class="page-header fade-in">
      <h1 class="page-title">串流配置</h1>
      <p class="page-subtitle">快速调整 Sunshine 核心串流参数</p>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading" class="loading-state fade-in">
      <div class="loading-spinner"></div>
      <span>正在读取 Sunshine 配置…</span>
    </div>

    <template v-else>
      <!-- 编码格式 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon">🎬</span>
            编码格式
          </div>
        </div>
        <div class="card-content">
          <div class="codec-grid">
            <div 
              v-for="codec in codecs" 
              :key="codec.key"
              class="codec-card"
              :class="{ active: configData[codec.key] > 0 }"
              @click="toggleCodec(codec.key)"
            >
              <div class="codec-header">
                <span class="codec-name">{{ codec.name }}</span>
                <span class="codec-toggle" :class="{ on: configData[codec.key] > 0 }">
                  {{ configData[codec.key] > 0 ? '已启用' : '未启用' }}
                </span>
              </div>
              <div class="codec-desc">{{ codec.desc }}</div>
              <div v-if="configData[codec.key] > 0" class="codec-mode" @click.stop>
                <PillGroup v-model="configData[codec.key]" :options="codecModes" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 码率上限 & HDR -->
      <div class="desktop-grid cols-2 fade-in">
        <div class="desktop-card">
          <div class="card-header">
            <div class="card-title">
              <span class="title-icon">📊</span>
              码率上限
            </div>
            <div class="card-actions">
              <span class="bitrate-badge">{{ bitrateDisplay }}</span>
            </div>
          </div>
          <div class="card-content">
            <input 
              type="range" 
              v-model.number="bitrateKbps" 
              :min="1000" 
              :max="200000" 
              :step="1000"
              class="slider"
            />
            <div class="slider-labels">
              <span>1 Mbps</span>
              <span>200 Mbps</span>
            </div>
            <div class="preset-row">
              <button class="preset-btn" @click="bitrateKbps = 20000">20 Mbps</button>
              <button class="preset-btn" @click="bitrateKbps = 50000">50 Mbps</button>
              <button class="preset-btn" @click="bitrateKbps = 100000">100 Mbps</button>
              <button class="preset-btn" @click="bitrateKbps = 0">不限制</button>
            </div>
          </div>
        </div>

        <div class="desktop-card">
          <div class="card-header">
            <div class="card-title">
              <span class="title-icon">🌈</span>
              HDR
            </div>
          </div>
          <div class="card-content">
            <div class="toggle-row">
              <div class="toggle-info">
                <div class="toggle-label">自动 HDR 切换</div>
                <div class="toggle-desc">串流时自动开启/关闭 HDR</div>
              </div>
              <button 
                class="toggle-btn" 
                :class="{ on: configData.hdr_prep === 'automatic' }"
                @click="configData.hdr_prep = configData.hdr_prep === 'automatic' ? 'no_operation' : 'automatic'"
              >
                {{ configData.hdr_prep === 'automatic' ? '自动' : '手动' }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 显示与捕获 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon">🖥️</span>
            显示与捕获
          </div>
        </div>
        <div class="card-content settings-list">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">输出显示器</div>
              <div class="setting-desc">选择串流捕获的显示器</div>
            </div>
            <FdDropdown 
              v-model="configData.output_name" 
              :options="displayOptions" 
              placeholder="自动选择" 
            />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">分辨率自适应</div>
              <div class="setting-desc">根据客户端请求自动切换分辨率</div>
            </div>
            <PillGroup v-model="configData.resolution_change" :options="adaptModes" />
          </div>
          <div v-if="configData.resolution_change === 2" class="setting-row sub">
            <div class="setting-info">
              <div class="setting-label">手动分辨率</div>
            </div>
            <input 
              v-model="configData.manual_resolution" 
              class="setting-input" 
              placeholder="1920x1080"
            />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">刷新率自适应</div>
              <div class="setting-desc">根据客户端请求自动切换刷新率</div>
            </div>
            <PillGroup v-model="configData.refresh_rate_change" :options="adaptModes" />
          </div>
          <div v-if="configData.refresh_rate_change === 2" class="setting-row sub">
            <div class="setting-info">
              <div class="setting-label">手动刷新率</div>
            </div>
            <input 
              v-model="configData.manual_refresh_rate" 
              class="setting-input" 
              placeholder="60"
            />
          </div>
        </div>
      </div>

      <!-- 启动模式 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon">🚀</span>
            启动模式
          </div>
        </div>
        <div class="card-content">
          <div class="launch-mode-card" :class="{ active: autoLaunchDesktop }" @click="autoLaunchDesktop = !autoLaunchDesktop">
            <div class="launch-mode-main">
              <div class="launch-mode-info">
                <div class="launch-mode-title">串流时自动打开 Desktop UI</div>
                <div class="launch-mode-desc">
                  Moonlight 连接「Desktop」后自动全屏启动本面板，
                  提供游戏库、快捷工具等沉浸式桌面体验
                </div>
              </div>
              <button 
                class="toggle-btn" 
                :class="{ on: autoLaunchDesktop }"
                @click.stop="autoLaunchDesktop = !autoLaunchDesktop"
              >
                {{ autoLaunchDesktop ? '已开启' : '未开启' }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 虚拟显示器 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon">💻</span>
            虚拟显示器 (VDD)
          </div>
        </div>
        <div class="card-content settings-list">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">物理显示器处理</div>
              <div class="setting-desc">使用 VDD 时如何处理物理显示器</div>
            </div>
            <PillGroup 
              v-model="configData.vdd_prep" 
              :options="[{ value: 0, label: '不处理' }, { value: 1, label: '禁用物理显示器' }]" 
            />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">保持 VDD 启用</div>
              <div class="setting-desc">串流结束后不销毁虚拟显示器</div>
            </div>
            <button 
              class="toggle-btn" 
              :class="{ on: configData.vdd_keep_enabled === 'enabled' }"
              @click="configData.vdd_keep_enabled = configData.vdd_keep_enabled === 'enabled' ? 'disabled' : 'enabled'"
            >
              {{ configData.vdd_keep_enabled === 'enabled' ? '是' : '否' }}
            </button>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">无头模式自动创建</div>
              <div class="setting-desc">无物理显示器时自动创建 VDD</div>
            </div>
            <button 
              class="toggle-btn" 
              :class="{ on: configData.vdd_headless_create_enabled === 'enabled' }"
              @click="configData.vdd_headless_create_enabled = configData.vdd_headless_create_enabled === 'enabled' ? 'disabled' : 'enabled'"
            >
              {{ configData.vdd_headless_create_enabled === 'enabled' ? '是' : '否' }}
            </button>
          </div>
        </div>
      </div>

      <!-- 虚拟鼠标驱动 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon">🖱️</span>
            虚拟鼠标 (VMouse)
          </div>
          <div class="card-actions">
            <span class="status-badge" :class="vmouseStatusClass">{{ vmouseStatusLabel }}</span>
          </div>
        </div>
        <div class="card-content settings-list">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">功能开关</div>
              <div class="setting-desc">启用后使用 HID 虚拟鼠标代替 SendInput（需重启 Sunshine）</div>
            </div>
            <button 
              class="toggle-btn" 
              :class="{ on: vmouseEnabled }"
              @click="toggleVmouse"
              :disabled="vmouseConfigSaving"
            >
              {{ vmouseEnabled ? '已启用' : '未启用' }}
            </button>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">驱动状态</div>
              <div class="setting-desc">{{ vmouseStatus.status_text || '检测中...' }}</div>
            </div>
            <button 
              v-if="!vmouseStatus.installed"
              class="desktop-btn primary"
              :disabled="vmouseInstalling"
              @click="installVmouse"
            >
              {{ vmouseInstalling ? '安装中…' : '安装驱动' }}
            </button>
            <button 
              v-else
              class="desktop-btn danger"
              :disabled="vmouseUninstalling"
              @click="uninstallVmouse"
            >
              {{ vmouseUninstalling ? '卸载中…' : '卸载驱动' }}
            </button>
          </div>
        </div>
      </div>

      <!-- 保存 -->
      <div class="actions-bar fade-in">
        <span v-if="saveMsg" class="save-msg" :class="saveMsg.type">{{ saveMsg.text }}</span>
        <button class="desktop-btn primary" :disabled="saving" @click="saveSettings">
          {{ saving ? '保存中…' : '保存设置' }}
        </button>
      </div>

    </template>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import PillGroup from '../components/PillGroup.vue'
import FdDropdown from '../components/FdDropdown.vue'
import { vmouse as vmouseApi } from '../../tauri-adapter.js'

const invoke = ref(null)
const proxyUrl = ref('http://localhost:48081')
const loading = ref(true)
const saving = ref(false)
const saveMsg = ref(null)
const displays = ref([])
const autoLaunchDesktop = ref(false)
const appsData = ref(null)  // 原始 apps.json 数据

const configData = ref({
  hevc_mode: 2,
  av1_mode: 0,
  max_bitrate: '50000',
  hdr_prep: 'automatic',
  output_name: '',
  resolution_change: 1,
  manual_resolution: '',
  refresh_rate_change: 1,
  manual_refresh_rate: '',
  vdd_prep: 0,
  vdd_keep_enabled: 'disabled',
  vdd_headless_create_enabled: 'disabled',
})

const bitrateKbps = computed({
  get: () => parseInt(configData.value.max_bitrate) || 0,
  set: (v) => { configData.value.max_bitrate = String(v) },
})

const bitrateDisplay = computed(() => {
  const kbps = bitrateKbps.value
  if (kbps <= 0) return '不限制'
  return kbps >= 1000 ? `${(kbps / 1000).toFixed(0)} Mbps` : `${kbps} Kbps`
})

const codecs = [
  { key: 'hevc_mode', name: 'HEVC (H.265)', desc: '高效编码，主流设备广泛支持' },
  { key: 'av1_mode', name: 'AV1', desc: '最新一代编码，需要较新硬件' },
]

const codecModes = [
  { value: 1, label: '允许' },
  { value: 2, label: '始终' },
  { value: 3, label: '始终+HDR' },
]

const adaptModes = [
  { value: 0, label: '不改变' },
  { value: 1, label: '自动' },
  { value: 2, label: '手动' },
]

const displayOptions = computed(() => {
  const opts = [{ value: '', label: '自动选择' }]
  for (const d of displays.value) {
    opts.push({ value: d, label: d })
  }
  return opts
})

function toggleCodec(key) {
  configData.value[key] = configData.value[key] > 0 ? 0 : 2
}

async function initTauri() {
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
    const url = await invoke.value('get_proxy_url_command')
    if (url) proxyUrl.value = url
  } catch (e) {
    // not in Tauri
  }
}

async function apiFetch(path, options = {}) {
  const response = await fetch(`${proxyUrl.value}${path}`, options)
  return await response.json()
}

async function loadSettings() {
  loading.value = true
  try {
    const data = await apiFetch('/api/config')
    if (data.status?.toString() === 'true') {
      const keys = Object.keys(configData.value)
      for (const key of keys) {
        if (data[key] != null) {
          // 整数类型字段
          if (['hevc_mode', 'av1_mode', 'resolution_change', 'refresh_rate_change', 'vdd_prep'].includes(key)) {
            configData.value[key] = parseInt(data[key]) || 0
          } else {
            configData.value[key] = data[key]
          }
        }
      }
    }

    // 检测可用显示器
    if (invoke.value) {
      try {
        const monitors = await invoke.value('get_monitors')
        if (monitors && monitors.length > 0) displays.value = monitors
      } catch (e) { /* no monitor list command */ }
    }

    // 加载 apps 数据，检测 Desktop 应用是否配置了自动启动 Desktop UI
    try {
      const appsResp = await apiFetch('/api/apps')
      const appsList = appsResp.apps || appsResp || []
      appsData.value = { apps: appsList, env: appsResp.env || {} }
      const desktopApp = appsList.find(isDesktopApp)
      if (desktopApp) {
        const detached = desktopApp.detached || []
        autoLaunchDesktop.value = detached.some(cmd => 
          cmd.includes('sunshine-gui') && (cmd.includes('--desktop') || cmd.includes('-d'))
        )
      }
    } catch (e) {
      // apps load failed
    }
  } catch (e) {
    console.error('Failed to load settings:', e)
  } finally {
    loading.value = false
  }
}

async function saveSettings() {
  saving.value = true
  saveMsg.value = null
  try {
    // 先获取当前完整配置，避免覆盖其他字段
    const current = await apiFetch('/api/config')
    if (current.status?.toString() !== 'true') {
      saveMsg.value = { type: 'error', text: '读取当前配置失败' }
      return
    }

    // 基于完整配置合并我们修改的字段
    const payload = {}
    for (const [key, value] of Object.entries(current)) {
      if (key === 'status') continue
      payload[key] = String(value)
    }
    // 覆盖 StreamView 管理的字段
    for (const [key, value] of Object.entries(configData.value)) {
      payload[key] = String(value)
    }

    const result = await apiFetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    if (result.status?.toString() !== 'true') {
      saveMsg.value = { type: 'error', text: result.error || '保存配置失败' }
      return
    }

    // 保存 Desktop 应用的启动模式
    await saveDesktopLaunchMode()

    if (!saveMsg.value) {
      saveMsg.value = { type: 'success', text: '已保存，部分设置需要重启 Sunshine 生效' }
    }
  } catch (e) {
    saveMsg.value = { type: 'error', text: '无法连接 Sunshine' }
  } finally {
    saving.value = false
    setTimeout(() => { saveMsg.value = null }, 5000)
  }
}

const GUI_DESKTOP_CMD = '.\\assets\\gui\\sunshine-gui.exe --desktop'
const DESKTOP_APP_NAMES = ['Desktop', '桌面']
function isDesktopApp(app) { return DESKTOP_APP_NAMES.includes(app.name) }

async function saveDesktopLaunchMode() {
  if (!appsData.value) return
  const appsList = appsData.value.apps || []
  const desktopIdx = appsList.findIndex(isDesktopApp)
  if (desktopIdx === -1) return

  const desktopApp = { ...appsList[desktopIdx] }
  let detached = [...(desktopApp.detached || [])]

  // 移除旧的 Desktop UI 启动命令
  detached = detached.filter(cmd => 
    !(cmd.includes('sunshine-gui') && (cmd.includes('--desktop') || cmd.includes('-d')))
  )

  // 如果开启了自动启动，添加命令
  if (autoLaunchDesktop.value) {
    detached.push(GUI_DESKTOP_CMD)
  }

  desktopApp.detached = detached

  try {
    const editApp = { ...desktopApp, index: desktopIdx }
    const appsResult = await apiFetch('/api/apps', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ apps: appsList, editApp }),
    })
    if (appsResult.status?.toString() !== 'true') {
      saveMsg.value = { type: 'error', text: '启动模式保存失败: ' + (appsResult.error || '') }
    }
  } catch (e) {
    saveMsg.value = { type: 'error', text: '启动模式保存失败' }
  }
}

onMounted(async () => {
  await initTauri()
  await loadSettings()
  await loadVmouseStatus()
})

// ========== 虚拟鼠标驱动管理 ==========
const vmouseStatus = ref({ installed: false, running: false, status_text: '检测中...', driver_path: '', config_enabled: true })
const vmouseEnabled = ref(true)
const vmouseConfigSaving = ref(false)
const vmouseInstalling = ref(false)
const vmouseUninstalling = ref(false)

const vmouseStatusClass = computed(() => {
  if (vmouseStatus.value.running) return 'good'
  if (vmouseStatus.value.installed) return 'warn'
  return 'off'
})

const vmouseStatusLabel = computed(() => {
  if (vmouseStatus.value.running) return '运行中'
  if (vmouseStatus.value.installed) return '已安装'
  return '未安装'
})

async function loadVmouseStatus() {
  try {
    const result = await vmouseApi.getStatus()
    if (result?.success) {
      vmouseStatus.value = result.data
      vmouseEnabled.value = result.data.config_enabled
    }
  } catch (e) {
    console.error('获取 vmouse 状态失败:', e)
  }
}

async function toggleVmouse() {
  const newVal = !vmouseEnabled.value
  vmouseConfigSaving.value = true
  try {
    const result = await vmouseApi.setConfig(newVal)
    if (result?.success) {
      vmouseEnabled.value = newVal
    } else {
      console.error('设置 vmouse 失败:', result?.message)
    }
  } catch (e) {
    console.error('设置 vmouse 失败:', e)
  } finally {
    vmouseConfigSaving.value = false
  }
}

async function installVmouse() {
  if (!confirm('将安装虚拟鼠标驱动，需要管理员权限。\n\n是否继续？')) return
  vmouseInstalling.value = true
  try {
    const result = await vmouseApi.install()
    if (result?.success) {
      alert(result.data)
      setTimeout(() => loadVmouseStatus(), 2000)
    } else {
      alert('安装失败: ' + (result?.message || '未知错误'))
    }
  } catch (e) {
    alert('安装失败: ' + e)
  } finally {
    vmouseInstalling.value = false
  }
}

async function uninstallVmouse() {
  if (!confirm('确定要卸载虚拟鼠标驱动吗？\nSunshine 将回退到 SendInput 方式。')) return
  vmouseUninstalling.value = true
  try {
    const result = await vmouseApi.uninstall()
    if (result?.success) {
      alert(result.data)
      setTimeout(() => loadVmouseStatus(), 2000)
    } else {
      alert('卸载失败: ' + (result?.message || '未知错误'))
    }
  } catch (e) {
    alert('卸载失败: ' + e)
  } finally {
    vmouseUninstalling.value = false
  }
}
</script>

<style lang="less" scoped>
.stream-view {
  max-width: 1200px;
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

// Loading
.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 16px;
  padding: 80px 0;
  font-size: 16px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
}

.loading-spinner {
  width: 24px;
  height: 24px;
  border: 3px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-top-color: var(--fd-accent, #00fff5);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

// Cards
.desktop-card {
  margin-bottom: 24px;
}

// Codec grid
.codec-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;

  @media (max-width: 700px) {
    grid-template-columns: 1fr;
  }
}

.codec-card {
  padding: 20px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
  }

  &.active {
    border-color: var(--fd-accent, #00fff5);
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
  }

  .codec-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .codec-name {
    font-size: 16px;
    font-weight: 600;
    color: var(--fd-text-primary, #fff);
  }

  .codec-toggle {
    font-size: 12px;
    padding: 3px 10px;
    border-radius: 12px;
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.08);
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);
    transition: all 0.2s ease;

    &.on {
      background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
      color: var(--fd-accent, #00fff5);
    }
  }

  .codec-desc {
    font-size: 13px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.45);
  }

  .codec-mode {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
  }
}

// Bitrate
.bitrate-badge {
  font-size: 22px;
  font-weight: 700;
  background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.slider {
  width: 100%;
  height: 6px;
  border-radius: 3px;
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
  appearance: none;
  outline: none;
  margin-bottom: 8px;

  &::-webkit-slider-thumb {
    appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);
    cursor: pointer;
    box-shadow: 0 0 8px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.4);
  }
}

.slider-labels {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);
  margin-bottom: 14px;
}

.preset-row {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.preset-btn {
  padding: 6px 14px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 6px;
  background: transparent;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  cursor: pointer;
  font-size: 13px;
  transition: all 0.2s ease;

  &:hover {
    border-color: var(--fd-accent, #00fff5);
    color: var(--fd-accent, #00fff5);
  }
}

// Toggle button
.toggle-row {
  display: flex;
  align-items: center;
  gap: 16px;
}

.toggle-info {
  flex: 1;
}

.toggle-label {
  font-weight: 500;
  color: var(--fd-text-primary, #fff);
  margin-bottom: 2px;
}

.toggle-desc {
  font-size: 13px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.45);
}

.toggle-btn {
  padding: 8px 20px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: transparent;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  transition: all 0.2s ease;
  min-width: 80px;
  text-align: center;

  &.on {
    border-color: var(--fd-accent, #00fff5);
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    color: var(--fd-accent, #00fff5);
  }
}

// Settings list
.settings-list {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px 0;
  border-bottom: 1px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.06);

  &:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  &:first-child {
    padding-top: 0;
  }

  &.sub {
    padding-left: 24px;
    opacity: 0.85;
  }
}

.setting-info {
  flex: 1;
}

.setting-label {
  font-weight: 500;
  color: var(--fd-text-primary, #fff);
  margin-bottom: 2px;
}

.setting-desc {
  font-size: 13px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.45);
}

.setting-input {
  padding: 8px 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.05);
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  outline: none;
  width: 140px;

  &:focus {
    border-color: var(--fd-accent, #00fff5);
  }
}

// Launch mode
.launch-mode-card {
  padding: 20px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
  }

  &.active {
    border-color: var(--fd-accent, #00fff5);
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
  }

  .launch-mode-main {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .launch-mode-info {
    flex: 1;
  }

  .launch-mode-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--fd-text-primary, #fff);
    margin-bottom: 6px;
  }

  .launch-mode-desc {
    font-size: 13px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.45);
    line-height: 1.5;
  }
}

// Actions bar
.actions-bar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 16px;
  margin-top: 32px;
  padding-top: 24px;
  border-top: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
}

.save-msg {
  font-size: 14px;
  margin-right: auto;

  &.success {
    color: var(--fd-status-online, #4ade80);
  }

  &.error {
    color: var(--fd-status-error, #f87171);
  }
}

// VMouse status badge
.status-badge {
  font-size: 12px;
  font-weight: 600;
  padding: 4px 10px;
  border-radius: 20px;
  letter-spacing: 0.5px;

  &.good {
    background: rgba(74, 222, 128, 0.15);
    color: #4ade80;
  }

  &.warn {
    background: rgba(251, 191, 36, 0.15);
    color: #fbbf24;
  }

  &.off {
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.08);
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);
  }
}

.desktop-btn.danger {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.3);

  &:hover {
    background: rgba(248, 113, 113, 0.1);
    border-color: #f87171;
  }
}

</style>

