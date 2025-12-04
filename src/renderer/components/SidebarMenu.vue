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

          <div class="menu-item" :class="{ active: !showVddSettings }" @click="showVddSettings = false">
            <el-icon :size="20"><Setting /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">高级设置</span>
            </transition>
          </div>

          <div class="menu-item" :class="{ active: showVddSettings }" @click="openVddSettings">
            <el-icon :size="20"><Monitor /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">虚拟显示器</span>
            </transition>
          </div>

          <div class="menu-item" @click="uninstallVdd">
            <el-icon :size="20"><Delete /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">卸载 VDD</span>
            </transition>
          </div>

          <div class="menu-item" @click="restartDriver">
            <el-icon :size="20"><RefreshRight /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">重启显卡驱动</span>
            </transition>
          </div>

          <div class="menu-item" @click="restartSunshine">
            <el-icon :size="20"><Refresh /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">重启 Sunshine</span>
            </transition>
          </div>

          <div class="menu-item" @click="handleCheckForUpdates">
            <el-icon :size="20"><Download /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">检查更新</span>
            </transition>
          </div>
        </div>

        <div class="menu-section">
          <p v-if="!isCollapsed" class="section-title">工具</p>

          <div class="menu-item" @click="openUrl('https://sunshine-foundation.vercel.app/')">
            <el-icon :size="20"><Link /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">官方网站</span>
            </transition>
          </div>

          <div class="menu-item" @click="openTimer">
            <el-icon :size="20"><Timer /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">串流计时器</span>
            </transition>
          </div>

          <div class="menu-item" @click="openUrl('https://yangkile.github.io/D-lay/')">
            <el-icon :size="20"><DataLine /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">延迟测试</span>
            </transition>
          </div>

          <div class="menu-item" @click="openUrl('https://hardwaretester.com/gamepad')">
            <el-icon :size="20"><Cpu /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">手柄测试</span>
            </transition>
          </div>

          <div class="menu-item" @click="openUrl('https://gcopy.rutron.net/zh')">
            <el-icon :size="20"><CopyDocument /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">剪贴板同步</span>
            </transition>
          </div>

          <div class="menu-item" @click="cleanupCovers">
            <el-icon :size="20"><Delete /></el-icon>
            <transition name="fade">
              <span v-if="!isCollapsed">清理临时文件</span>
            </transition>
          </div>
        </div>
      </el-scrollbar>

      <!-- 底部操作 -->
      <div class="sidebar-footer">
        <!-- 主题切换 -->
        <div class="menu-item" @click="toggleTheme">
          <el-icon :size="20">
            <Sunny v-if="isDark" />
            <Moon v-else />
          </el-icon>
          <transition name="fade">
            <span v-if="!isCollapsed">{{ isDark ? '浅色模式' : '深色模式' }}</span>
          </transition>
        </div>

        <div class="menu-item" @click="minimizeWindow">
          <el-icon :size="20"><Minus /></el-icon>
          <transition name="fade">
            <span v-if="!isCollapsed">最小化</span>
          </transition>
        </div>

        <div class="menu-item danger" @click="closeWindow">
          <el-icon :size="20"><Close /></el-icon>
          <transition name="fade">
            <span v-if="!isCollapsed">隐藏窗口</span>
          </transition>
        </div>

        <div v-if="!isAdmin" class="menu-item warning" @click="restartAsAdmin">
          <el-icon :size="20"><Key /></el-icon>
          <transition name="fade">
            <span v-if="!isCollapsed">以管理员重启</span>
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
              v-if="isMaximized"
              class="control-icon"
              src="../public/icons/btn-restore-buoy.svg"
              alt="还原"
              width="20"
              height="20"
            />
            <img
              v-else
              class="control-icon"
              src="../public/icons/btn-maximize-buoy.svg"
              alt="最大化"
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
        <VddSettings v-if="showVddSettings" @close="showVddSettings = false" />
        <UpdateDialog
          v-if="showUpdateDialog"
          v-model="showUpdateDialog"
          :update-info="updateInfo"
          :current-version="currentVersion"
          @close="showUpdateDialog = false"
          @skip-version="handleSkipVersion"
        />
        <slot v-if="!showVddSettings && !showUpdateDialog"></slot>
      </div>
    </div>
  </div>
</template>

<script setup>
import VddSettings from './VddSettings.vue'
import UpdateDialog from './UpdateDialog.vue'
import { useSidebarState } from '../composables/useSidebarState.js'
import { useWindowControls } from '../composables/useWindowControls.js'
import { useTools } from '../composables/useTools.js'
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
  FullScreen,
  DArrowLeft,
  DArrowRight,
  Sunny,
  Moon,
  Key,
  Download,
} from '@element-plus/icons-vue'

// 使用 composables
const {
  isCollapsed,
  isDark,
  isMaximized,
  isAdmin,
  showVddSettings,
  showUpdateDialog,
  updateInfo,
  currentVersion,
  toggleTheme,
  toggleCollapse,
  openVddSettings,
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

// 处理检查更新的结果
const handleCheckForUpdates = async () => {
  const result = await checkForUpdates()
  if (result) {
    updateInfo.value = result
    showUpdateDialog.value = true
  }
}

// 处理忽略版本
const handleSkipVersion = (version) => {
  skipVersion(version)
}

// 暴露方法供父组件调用
defineExpose({
  openVddSettings,
})
</script>

<style scoped lang="less">
@import '../styles/SidebarMenu.less';
</style>
