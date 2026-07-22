<template>
  <div class="stream-view">
    <div class="page-header fade-in">
      <h1 class="page-title">{{ t.stream.pageTitle }}</h1>
      <p class="page-subtitle">{{ t.stream.pageSubtitle }}</p>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading" class="loading-state fade-in">
      <div class="loading-spinner"></div>
      <span>{{ t.stream.loading }}</span>
    </div>

    <template v-else>
      <!-- 编码格式 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon"><Film /></span>
            {{ t.stream.codec.format }}
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
                  {{ configData[codec.key] > 0 ? t.stream.codec.enabled : t.stream.codec.disabled }}
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
              <span class="title-icon"><DataAnalysis /></span>
              {{ t.stream.bitrateLimit }}
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
              <button class="preset-btn" @click="bitrateKbps = 0">{{ t.stream.bitrateUnlimited }}</button>
            </div>
          </div>
        </div>

        <div class="desktop-card">
          <div class="card-header">
            <div class="card-title">
              <span class="title-icon"><Sunny /></span>
              {{ t.stream.hdr }}
            </div>
          </div>
          <div class="card-content">
            <div class="toggle-row">
              <div class="toggle-info">
                <div class="toggle-label">{{ t.stream.hdrAutoSwitch }}</div>
                <div class="toggle-desc">{{ t.stream.hdrAutoSwitchDesc }}</div>
              </div>
              <button 
                class="toggle-btn" 
                :class="{ on: configData.hdr_prep === 'automatic' }"
                @click="configData.hdr_prep = configData.hdr_prep === 'automatic' ? 'no_operation' : 'automatic'"
              >
                {{ configData.hdr_prep === 'automatic' ? t.stream.hdrMode.auto : t.stream.hdrMode.manual }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 显示与捕获 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon"><Monitor /></span>
            {{ t.stream.displayCapture }}
          </div>
        </div>
        <div class="card-content settings-list">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.outputDisplay }}</div>
              <div class="setting-desc">{{ t.stream.outputDisplayDesc }}</div>
            </div>
            <FdDropdown 
              v-model="configData.output_name" 
              :options="displayOptions" 
              :placeholder="t.stream.outputDisplayAuto" 
            />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.resolutionAdapt }}</div>
              <div class="setting-desc">{{ t.stream.resolutionAdaptDesc }}</div>
            </div>
            <PillGroup v-model="configData.resolution_change" :options="adaptModes" />
          </div>
          <div v-if="configData.resolution_change === 2" class="setting-row sub">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.manualResolution }}</div>
            </div>
            <input 
              v-model="configData.manual_resolution" 
              class="setting-input" 
              placeholder="1920x1080"
            />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.refreshRateAdapt }}</div>
              <div class="setting-desc">{{ t.stream.refreshRateAdaptDesc }}</div>
            </div>
            <PillGroup v-model="configData.refresh_rate_change" :options="adaptModes" />
          </div>
          <div v-if="configData.refresh_rate_change === 2" class="setting-row sub">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.manualRefreshRate }}</div>
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
            <span class="title-icon"><Promotion /></span>
            {{ t.stream.launchMode }}
          </div>
        </div>
        <div class="card-content">
          <div class="launch-mode-card" :class="{ active: autoLaunchDesktop }" @click="autoLaunchDesktop = !autoLaunchDesktop">
            <div class="launch-mode-main">
              <div class="launch-mode-info">
                <div class="launch-mode-title">{{ t.stream.autoLaunchDesktopUI }}</div>
                <div class="launch-mode-desc">
                  {{ t.stream.autoLaunchDesktopUIDesc }}
                </div>
              </div>
              <button 
                class="toggle-btn" 
                :class="{ on: autoLaunchDesktop }"
                @click.stop="autoLaunchDesktop = !autoLaunchDesktop"
              >
                {{ autoLaunchDesktop ? t.stream.launchOn : t.stream.launchOff }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 虚拟显示器 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon"><Monitor /></span>
            {{ t.stream.virtualDisplay }}
          </div>
          <div class="card-actions">
            <span class="status-badge" :class="vddStatusClass">{{ vddStatusLabel }}</span>
          </div>
        </div>
        <div class="card-content settings-list">
          <div v-if="vddStatus.state !== 'ready'" class="driver-notice" :class="{ unavailable: !vddReady }">
            <div class="driver-notice-copy">
              <strong>{{ t.vddSettings.driverPrerequisiteTitle }}</strong>
              <span>{{ vddStatus.status_text || t.vddSettings.driverPrerequisiteDesc }}</span>
            </div>
            <div class="driver-notice-actions">
              <button
                v-if="vddCanInstall"
                class="desktop-btn primary"
                :disabled="vddInstalling"
                @click="installOrRepairVdd"
              >
                {{ t.vddSettings.installRepairDriver }}
              </button>
              <button class="desktop-btn" :disabled="vddChecking" @click="loadVddStatus">
                {{ t.vddSettings.recheckDriver }}
              </button>
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.vddPhysicalHandling }}</div>
              <div class="setting-desc">{{ t.stream.vddPhysicalHandlingDesc }}</div>
            </div>
            <PillGroup 
              v-model="configData.vdd_prep" 
              :options="[{ value: 0, label: t.stream.vddPhysicalMode.noAction }, { value: 1, label: t.stream.vddPhysicalMode.disable }]" 
            />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.vddKeepEnabled }}</div>
              <div class="setting-desc">{{ t.stream.vddKeepEnabledDesc }}</div>
            </div>
            <button 
              class="toggle-btn" 
              :class="{ on: configData.vdd_keep_enabled === 'enabled' }"
              :disabled="vddConfigSaving || (configData.vdd_keep_enabled !== 'enabled' && !vddReady)"
              @click="toggleVddOption('keep')"
            >
              {{ configData.vdd_keep_enabled === 'enabled' ? t.stream.yes : t.stream.no }}
            </button>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.vddHeadlessCreate }}</div>
              <div class="setting-desc">{{ t.stream.vddHeadlessCreateDesc }}</div>
            </div>
            <button 
              class="toggle-btn" 
              :class="{ on: configData.vdd_headless_create_enabled === 'enabled' }"
              :disabled="vddConfigSaving || (configData.vdd_headless_create_enabled !== 'enabled' && !vddReady)"
              @click="toggleVddOption('headless')"
            >
              {{ configData.vdd_headless_create_enabled === 'enabled' ? t.stream.yes : t.stream.no }}
            </button>
          </div>
        </div>
      </div>

      <!-- 虚拟鼠标驱动 -->
      <div class="desktop-card fade-in">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon"><Mouse /></span>
            {{ t.stream.vmouse }}
          </div>
          <div class="card-actions">
            <span class="status-badge" :class="vmouseStatusClass">{{ vmouseStatusLabel }}</span>
          </div>
        </div>
        <div class="card-content settings-list">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.vmouseToggle }}</div>
              <div class="setting-desc">{{ t.stream.vmouseToggleDesc }}</div>
            </div>
            <button 
              class="toggle-btn" 
              :class="{ on: vmouseEnabled }"
              @click="toggleVmouse"
              :disabled="vmouseConfigSaving"
            >
              {{ vmouseEnabled ? t.stream.vmouseOn : t.stream.vmouseOff }}
            </button>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-label">{{ t.stream.vmouseDriverStatus }}</div>
              <div class="setting-desc">{{ vmouseStatus.status_text || t.stream.vmouseDetecting }}</div>
            </div>
            <button 
              v-if="!vmouseStatus.installed"
              class="desktop-btn primary"
              :disabled="vmouseInstalling"
              @click="installVmouse"
            >
              {{ vmouseInstalling ? t.stream.vmouseInstalling : t.stream.vmouseInstall }}
            </button>
            <button 
              v-else
              class="desktop-btn danger"
              :disabled="vmouseUninstalling"
              @click="uninstallVmouse"
            >
              {{ vmouseUninstalling ? t.stream.vmouseUninstalling : t.stream.vmouseUninstall }}
            </button>
          </div>
        </div>
      </div>

      <!-- 保存 -->
      <div class="actions-bar fade-in">
        <span v-if="saveMsg" class="save-msg" :class="saveMsg.type">{{ saveMsg.text }}</span>
        <button class="desktop-btn primary" :disabled="saving" @click="saveSettings">
          {{ saving ? t.stream.saving : t.stream.saveSettings }}
        </button>
      </div>

    </template>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { DataAnalysis, Film, Monitor, Mouse, Promotion, Sunny } from '@element-plus/icons-vue'
