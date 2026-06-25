<template>
  <DesktopWindow :title="appTitle" :icon="sunshineIcon" :has-sidebar="true" :show-title-bar="false" :class="{ 'gamepad-active': gamepadActive }">
    <template #sidebar>
      <DesktopSidebar
        :items="navItems"
        :bottom-items="bottomNavItems"
        :active-item="activeNav"
        @item-click="handleNavClick"
        @update:active-item="activeNav = $event"
      />
    </template>

    <template #default>
      <component :is="currentView" @openThemeEditor="themeEditorOpen = true" />
    </template>
  </DesktopWindow>

  <ThemeEditor
    :open="themeEditorOpen"
    :vars="themeVars"
    :activePreset="activePreset"
    :presets="presets"
    :wallpaper="wallpaper"
    :wallpaperColors="wallpaperColors"
    @close="themeEditorOpen = false"
    @setVar="setVar"
    @applyPreset="applyPreset"
    @export="handleThemeExport"
    @import="handleThemeImport"
    @setWallpaper="setWallpaper"
    @removeWallpaper="removeWallpaper"
  />

  <SplashScreen
    :visible="showSplash"
    @done="showSplash = false"
  />
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'

// 桌面 UI 组件
import DesktopWindow from './components/DesktopWindow.vue'
import DesktopSidebar from './components/DesktopSidebar.vue'
import ThemeEditor from './components/ThemeEditor.vue'
import SplashScreen from './components/SplashScreen.vue'

// 手柄支持
import { useGamepad, navigateFocus, confirmFocused } from './composables/useGamepad.js'
import { useTheme } from './composables/useTheme.js'
import { useLaunchHelpers } from './composables/useLaunchHelpers.js'
import {
  DESKTOP_SETTINGS_UPDATED,
  desktopSettings,
  loadDesktopSettings,
  requestNotificationPermission,
  showDesktopNotification,
} from './composables/useDesktopSettings.js'
import { useI18n } from './i18n/index.js'

// 图标组件
import IconApps from './icons/IconApps.vue'
import IconDashboard from './icons/IconDashboard.vue'
import IconDevices from './icons/IconDevices.vue'
import IconStream from './icons/IconStream.vue'
import IconTools from './icons/IconTools.vue'
import IconSettings from './icons/IconSettings.vue'
import IconPower from './icons/IconPower.vue'
import IconPalette from './icons/IconPalette.vue'
import IconLang from './icons/IconLang.vue'

// 视图组件
import AppsView from './views/AppsView.vue'
import DashboardView from './views/DashboardView.vue'
import DevicesView from './views/DevicesView.vue'
import StreamView from './views/StreamView.vue'
import ToolsView from './views/ToolsView.vue'
import SettingsView from './views/SettingsView.vue'

// 导入图标资源
import sunshineIcon from '../../assets/sunshine.ico'

// i18n
const { t, locale, toggleLocale } = useI18n()

// 应用配置
const appTitle = 'FOUNDATION DESKTOP'

// 主题
const { themeVars, activePreset, presets, setVar, applyPreset, exportTheme, importTheme, wallpaper, wallpaperColors, setWallpaper, removeWallpaper } = useTheme()
const themeEditorOpen = ref(false)
const showSplash = ref(true)
const { helperPanelOpen } = useLaunchHelpers(t)

function handleThemeExport() {
  const json = exportTheme()
  navigator.clipboard.writeText(json).catch(() => {})
}

function handleThemeImport() {
  const json = prompt(t.value.nav.theme + ' JSON:')
  if (json) importTheme(json)
}

// 导航状态 - 默认进入应用库页面
const activeNav = ref('apps')

// 主导航项
const navItems = computed(() => [
  { id: 'apps', label: t.value.nav.apps, icon: IconApps, disabled: false },
  { id: 'dashboard', label: t.value.nav.dashboard, icon: IconDashboard, disabled: false },
  { id: 'devices', label: t.value.nav.devices, icon: IconDevices, disabled: false },
  { id: 'stream', label: t.value.nav.stream, icon: IconStream, disabled: false },
  { id: 'tools', label: t.value.nav.tools, icon: IconTools, disabled: false },
])

// 底部导航项
const bottomNavItems = computed(() => [
  { id: 'lang', label: locale.value === 'zh' ? 'EN' : '中文', icon: IconLang, disabled: false },
  { id: 'theme', label: t.value.nav.theme, icon: IconPalette, disabled: false },
  { id: 'settings', label: t.value.nav.settings, icon: IconSettings, disabled: false },
  { id: 'exit', label: t.value.nav.exit, icon: IconPower, disabled: false },
])

