<template>
  <div class="sidebar-wrapper">
    <!-- 侧边栏 -->
    <aside class="sidebar" :class="{ collapsed: isCollapsed }">
      <!-- Gura 背景装饰 -->
      <div class="gura-background">
        <img src="../public/gura-pix.png" alt="Gura" class="gura-bg-img" />
      </div>

      <!-- Logo 区域 (可拖动) -->
      <div
        class="sidebar-header"
        data-tauri-drag-region
        @pointerdown="onTouchWindowDragStart"
      >
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
          <div
            v-for="item in toolsMenuItems"
            :key="item.label"
            class="menu-item"
            :class="{ active: item.isActive?.() }"
            @click.stop="item.action?.()"
          >
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
      <div
        class="drag-region"
        data-tauri-drag-region
        @pointerdown="onTouchWindowDragStart"
      ></div>

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
        <DualSenseSettings v-if="router.isRoute(ROUTES.DUALSENSE)" @close="goHome" />

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
import { computed, defineAsyncComponent, ref, watch, onMounted } from 'vue'
import VddSettings from './VddSettings.vue'
import Welcome from './welcome.vue'
import WebStreamSettings from './WebStreamSettings.vue'
import AiAssistant from './AiAssistant.vue'
import DualSenseSettings from './DualSenseSettings.vue'
const UpdateDialog = defineAsyncComponent(() => import('./UpdateDialog.vue'))
import { useSidebarState } from '../composables/useSidebarState.js'
import { useWindowControls } from '../composables/useWindowControls.js'
import { useTools } from '../composables/useTools.js'
import { useTouchWindowDrag } from '../composables/useTouchWindowDrag.js'
import {
  createManagementTools,
  createUtilityTools,
  createFooterTools,
} from '../composables/toolsRegistry.js'
import { ROUTES } from '../composables/useRouter.js'
import { useI18n } from '../desktop/i18n/index.js'

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
  openDualSense,
  goHome,
  skipVersion,
  includePrerelease,
  setIncludePrerelease,
} = useSidebarState()

const { minimizeWindow, toggleMaximize, closeWindow } = useWindowControls(isMaximized)
const { onTouchWindowDragStart } = useTouchWindowDrag(isMaximized)

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
  showClipboardSyncStatus,
  initClipboardSyncStatus,
  clipboardSyncEnabled,
} = useTools()

// Read the agent's current state once on mount so the sidebar reflects
// any sync that survives across panel reloads (it doesn't currently, but the
// hook is there for future persistence).
onMounted(() => {
  initClipboardSyncStatus()
})

const handleCheckForUpdates = async (channel = null) => {
  const result = await checkForUpdates(channel)
  if (result) {
    updateInfo.value = result
    showUpdateDialog.value = true
    return true
  }
  return false
}

const handleSkipVersion = (version) => skipVersion(version)

// 菜单配置：从 toolsRegistry 读取（新增/修改菜单只需改那个文件）
const toolsCtx = {
  t,
  locale,
  router,
  isDark,
  isAdmin,
  toggleTheme,
  toggleLocale,
  minimizeWindow,
  closeWindow,
  goHome,
  openVddSettings,
  openWebStream,
  openAiAssistant,
  openDualSense,
  handleCheckForUpdates,
  openTimer,
  openUrl,
  openGamepadTest,
  cleanupCovers,
  restartAsAdmin,
  showClipboardSyncStatus,
  clipboardSyncEnabled,
}

const managementMenuItems = computed(() => createManagementTools(toolsCtx))
const toolsMenuItems = computed(() => createUtilityTools(toolsCtx))
const footerMenuItems = computed(() => createFooterTools(toolsCtx))

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
  openDualSense,
  goHome,
  checkForUpdates: handleCheckForUpdates,
  router,
})
</script>

<style scoped lang="less">
@import '../styles/SidebarMenu.less';
</style>