import PillGroup from '../components/PillGroup.vue'
import FdDropdown from '../components/FdDropdown.vue'
import { vdd as vddApi, vmouse as vmouseApi } from '../../tauri-adapter.js'
import { installVddWithRecovery } from '../../composables/vddInstallRecovery.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

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
const trayManagedVddKeys = new Set(['vdd_keep_enabled', 'vdd_headless_create_enabled'])

const bitrateKbps = computed({
  get: () => parseInt(configData.value.max_bitrate) || 0,
  set: (v) => { configData.value.max_bitrate = String(v) },
})

const bitrateDisplay = computed(() => {
  const kbps = bitrateKbps.value
  if (kbps <= 0) return t.value.stream.bitrateUnlimited
  return kbps >= 1000 ? `${(kbps / 1000).toFixed(0)} Mbps` : `${kbps} Kbps`
})

const codecs = computed(() => [
  { key: 'hevc_mode', name: t.value.stream.codec.hevc, desc: t.value.stream.codec.hevcDesc },
  { key: 'av1_mode', name: t.value.stream.codec.av1, desc: t.value.stream.codec.av1Desc },
])

const codecModes = computed(() => [
  { value: 1, label: t.value.stream.codec.allow },
  { value: 2, label: t.value.stream.codec.always },
  { value: 3, label: t.value.stream.codec.alwaysHdr },
])

