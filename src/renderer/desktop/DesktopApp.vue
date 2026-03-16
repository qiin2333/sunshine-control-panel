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
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'

// 桌面 UI 组件
import DesktopWindow from './components/DesktopWindow.vue'
import DesktopSidebar from './components/DesktopSidebar.vue'
import ThemeEditor from './components/ThemeEditor.vue'

// 手柄支持
import { useGamepad, navigateFocus, confirmFocused } from './composables/useGamepad.js'
import { useTheme } from './composables/useTheme.js'

// 图标组件
import IconApps from './icons/IconApps.vue'
import IconDashboard from './icons/IconDashboard.vue'
import IconDevices from './icons/IconDevices.vue'
import IconStream from './icons/IconStream.vue'
import IconTools from './icons/IconTools.vue'
import IconSettings from './icons/IconSettings.vue'
import IconPower from './icons/IconPower.vue'
import IconPalette from './icons/IconPalette.vue'

// 视图组件
import AppsView from './views/AppsView.vue'
import DashboardView from './views/DashboardView.vue'
import DevicesView from './views/DevicesView.vue'
import StreamView from './views/StreamView.vue'
import ToolsView from './views/ToolsView.vue'
import SettingsView from './views/SettingsView.vue'

// 导入图标资源
import sunshineIcon from '../../assets/sunshine.ico'

// 应用配置
const appTitle = 'FOUNDATION DESKTOP'

// 主题
const { themeVars, activePreset, presets, setVar, applyPreset, exportTheme, importTheme, wallpaper, wallpaperColors, setWallpaper, removeWallpaper } = useTheme()
const themeEditorOpen = ref(false)

function handleThemeExport() {
  const json = exportTheme()
  navigator.clipboard.writeText(json).catch(() => {})
}

function handleThemeImport() {
  const json = prompt('粘贴主题 JSON:')
  if (json) importTheme(json)
}

// 导航状态 - 默认进入应用库页面
const activeNav = ref('apps')

// 主导航项
const navItems = [
  {
    id: 'apps',
    label: '应用',
    icon: IconApps,
    disabled: false,
  },
  {
    id: 'dashboard',
    label: '仪表盘',
    icon: IconDashboard,
    disabled: false,
  },
  {
    id: 'devices',
    label: '设备',
    icon: IconDevices,
    disabled: false,
  },
  {
    id: 'stream',
    label: '串流',
    icon: IconStream,
    disabled: false,
  },
  {
    id: 'tools',
    label: '工具',
    icon: IconTools,
    disabled: false,
  },
]

// 底部导航项
const bottomNavItems = [
  {
    id: 'theme',
    label: '主题',
    icon: IconPalette,
    disabled: false,
  },
  {
    id: 'settings',
    label: '设置',
    icon: IconSettings,
    disabled: false,
  },
  {
    id: 'exit',
    label: '退出',
    icon: IconPower,
    disabled: false,
  },
]

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

onMounted(async () => {
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }
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
    if (themeEditorOpen.value) {
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
