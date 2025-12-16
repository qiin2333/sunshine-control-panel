<template>
  <div class="sidebar-wrapper">
    <!-- 侧边栏 -->
    <aside class="sidebar" :class="{ collapsed: isCollapsed }">
      <!-- Gura 背景装饰 -->
      <div class="gura-background">
        <img src="../public/gura-pix.png" alt="Gura" class="gura-bg-img" />
      </div>

      <!-- Logo 区域 (可拖动) -->
      <div class="sidebar-header" data-tauri-drag-region>
        <div class="logo">
          <img src="../public/gura-pix.png" alt="Sunshine Logo" class="logo-img" />
        </div>
        <transition name="fade">
          <h3 v-if="!isCollapsed" class="app-name">Sunshine Foundation</h3>
        </transition>
      </div>

      <!-- 折叠按钮 -->
      <div class="collapse-btn" @click="toggleCollapse" aria-label="折叠菜单">
        <img
          :class="['clip-icon', { collapsed: isCollapsed }]"
          src="../public/gura-clip.svg"
          alt="折叠发卡"
          width="24"
          height="24"
          aria-hidden="true"
        />
      </div>

      <!-- 菜单列表 -->
      <el-scrollbar class="menu-scrollbar">
        <div class="menu-section">
          <p v-if="!isCollapsed" class="section-title">管理</p>
          <div
            v-for="item in managementMenuItems"
            :key="item.label"
            class="menu-item"
            :class="[{ active: item.isActive?.() }, { 'menu-item-switch': item.hasSwitch }]"
            @click.stop="item.action"
          >
            <el-icon :size="20"><component :is="item.icon" /></el-icon>
            <transition name="fade">
              <template v-if="!isCollapsed">
                <div v-if="item.hasSwitch" class="update-item-content">
                  <span>{{ item.label }}</span>
                  <el-switch
                    v-model="includePrerelease"
                    size="small"
                    active-text="Beta"
                    @change="handlePrereleaseToggle"
                    @click.stop
                  />
                </div>
                <span v-else>{{ item.label }}</span>
              </template>
              <span v-else>{{ item.label }}</span>
            </transition>
          </div>
        </div>

        <!-- 工具菜单 -->
        <div class="menu-section">
          <p v-if="!isCollapsed" class="section-title">工具</p>
          <div v-for="item in toolsMenuItems" :key="item.label" class="menu-item" @click="item.action">
            <el-icon :size="20"><component :is="item.icon" /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">{{ item.label }}</span>
            </transition>
          </div>
        </div>
      </el-scrollbar>

      <!-- 底部操作 -->
      <div class="sidebar-footer">
        <div
          v-for="item in footerMenuItems"
          :key="item.label"
          class="menu-item"
          :class="item.class"
          @click="item.action"
        >
          <el-icon :size="20"><component :is="item.icon" /></el-icon>
          <transition name="fade">
            <span v-if="!isCollapsed">{{ item.label }}</span>
          </transition>
        </div>
      </div>
    </aside>

    <!-- 主内容区域 -->
    <div class="main-content" :class="{ expanded: isCollapsed }">
      <!-- 顶部拖动区域 -->
      <div class="drag-region" data-tauri-drag-region></div>

      <!-- Windows 经典窗口控制按钮 -->
      <div class="window-controls">
        <el-tooltip content="最小化" placement="bottom">
          <div class="control-btn minimize" @click="minimizeWindow">
            <img class="control-icon" src="../public/icons/btn-minimize-buoy.svg" alt="最小化" width="20" height="20" />
          </div>
        </el-tooltip>

        <el-tooltip :content="isMaximized ? '还原' : '最大化'" placement="bottom">
          <div class="control-btn maximize" @click="toggleMaximize">
            <img
              :class="['control-icon']"
              :src="isMaximized ? '../public/icons/btn-restore-buoy.svg' : '../public/icons/btn-maximize-buoy.svg'"
              :alt="isMaximized ? '还原' : '最大化'"
              width="20"
              height="20"
            />
          </div>
        </el-tooltip>

        <el-tooltip content="关闭" placement="bottom">
          <div class="control-btn close" @click="closeWindow">
            <img class="control-icon" src="../public/icons/btn-close-buoy.svg" alt="关闭" width="20" height="20" />
          </div>
        </el-tooltip>
      </div>

      <!-- 页面内容 -->
      <div class="page-content">
        <!-- 动态路由组件 -->
        <VddSettings v-if="router.isRoute(ROUTES.VDD_SETTINGS)" @close="goHome" />
        <Welcome v-if="router.isRoute(ROUTES.WELCOME)" @close="goHome" />

        <!-- 默认内容 (slot) -->
        <slot v-if="router.isRoute(ROUTES.HOME)" />

        <!-- 更新对话框 -->
        <UpdateDialog
          v-if="showUpdateDialog"
          v-model="showUpdateDialog"
          :update-info="updateInfo"
          :current-version="currentVersion"
          @close="showUpdateDialog = false"
          @skip-version="handleSkipVersion"
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, onMounted } from 'vue'
import VddSettings from './VddSettings.vue'
import Welcome from './welcome.vue'
import UpdateDialog from './UpdateDialog.vue'
import { useSidebarState } from '../composables/useSidebarState.js'
import { useWindowControls } from '../composables/useWindowControls.js'
import { useTools } from '../composables/useTools.js'
import { ROUTES } from '../composables/useRouter.js'
import {
  Monitor,
  Delete,
  RefreshRight,
  Refresh,
  Link,
  Setting,
  CopyDocument,
  Timer,
  DataLine,
  Cpu,
  Minus,
  Close,
  Sunny,
  Moon,
  Key,
  Download,
} from '@element-plus/icons-vue'