const adaptModes = computed(() => [
  { value: 0, label: t.value.stream.adaptMode.noChange },
  { value: 1, label: t.value.stream.adaptMode.auto },
  { value: 2, label: t.value.stream.adaptMode.manual },
])

const displayOptions = computed(() => {
  const opts = [{ value: '', label: t.value.stream.outputDisplayAuto }]
  for (const d of displays.value) {
    if (typeof d === 'string') {
      opts.push({ value: d, label: d })
    } else {
      opts.push(d)
    }
  }
  return opts
})

function formatDisplayDeviceName(data = '') {
  const text = String(data || '')
  const displayMatch = text.match(/DISPLAY\d+/)
  const friendlyMatch = text.match(/FRIENDLY NAME:\s*([^\n\r]+)/)
  const friendly = friendlyMatch?.[1]?.trim()
  const display = displayMatch?.[0]
  if (friendly && display) return `${friendly} (${display})`
  return friendly || display || text.trim()
}

function normalizeDisplayDevices(devices) {
  if (!Array.isArray(devices)) return []
  return devices
    .map((device) => {
      if (typeof device === 'string') return { value: device, label: device }
      const value = device.device_id || device.id || device.value || ''
      const label = formatDisplayDeviceName(device.data || device.name || device.label || value)
      return value ? { value, label: label || value } : null
    })
    .filter(Boolean)
}

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
          } else if (trayManagedVddKeys.has(key)) {
            configData.value[key] = ['enabled', 'true', '1', 'yes'].includes(String(data[key]).toLowerCase())
              ? 'enabled'
              : 'disabled'
          } else {
            configData.value[key] = data[key]
          }
        }
      }
    }

    // 检测可用显示器
    const configDevices = normalizeDisplayDevices(data.display_devices)
    if (configDevices.length > 0) displays.value = configDevices

    if (invoke.value && displays.value.length === 0) {
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
      saveMsg.value = { type: 'error', text: t.value.stream.msg.readFailed }
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
      if (trayManagedVddKeys.has(key)) continue
      payload[key] = String(value)
    }

    const result = await apiFetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    if (result.status?.toString() !== 'true') {
      saveMsg.value = { type: 'error', text: result.error || t.value.stream.msg.saveFailed }
      return
    }

    // 保存 Desktop 应用的启动模式
    await saveDesktopLaunchMode()

    if (!saveMsg.value) {
      saveMsg.value = { type: 'success', text: t.value.stream.msg.saveSuccess }
    }
  } catch (e) {
    saveMsg.value = { type: 'error', text: t.value.stream.msg.connectionError }
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
      saveMsg.value = { type: 'error', text: t.value.stream.msg.launchModeSaveFailed + ': ' + (appsResult.error || '') }
    }
  } catch (e) {
    saveMsg.value = { type: 'error', text: t.value.stream.msg.launchModeSaveFailed }
  }
}

onMounted(async () => {
  await initTauri()
  await Promise.all([loadSettings(), loadVddStatus(), loadVmouseStatus()])
})

const vddStatus = ref({
  state: 'unknown',
  installed: false,
  running: false,
  control_available: false,
  status_text: '',
})
const vddChecking = ref(false)
const vddInstalling = ref(false)
const vddConfigSaving = ref(false)

const vddReady = computed(() => vddStatus.value.running && ['ready', 'degraded'].includes(vddStatus.value.state))
const vddCanInstall = computed(() => !['unsupported', 'payload_missing'].includes(vddStatus.value.state))
const vddStatusClass = computed(() => {
  if (vddStatus.value.state === 'ready') return 'good'
  if (vddReady.value) return 'warn'
  return 'off'
})
const vddStatusLabel = computed(() => {
  const labels = {
    ready: t.value.vddSettings.driverStateReady,
    degraded: t.value.vddSettings.driverStateDegraded,
    not_installed: t.value.vddSettings.driverStateNotInstalled,
    unhealthy: t.value.vddSettings.driverStateUnhealthy,
    reboot_required: t.value.vddSettings.driverStateRebootRequired,
    payload_missing: t.value.vddSettings.driverStatePayloadMissing,
    unsupported: t.value.vddSettings.driverStateUnsupported,
    unknown: t.value.vddSettings.driverStateUnknown,
  }
  return labels[vddStatus.value.state] || labels.unknown
})

