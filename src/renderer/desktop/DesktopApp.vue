<template>
  <DesktopWindow :title="appTitle" :icon="sunshineIcon" :has-sidebar="true">
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
      <component :is="currentView" />
    </template>
  </DesktopWindow>
</template>

<script setup>
import { ref, computed } from 'vue'

// 桌面 UI 组件
import DesktopWindow from './components/DesktopWindow.vue'
import DesktopSidebar from './components/DesktopSidebar.vue'

// 图标组件
import IconDashboard from './icons/IconDashboard.vue'
import IconDevices from './icons/IconDevices.vue'
import IconStream from './icons/IconStream.vue'
import IconTools from './icons/IconTools.vue'
import IconSettings from './icons/IconSettings.vue'

// 视图组件
import DashboardView from './views/DashboardView.vue'
import DevicesView from './views/DevicesView.vue'
import StreamView from './views/StreamView.vue'
import ToolsView from './views/ToolsView.vue'
import SettingsView from './views/SettingsView.vue'

// 导入图标资源
import sunshineIcon from '../../assets/sunshine.ico'

// 应用配置
const appTitle = 'SUNSHINE DESKTOP'

// 导航状态 - 默认进入工具页面
const activeNav = ref('tools')

// 主导航项
const navItems = [
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
    id: 'settings',
    label: '设置',
    icon: IconSettings,
    disabled: false,
  },
]

// 视图映射
const viewMap = {
  dashboard: DashboardView,
  devices: DevicesView,
  stream: StreamView,
  tools: ToolsView,
  settings: SettingsView,
}

const currentView = computed(() => viewMap[activeNav.value] || DashboardView)

// 导航点击处理
function handleNavClick(item) {
  if (item.disabled) return
  activeNav.value = item.id
}
</script>

<style lang="less" scoped>
// 组件样式由各自的组件文件管理
</style>