// Composables
const {
  isCollapsed,
  isDark,
  isMaximized,
  isAdmin,
  showUpdateDialog,
  updateInfo,
  currentVersion,
  router,
  toggleTheme,
  toggleCollapse,
  openVddSettings,
  openWelcome,
  goHome,
  skipVersion,
} = useSidebarState()

const { minimizeWindow, toggleMaximize, closeWindow } = useWindowControls(isMaximized)

const {
  uninstallVdd,
  restartDriver,
  restartSunshine,
  openTimer,
  openUrl,
  cleanupCovers,
  restartAsAdmin,
  checkForUpdates,
} = useTools()

const includePrerelease = ref(false)

onMounted(async () => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    includePrerelease.value = await invoke('get_include_prerelease_preference')
  } catch (error) {
    console.error('加载内测偏好设置失败:', error)
  }
})

const handlePrereleaseToggle = async (value) => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('set_include_prerelease_preference', { include: value })
  } catch (error) {
    console.error('保存内测偏好设置失败:', error)
  }
}

const handleCheckForUpdates = async () => {
  const result = await checkForUpdates()
  if (result) {
    updateInfo.value = result
    showUpdateDialog.value = true
  }
}

const handleSkipVersion = (version) => skipVersion(version)

// 菜单配置
const managementMenuItems = computed(() => [
  { icon: Setting, label: '高级设置', action: goHome, isActive: () => router.isRoute(ROUTES.HOME) },
  { icon: Monitor, label: '虚拟显示器', action: openVddSettings, isActive: () => router.isRoute(ROUTES.VDD_SETTINGS) },
  { icon: Delete, label: '卸载 VDD', action: uninstallVdd },
  { icon: RefreshRight, label: '重启显卡驱动', action: restartDriver },
  { icon: Refresh, label: '重启 Sunshine', action: restartSunshine },
  { icon: Download, label: '检查更新', action: handleCheckForUpdates, hasSwitch: true },
])

const toolsMenuItems = [
  { icon: Link, label: '官方网站', action: () => openUrl('https://sunshine-foundation.vercel.app/') },
  { icon: Timer, label: '串流计时器', action: openTimer },
  { icon: DataLine, label: '延迟测试', action: () => openUrl('https://yangkile.github.io/D-lay/') },
  { icon: Cpu, label: '手柄测试', action: () => openUrl('https://hardwaretester.com/gamepad') },
  { icon: CopyDocument, label: '剪贴板同步', action: () => openUrl('https://gcopy.rutron.net/zh') },
  { icon: Delete, label: '清理临时文件', action: cleanupCovers },
]

const footerMenuItems = computed(() => {
  const items = [
    { icon: isDark.value ? Sunny : Moon, label: isDark.value ? '浅色模式' : '深色模式', action: toggleTheme },
    { icon: Minus, label: '最小化', action: minimizeWindow },
    { icon: Close, label: '隐藏窗口', action: closeWindow, class: 'danger' },
  ]
  if (!isAdmin.value) {
    items.push({ icon: Key, label: '以管理员重启', action: restartAsAdmin, class: 'warning' })
  }
  return items
})

// 暴露方法供父组件调用
defineExpose({
  openVddSettings,
  openWelcome,
  goHome,
  router,
})
</script>

<style scoped lang="less">
@import '../styles/SidebarMenu.less';
</style>