async function loadVddStatus() {
  vddChecking.value = true
  try {
    const result = await vddApi.getStatus()
    if (!result?.success) throw new Error(result?.message || t.value.vddSettings.driverStatusCheckFailed)
    vddStatus.value = { ...vddStatus.value, ...result.data }
  } catch (error) {
    vddStatus.value = {
      ...vddStatus.value,
      state: 'unknown',
      running: false,
      status_text: error?.message || String(error),
    }
  } finally {
    vddChecking.value = false
  }
}

async function installOrRepairVdd() {
  if (!confirm(t.value.vddSettings.installRepairConfirm)) return
  vddInstalling.value = true
  try {
    await installVddWithRecovery({
      install: () => vddApi.install(),
      getStatus: () => vddApi.getStatus(),
      onStatus: (status) => {
        vddStatus.value = { ...vddStatus.value, ...status }
      },
      verificationError: t.value.vddSettings.installVerificationFailed,
    })
    alert(t.value.vddSettings.installRepairSuccess)
  } catch (error) {
    alert(`${t.value.vddSettings.installRepairFailed}: ${error?.message || error}`)
  } finally {
    vddInstalling.value = false
  }
}

async function toggleVddOption(option) {
  const key = option === 'keep' ? 'vdd_keep_enabled' : 'vdd_headless_create_enabled'
  const enabled = configData.value[key] !== 'enabled'
  vddConfigSaving.value = true
  try {
    const result = option === 'keep'
      ? await vddApi.setKeepEnabled(enabled)
      : await vddApi.setHeadlessCreateEnabled(enabled)
    if (!result?.success) throw new Error(result?.message || t.value.stream.msg.saveFailed)
    configData.value[key] = enabled ? 'enabled' : 'disabled'
  } catch (error) {
    alert(error?.message || error)
  } finally {
    vddConfigSaving.value = false
  }
}

// ========== 虚拟鼠标驱动管理 ==========
const vmouseStatus = ref({ installed: false, running: false, status_text: t.value.stream.vmouseDetecting, driver_path: '', config_enabled: true })
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
  if (vmouseStatus.value.running) return t.value.stream.vmouseStatusRunning
  if (vmouseStatus.value.installed) return t.value.stream.vmouseStatusInstalled
  return t.value.stream.vmouseStatusNotInstalled
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
  if (!confirm(t.value.stream.msg.vmouseInstallConfirm)) return
  vmouseInstalling.value = true
  try {
    const result = await vmouseApi.install()
    if (result?.success) {
      alert(result.data)
      setTimeout(() => loadVmouseStatus(), 2000)
    } else {
      alert(t.value.stream.msg.vmouseInstallFailed + (result?.message || ''))
    }
  } catch (e) {
    alert(t.value.stream.msg.vmouseInstallFailed + e)
  } finally {
    vmouseInstalling.value = false
  }
}

async function uninstallVmouse() {
  if (!confirm(t.value.stream.msg.vmouseUninstallConfirm)) return
  vmouseUninstalling.value = true
  try {
    const result = await vmouseApi.uninstall()
    if (result?.success) {
      alert(result.data)
      setTimeout(() => loadVmouseStatus(), 2000)
    } else {
      alert(t.value.stream.msg.vmouseUninstallFailed + (result?.message || ''))
    }
  } catch (e) {
    alert(t.value.stream.msg.vmouseUninstallFailed + e)
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
.driver-notice {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 12px;
  padding: 14px 16px;
  border: 1px solid rgba(251, 191, 36, 0.3);
  border-radius: 10px;
  background: rgba(251, 191, 36, 0.08);

  &.unavailable {
    border-color: rgba(248, 113, 113, 0.35);
    background: rgba(248, 113, 113, 0.08);
  }
}

.driver-notice-copy {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 4px;
  color: var(--fd-text-primary, #fff);

  span {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
    font-size: 13px;
  }
}

.driver-notice-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.settings-list {
  display: flex;
  flex-direction: column;
  gap: 0;
}

@media (max-width: 700px) {
  .driver-notice {
    align-items: stretch;
    flex-direction: column;
  }
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