// 视图映射
const viewMap = {
  apps: AppsView,
  dashboard: DashboardView,
  devices: DevicesView,
  stream: StreamView,
  tools: ToolsView,
  settings: SettingsView,
}

const currentView = computed(() => viewMap[activeNav.value] || DashboardView)

// Tauri invoke
const invoke = ref(null)
let sessionNotifyTimer = null
let lastSessionIds = new Set()
let settingsListener = null
let updateUnlisten = null

function applyRuntimeSettings() {
  document.documentElement.dataset.fdDevMode = desktopSettings.value.devMode ? 'true' : 'false'
  if (desktopSettings.value.notifications) {
    requestNotificationPermission(desktopSettings.value).catch(() => {})
  }
}

function sessionKey(session) {
  return String(session.session_id ?? session.id ?? `${session.client_name || 'client'}:${session.client_address || ''}`)
}

async function pollSessionsForNotifications(initial = false) {
  if (!invoke.value) return
  try {
    const sessions = await invoke.value('get_active_sessions')
    const nextIds = new Set((sessions || []).map(sessionKey))
    if (!initial && desktopSettings.value.notifications && desktopSettings.value.connectionNotify) {
      for (const session of sessions || []) {
        const id = sessionKey(session)
        if (!lastSessionIds.has(id)) {
          showDesktopNotification('Sunshine', `${session.client_name || 'Client'} connected`, desktopSettings.value)
        }
      }
      for (const id of lastSessionIds) {
        if (!nextIds.has(id)) {
          showDesktopNotification('Sunshine', 'Client disconnected', desktopSettings.value)
        }
      }
    }
    lastSessionIds = nextIds
  } catch {
    // Sunshine may be offline; keep the last known state.
  }
}

async function setupUpdateNotifications() {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    updateUnlisten = await listen('update-available', (event) => {
      if (!desktopSettings.value.notifications || !desktopSettings.value.updateNotify) return
      const version = event.payload?.version || ''
      showDesktopNotification('Sunshine GUI', version ? `Update available: ${version}` : 'Update available', desktopSettings.value)
    })
  } catch {
    // event API unavailable outside Tauri
  }
}

onMounted(async () => {
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }
  await loadDesktopSettings()
  applyRuntimeSettings()
  settingsListener = () => applyRuntimeSettings()
  window.addEventListener(DESKTOP_SETTINGS_UPDATED, settingsListener)
  await setupUpdateNotifications()
  await pollSessionsForNotifications(true)
  sessionNotifyTimer = setInterval(() => pollSessionsForNotifications(false), 15000)
})

onUnmounted(() => {
  if (sessionNotifyTimer) clearInterval(sessionNotifyTimer)
  if (settingsListener) window.removeEventListener(DESKTOP_SETTINGS_UPDATED, settingsListener)
  if (updateUnlisten) updateUnlisten()
})

// 导航点击处理
async function handleNavClick(item) {
  if (item.disabled) return
  if (item.id === 'exit') {
    if (invoke.value) {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window')
        await getCurrentWindow().close()
      } catch (e) {
        window.close()
      }
    }
    return
  }
  if (item.id === 'theme') {
    themeEditorOpen.value = !themeEditorOpen.value
    return
  }
  if (item.id === 'lang') {
    toggleLocale()
    return
  }
  activeNav.value = item.id
}

// 手柄导航
const allNavIds = computed(() => [
  ...navItems.filter(i => !i.disabled).map(i => i.id),
  ...bottomNavItems.filter(i => !i.disabled).map(i => i.id),
])

const { gamepadActive } = useGamepad({
  onNavigate(direction) {
    navigateFocus(direction)
  },
  onConfirm() {
    confirmFocused()
  },
  onBack() {
    // B 按钮：优先关闭打开的抽屉面板，否则回到应用库首页
    if (helperPanelOpen.value) {
      helperPanelOpen.value = false
    } else if (themeEditorOpen.value) {
      themeEditorOpen.value = false
    } else {
      activeNav.value = 'apps'
    }
  },
  onTabPrev() {
    // LB：切换到上一个标签
    const ids = allNavIds.value
    const idx = ids.indexOf(activeNav.value)
    if (idx > 0) {
      const prevId = ids[idx - 1]
      if (prevId === 'exit') return
      activeNav.value = prevId
    }
  },
  onTabNext() {
    // RB：切换到下一个标签
    const ids = allNavIds.value
    const idx = ids.indexOf(activeNav.value)
    if (idx < ids.length - 1) {
      const nextId = ids[idx + 1]
      if (nextId === 'exit') return
      activeNav.value = nextId
    }
  },
})
</script>

<style lang="less" scoped>
// 组件样式由各自的组件文件管理
</style>
