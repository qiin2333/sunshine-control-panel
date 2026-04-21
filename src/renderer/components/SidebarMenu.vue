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
          <h3 v-if="!isCollapsed" class="app-name">Foundation Sunshine</h3>
        </transition>
      </div>

      <!-- 折叠按钮 -->
      <div class="collapse-btn" @click="toggleCollapse" :aria-label="t.sidebar.collapse">
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
          <p v-if="!isCollapsed" class="section-title">{{ t.sidebar.sectionManage }}</p>
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
                    @change="setIncludePrerelease"
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
          <p v-if="!isCollapsed" class="section-title">{{ t.sidebar.sectionTools }}</p>
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
        <el-tooltip :content="t.sidebar.minimize" placement="bottom">
          <div class="control-btn minimize" @click="minimizeWindow">
            <img class="control-icon" src="../public/icons/btn-minimize-buoy.svg" :alt="t.sidebar.minimize" width="20" height="20" />
          </div>
        </el-tooltip>

        <el-tooltip :content="isMaximized ? t.sidebar.restore : t.sidebar.maximize" placement="bottom">
          <div class="control-btn maximize" @click="toggleMaximize">
            <img
              v-if="isMaximized"
              class="control-icon"
              src="../public/icons/btn-restore-buoy.svg"
              :alt="t.sidebar.restore"
              width="20"
              height="20"
            />
            <img
              v-else
              class="control-icon"
              src="../public/icons/btn-maximize-buoy.svg"
              :alt="t.sidebar.maximize"
              width="20"
              height="20"
            />
          </div>
        </el-tooltip>

        <el-tooltip :content="t.sidebar.close" placement="bottom">
          <div class="control-btn close" @click="closeWindow">
            <img class="control-icon" src="../public/icons/btn-close-buoy.svg" :alt="t.sidebar.close" width="20" height="20" />
          </div>
        </el-tooltip>
      </div>

      <!-- 页面内容 -->
      <div class="page-content">
        <!-- 动态路由组件 -->
        <VddSettings v-if="router.isRoute(ROUTES.VDD_SETTINGS)" @close="goHome" />
        <Welcome v-if="router.isRoute(ROUTES.WELCOME)" @close="goHome" />
        <WebStreamSettings v-if="router.isRoute(ROUTES.WEB_STREAM)" @close="goHome" />
        <AiAssistant v-if="router.isRoute(ROUTES.AI_ASSISTANT)" @close="goHome" />

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
import { computed, ref, watch } from 'vue'
import VddSettings from './VddSettings.vue'
import Welcome from './welcome.vue'
import WebStreamSettings from './WebStreamSettings.vue'
import AiAssistant from './AiAssistant.vue'
import UpdateDialog from './UpdateDialog.vue'
import { useSidebarState } from '../composables/useSidebarState.js'
import { useWindowControls } from '../composables/useWindowControls.js'
import { useTools } from '../composables/useTools.js'
import { ROUTES } from '../composables/useRouter.js'
import { useI18n } from '../desktop/i18n/index.js'
import IconLang from '../desktop/icons/IconLang.vue'
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
  Connection,
  MagicStick,
} from '@element-plus/icons-vue'

const emit = defineEmits(['route-change'])

const { t, locale, toggleLocale } = useI18n()

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
  openWebStream,
  openAiAssistant,
  goHome,
  skipVersion,
  includePrerelease,
  setIncludePrerelease,
} = useSidebarState()

const { minimizeWindow, toggleMaximize, closeWindow } = useWindowControls(isMaximized)

const {
  uninstallVdd,
  restartDriver,
  restartSunshine,
  restartSunshineInUserMode,
  openTimer,
  openUrl,
  cleanupCovers,
  restartAsAdmin,
  checkForUpdates,
  openGamepadTest,
} = useTools()

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
  { icon: Setting, label: t.value.sidebar.advancedSettings, action: goHome, isActive: () => router.isRoute(ROUTES.HOME) },
  { icon: Monitor, label: t.value.sidebar.virtualDisplay, action: openVddSettings, isActive: () => router.isRoute(ROUTES.VDD_SETTINGS) },
  ...(import.meta.env.DEV ? [{ icon: Connection, label: t.value.sidebar.webStream, action: openWebStream, isActive: () => router.isRoute(ROUTES.WEB_STREAM) }] : []),
  { icon: MagicStick, label: t.value.sidebar.aiAssistant, action: openAiAssistant, isActive: () => router.isRoute(ROUTES.AI_ASSISTANT) },
  // { icon: Delete, label: t.value.sidebar.uninstallVdd, action: uninstallVdd },
  // { icon: RefreshRight, label: t.value.sidebar.restartGpu, action: restartDriver },
  // { icon: Refresh, label: '使用WGC捕获', action: restartSunshineInUserMode },
  { icon: Download, label: t.value.sidebar.checkUpdate, action: handleCheckForUpdates, hasSwitch: true },
])

const toolsMenuItems = computed(() => [
  { icon: Link, label: t.value.sidebar.officialWebsite, action: () => openUrl('https://www.alkaidlab.com/') },
  { icon: Timer, label: t.value.sidebar.streamTimer, action: openTimer },
  { icon: DataLine, label: t.value.sidebar.latencyTest, action: () => openUrl('https://yangkile.github.io/D-lay/') },
  { icon: Cpu, label: t.value.sidebar.gamepadTest, action: openGamepadTest },
  { icon: CopyDocument, label: t.value.sidebar.clipboardSync, action: () => openUrl('https://gcopy.rutron.net/zh') },
  { icon: Delete, label: t.value.sidebar.cleanTemp, action: cleanupCovers },
])

const footerMenuItems = computed(() => {
  const items = [
    { icon: isDark.value ? Sunny : Moon, label: isDark.value ? t.value.sidebar.lightMode : t.value.sidebar.darkMode, action: toggleTheme },
    { icon: IconLang, label: locale.value === 'zh' ? 'EN' : '中文', action: toggleLocale },
    { icon: Minus, label: t.value.sidebar.minimize, action: minimizeWindow },
    { icon: Close, label: t.value.sidebar.hideWindow, action: closeWindow, class: 'danger' },
  ]
  if (!isAdmin.value) {
    items.push({ icon: Key, label: t.value.sidebar.restartAsAdmin, action: restartAsAdmin, class: 'warning' })
  }
  return items
})

// 路由变化时通知父组件（用于 iframe 休眠/唤醒）
watch(() => router.currentRoute.value, (newRoute, oldRoute) => {
  if (newRoute !== oldRoute) {
    emit('route-change', { from: oldRoute, to: newRoute })
  }
})

// 暴露方法供父组件调用
defineExpose({
  openVddSettings,
  openWelcome,
  openWebStream,
  openAiAssistant,
  goHome,
  router,
})
</script>

<style scoped lang="less">
@import '../styles/SidebarMenu.less';
</style>
