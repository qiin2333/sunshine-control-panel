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
        <slot v-else></slot>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import VddSettings from './VddSettings.vue'
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
} from '@element-plus/icons-vue'
import { openExternalUrl, tools } from '@/tauri-adapter.js'

const isCollapsed = ref(false)
const isDark = ref(true)
const isMaximized = ref(false)
const isAdmin = ref(true)
const showVddSettings = ref(false) // 控制 VDD 设置页面显示

// 初始化时检测系统主题和窗口状态
onMounted(async () => {
  const body = document.querySelector('body')

  // 检测是否以管理员权限运行
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const adminStatus = await invoke('is_running_as_admin')
    isAdmin.value = adminStatus
    if (!adminStatus) {
      console.log('⚠️  当前未以管理员权限运行')
    } else {
      console.log('✅ 当前以管理员权限运行')
    }
  } catch (error) {
    console.error('检测管理员权限失败:', error)
  }

  // 首先从 localStorage 读取保存的主题
  const savedTheme = localStorage.getItem('sunshine-theme')
  if (savedTheme) {
    isDark.value = savedTheme === 'dark'
    body?.setAttribute('data-bs-theme', savedTheme)
  } else {
    const currentTheme = body?.getAttribute('data-bs-theme')
    isDark.value = currentTheme === 'dark' || currentTheme === null

    // 同步系统主题
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    if (!currentTheme) {
      isDark.value = prefersDark
      body?.setAttribute('data-bs-theme', prefersDark ? 'dark' : 'light')
    }
  }

  // 检测窗口是否已经最大化
  try {
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const window = getCurrentWebviewWindow()
    isMaximized.value = await window.isMaximized()
  } catch (error) {
    console.error('检测窗口状态失败:', error)
  }

  // 监听来自 iframe 的主题请求
  window.addEventListener('message', (event) => {
    // 安全检查：只接受来自 localhost 的消息
    if (event.origin.includes('localhost') || event.origin.includes('127.0.0.1')) {
      if (event.data.type === 'request-theme') {
        // 回复当前主题
        const iframe = document.querySelector('iframe')
        if (iframe && iframe.contentWindow) {
          iframe.contentWindow.postMessage(
            {
              type: 'theme-sync',
              theme: isDark.value ? 'dark' : 'light',
            },
            '*'
          )
        }
      }
    }
  })
})

const toggleTheme = () => {
  isDark.value = !isDark.value
  const body = document.querySelector('body')
  if (body) {
    body.setAttribute('data-bs-theme', isDark.value ? 'dark' : 'light')
  }

  // 保存主题偏好
  localStorage.setItem('sunshine-theme', isDark.value ? 'dark' : 'light')

  // 向所有 iframe 发送主题变化消息
  const iframes = document.querySelectorAll('iframe')
  iframes.forEach((iframe) => {
    try {
      if (iframe.contentWindow) {
        iframe.contentWindow.postMessage(
          {
            type: 'theme-sync',
            theme: isDark.value ? 'dark' : 'light',
          },
          '*'
        )
      }
    } catch (error) {
      console.log('无法向 iframe 发送主题消息（跨域限制）')
    }
  })

  ElMessage.success(isDark.value ? '已切换到深色模式' : '已切换到浅色模式')
}

const toggleCollapse = () => {
  isCollapsed.value = !isCollapsed.value
}

/**
 * 清理无用的封面图片和临时文件
 */
const cleanupCovers = async () => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    
    // 首先检查是否以管理员权限运行
    const isRunningAsAdmin = await invoke('is_running_as_admin')
    
    if (!isRunningAsAdmin) {
      // 不是管理员，提示重启
      await ElMessageBox.confirm('清理临时文件需要管理员权限。\n\n是否以管理员身份重启应用？', '需要管理员权限', {
          confirmButtonText: '以管理员重启',
          cancelButtonText: '取消',
          type: 'warning',
      })
      
      // 用户确认后，调用重启为管理员
      await restartAsAdmin()
      return
    }
    
    // 已经是管理员，继续执行清理
    await ElMessageBox.confirm(
      '此操作将删除：\n1. 未被应用使用的封面图片\n2. config 目录下的 temp_ 临时文件\n\n是否继续？',
      '清理无用文件',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )

    // 显示加载提示
    const loading = ElMessage({
      message: '正在清理无用文件...',
      type: 'info',
      duration: 0,
    })

    // 调用 Tauri 命令
    const result = await invoke('cleanup_unused_covers')

    loading.close()

    // 显示结果
    if (result.success) {
      if (result.deleted_count > 0) {
        ElMessageBox.alert(
          `${result.message}\n\n删除的文件数: ${result.deleted_count}\n释放的空间: ${(
            result.freed_space / 1024
          ).toFixed(2)} KB`,
          '清理完成',
          {
            confirmButtonText: '确定',
            type: 'success',
          }
        )
      } else {
        ElMessage.success(result.message)
      }
    } else {
      ElMessage.error('清理失败: ' + result.message)
    }
  } catch (error) {
    if (error !== 'cancel') {
      console.error('清理文件失败:', error)
      ElMessage.error('清理文件失败: ' + error)
    }
  }
}

/**
 * 以管理员权限重启 GUI
 */
const restartAsAdmin = async () => {
  try {
    // 确认对话框
    await ElMessageBox.confirm('将以管理员权限重启应用，当前窗口会关闭。是否继续？', '提升权限', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })

    // 显示提示
    ElMessage.info('正在请求管理员权限...')

    // 调用 Tauri 命令
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('restart_as_admin')

    // 如果到这里说明成功请求了重启
    ElMessage.success('正在以管理员权限重启...')
  } catch (error) {
    if (error !== 'cancel') {
      console.error('重启失败:', error)
      ElMessage.error('重启失败: ' + error)
    }
  }
}

/**
 * 公共窗口创建函数
 * @param {string} url - 窗口URL路径
 * @param {string} title - 窗口标题
 * @param {object} options - 窗口配置选项
 */
const createWindow = async (url, title, options = {}) => {
  try {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const baseUrl = window.location.origin
    const windowId = `${options.prefix || 'window'}_${Date.now()}`

    const newWindow = new WebviewWindow(windowId, {
      url: `${baseUrl}${url}`,
      title,
      width: options.width || 1080,
      height: options.height || 800,
      decorations: options.decorations !== false,
      center: true,
    })

    // 等待窗口创建完成后显示
    newWindow.once('tauri://created', async () => {
      console.log(`✅ ${title}窗口已创建`)
      await newWindow.show()
      await newWindow.setFocus()
      console.log(`✅ ${title}窗口已显示`)
    })

    newWindow.once('tauri://error', (e) => {
      console.error(`❌ ${title}窗口创建失败:`, e)
      ElMessage.error(`${title}窗口创建失败`)
    })
  } catch (error) {
    console.error(`❌ 打开${title}失败:`, error)
    ElMessage.error(`打开${title}失败: ${error.message}`)
  }
}

const openVddSettings = () => {
  showVddSettings.value = true
}

/**
 * 公共确认对话框操作
 * @param {string} message - 确认消息
 * @param {string} title - 对话框标题
 * @param {function} action - 执行的操作
 * @param {string} successMsg - 成功消息
 */
const confirmAction = async (message, title, action, successMsg) => {
  try {
    await ElMessageBox.confirm(message, title, {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })
    await action()
    ElMessage.success(successMsg)
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(`操作失败: ${error}`)
    }
  }
}

const uninstallVdd = async () => {
  await confirmAction(
    '确定要卸载虚拟显示器驱动吗？此操作需要管理员权限。',
    '确认卸载',
    tools.uninstallVddDriver,
    '卸载请求已发送'
  )
}

const restartDriver = async () => {
  await confirmAction(
    '确定要重启显卡驱动吗？这将暂时中断屏幕显示。',
    '确认重启',
    tools.restartGraphicsDriver,
    '重启请求已发送'
  )
}

const restartSunshine = async () => {
  await confirmAction(
    '确定要重启 Sunshine 服务吗？这将断开当前所有连接。',
    '确认重启',
    tools.restartSunshineService,
    '重启请求已发送'
  )
}

const openTimer = async () => {
  await createWindow('/stop-clock-canvas/index.html', '串流计时器', {
    prefix: 'timer',
    width: 1080,
    height: 600,
  })
}

const openUrl = async (url) => {
  await openExternalUrl(url)
}

/**
 * 执行窗口操作
 * @param {string} action - 操作名称 (minimize/hide)
 * @param {string} actionName - 操作显示名称
 */
const performWindowAction = async (action, actionName) => {
  try {
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const currentWindow = getCurrentWebviewWindow()
    await currentWindow[action]()
    console.log(`✅ 窗口已${actionName}`)
  } catch (error) {
    console.error(`${actionName}窗口失败:`, error)
    ElMessage.error(`${actionName}失败: ${error.message}`)
  }
}

const minimizeWindow = async () => {
  await performWindowAction('minimize', '最小化')
}

const toggleMaximize = async () => {
  try {
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const window = getCurrentWebviewWindow()

    const maximized = await window.isMaximized()

    if (maximized) {
      await window.unmaximize()
      isMaximized.value = false
      console.log('✅ 窗口已还原')
    } else {
      await window.maximize()
      isMaximized.value = true
      console.log('✅ 窗口已最大化')
    }
  } catch (error) {
    console.error('❌ 切换最大化失败:', error)
    ElMessage.error(`切换最大化失败: ${error}`)
  }
}

const closeWindow = async () => {
  await performWindowAction('hide', '隐藏')
}

// 暴露方法供父组件调用
defineExpose({
  openVddSettings,
})
</script>

<style scoped lang="less">
@import '../styles/theme.less';

// ========== 主容器 ==========
.sidebar-wrapper {
  display: flex;
  width: 100%;
  height: 100vh;
  overflow: hidden;
  background: linear-gradient(135deg, @morandi-dark-bg 0%, @morandi-mid-bg 50%, @morandi-light-bg 100%);
  border-radius: @border-radius;
}

.gura-background {
  position: absolute;
  bottom: 0;
  right: 0;
  width: 160px;
  height: 160px;
  pointer-events: none;
  z-index: 0;
  opacity: 0.15;
  transition: all 0.3s ease;
  overflow: hidden;

  .gura-bg-img {
    width: 160px;
    height: 160px;
    object-fit: contain;
    transform: rotate(-15deg);
    image-rendering: pixelated;
    image-rendering: crisp-edges;
    position: absolute;
    bottom: -40px;
    right: -28px;
  }
}

// ========== 侧边栏 ==========
.sidebar {
  width: @sidebar-width;
  height: 100vh;
  background: linear-gradient(180deg, @morandi-mid-bg 0%, @morandi-dark-bg 100%);
  display: flex;
  flex-direction: column;
  position: relative;
  transition: width @transition-smooth, box-shadow @transition-smooth;
  .shadow-sidebar-dark();
  z-index: 1000;
  border-radius: @border-radius 0 0 @border-radius;

  &.collapsed {
    width: @sidebar-collapsed-width;
    .shadow-sidebar-dark-collapsed();
  }
}

// ========== Logo 区域 ==========
.sidebar-header {
  padding: 24px 20px;
  display: flex;
  align-items: center;
  gap: 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  margin-bottom: 20px;
  position: relative;
  z-index: 10; // 确保在 Gura 背景上方
}

.logo {
  width: 40px;
  height: 40px;
  .morandi-gradient();
  border-radius: @border-radius;
  .flex-center();
  color: white;
  flex-shrink: 0;
  overflow: hidden;
  padding: 6px;
  box-shadow: 0 2px 8px rgba(230, 213, 184, 0.2);
}

.logo-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  animation: logoFloat 3s ease-in-out infinite;
}

@keyframes logoFloat {
  0%,
  100% {
    transform: translateY(0) scale(1);
  }
  50% {
    transform: translateY(-3px) scale(1.05);
  }
}

.app-name {
  color: @morandi-yellow;
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  white-space: nowrap;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

// ========== 折叠按钮 ==========
.collapse-btn {
  position: absolute;
  right: -10px;
  top: 76px;
  padding: 0;
  background: transparent;
  border: none;
  width: auto;
  height: auto;
  transform: none;
  .flex-center();
  cursor: pointer;
  z-index: 20; // 确保在 Gura 背景上方
  transition: none;
  box-shadow: none;

  &::before,
  &::after {
    content: none;
    display: none;
  }

  .clip-icon {
    width: 26px;
    height: 26px;
    transform: rotate(90deg);
    transform-origin: 50% 50%;
    transition: transform @transition-fast, filter @transition-fast;
  }

  &:hover,
  &:active {
    transform: none;
    box-shadow: none;
  }
}

// 发卡交互：悬停放大、按下压缩；根据收起状态切换方向
.collapse-btn:hover .clip-icon {
  transform: rotate(90deg) scale(1.06);
  filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.25));
}

.collapse-btn:active .clip-icon {
  transform: rotate(90deg) scale(0.96);
}

.clip-icon.collapsed {
  transform: rotate(-90deg);
}

// ========== 菜单区域 ==========
.menu-scrollbar {
  flex: 1;
  padding: 0 12px;
  position: relative;
  z-index: 10; // 确保在 Gura 背景上方

  :deep(.el-scrollbar__wrap) {
    overflow-x: hidden;
  }
}

.menu-section {
  margin-bottom: 24px;
}

.section-title {
  color: #d4c4a8;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 1px;
  margin: 0 0 12px 12px;
  transition: opacity @transition-smooth;
}

// ========== 菜单项 ==========
.menu-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  margin-bottom: 8px;
  border-radius: @border-radius;
  color: @morandi-yellow;
  cursor: pointer;
  transition: all @transition-smooth;
  position: relative;
  overflow: hidden;
  user-select: none;

  // 左侧装饰条
  &::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    width: 3px;
    height: 100%;
    background: linear-gradient(180deg, @morandi-red 0%, @morandi-yellow 100%);
    opacity: 0;
    transition: opacity @transition-smooth;
  }

  // 悬停效果
  &:hover {
    background: rgba(212, 165, 165, 0.15);
    color: #f0e5cc;
    transform: translateX(4px);
    .shadow-menu-item-dark();

    &::before {
      opacity: 1;
    }

    .el-icon {
      transform: scale(1.1);
      filter: drop-shadow(0 2px 4px rgba(230, 213, 184, 0.3));
    }
  }

  // 按下效果
  &:active {
    transform: translateX(2px) scale(0.98);
  }

  // 危险操作样式
  &.danger:hover {
    background: rgba(212, 165, 165, 0.25);
    color: @morandi-red;
  }

  &.warning {
    color: @gura-yellow;

    &:hover {
      background: rgba(255, 193, 7, 0.15);
      color: #ff9800;
    }
  }

  // 活动状态样式
  &.active {
    background: linear-gradient(90deg, rgba(212, 165, 165, 0.3), rgba(230, 213, 184, 0.2));
    border-left: 3px solid @morandi-red;
    color: @morandi-yellow;
    font-weight: 600;
    .shadow-menu-item-dark();

    &::before {
      opacity: 1;
      background: linear-gradient(90deg, rgba(212, 165, 165, 0.3), transparent);
    }

    .el-icon {
      color: @morandi-red;
      transform: scale(1.15);
      filter: drop-shadow(0 2px 6px rgba(212, 165, 165, 0.5));
    }

    &:hover {
      background: linear-gradient(90deg, rgba(212, 165, 165, 0.4), rgba(230, 213, 184, 0.25));
    }
  }

  // 图标
  .el-icon {
    flex-shrink: 0;
    transition: all @transition-smooth;
  }

  // 文本
  span {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
  }
}

// ========== 底部区域 ==========
.sidebar-footer {
  padding: 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  margin-top: auto;
  position: relative;
  z-index: 10; // 确保在 Gura 背景上方
}

// ========== 主内容区域 ==========
.main-content {
  flex: 1;
  width: calc(100% - @sidebar-width);
  height: 100vh;
  display: flex;
  flex-direction: column;
  position: relative;
  transition: width @transition-smooth, box-shadow @transition-smooth;
  background: linear-gradient(135deg, @morandi-mid-bg 0%, @morandi-light-bg 100%);
  overflow: hidden;
  border-radius: 0 @border-radius @border-radius 0;
  .shadow-content-dark();

  &.expanded {
    width: calc(100% - @sidebar-collapsed-width);
  }
}

// ========== 顶部拖动区域 ==========
.drag-region {
  position: absolute;
  top: 0;
  left: 0;
  right: 80px; // 右侧留空给浮动按钮
  height: 20px;
  z-index: 100;
  cursor: move;
  pointer-events: auto;

  // 调试模式可视化（可选，生产环境可移除）
  &:hover {
    background: rgba(230, 213, 184, 0.03);
  }
}

// ========== Windows 经典窗口控制按钮 ==========
.window-controls {
  position: absolute;
  top: 0;
  right: 0;
  display: flex;
  z-index: 10000;
  pointer-events: auto;
}

.control-btn {
  width: 46px;
  height: 32px;
  .flex-center();
  cursor: pointer;
  transition: all 0.15s ease;
  border: none;
  background: transparent;

  .el-icon,
  .control-icon {
    // 多重阴影让图标看起来更粗
    filter: drop-shadow(0 0.5px 0 currentColor) drop-shadow(0.5px 0 0 currentColor) drop-shadow(0 -0.5px 0 currentColor)
      drop-shadow(-0.5px 0 0 currentColor);
    font-weight: bold;
    transform: scale(1.05);
    transition: transform 0.15s ease;
  }

  &.minimize {
    color: @morandi-yellow;

    &:hover {
      background: rgba(230, 213, 184, 0.25);
      color: #fff8e7;

      .el-icon,
      .control-icon {
        transform: scale(1.15);
      }
    }

    &:active {
      background: rgba(230, 213, 184, 0.35);
      color: #fff8e7;
    }
  }

  &.maximize {
    color: @morandi-yellow;

    &:hover {
      background: rgba(230, 213, 184, 0.25);
      color: #fff8e7;

      .el-icon,
      .control-icon {
        transform: scale(1.15);
      }
    }

    &:active {
      background: rgba(230, 213, 184, 0.35);
      color: #fff8e7;
    }
  }

  &.close {
    color: @morandi-red;

    &:hover {
      background: #e81123;
      color: white;

      .el-icon,
      .control-icon {
        transform: scale(1.15);
      }
    }

    &:active {
      background: #c50f1f;
      color: rgba(255, 255, 255, 0.8);
    }
  }
}

// ========== 页面内容 ==========
.page-content {
  flex: 1;
  overflow: auto;
}

// ========== 动画 ==========
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

body[data-bs-theme='light'] {
  .sidebar-wrapper {
    background: linear-gradient(135deg, @gura-bg-light 0%, @gura-bg-mid 50%, #d4e8ff 100%);
  }

  .sidebar {
    background: linear-gradient(180deg, @gura-bg-mid 0%, @gura-bg-light 100%);
    .shadow-sidebar-light();

    &.collapsed {
      .shadow-sidebar-light-collapsed();
    }

    &-header,
    &-footer {
      border-color: rgba(74, 158, 255, 0.2);
    }
  }

  .logo {
    background: linear-gradient(135deg, @gura-blue 0%, @gura-light-blue 100%);
    box-shadow: 0 2px 8px rgba(74, 158, 255, 0.3);
  }

  .app-name {
    color: @gura-blue;
    text-shadow: 0 2px 4px rgba(74, 158, 255, 0.2);
  }

  .collapse-btn {
    background: transparent !important;
    color: inherit;
    box-shadow: none;

    &::before,
    &::after {
      content: none;
      display: none;
    }

    &:hover {
      background: transparent;
      box-shadow: none;
    }
  }

  .section-title {
    color: #5a8db8;
  }

  .menu-item {
    color: @gura-accent;

    &::before {
      background: linear-gradient(180deg, @gura-blue 0%, @gura-light-blue 100%);
    }

    &:hover {
      background: rgba(74, 158, 255, 0.15);
      color: @gura-blue;
      .shadow-menu-item-light();

      .el-icon {
        filter: drop-shadow(0 2px 4px rgba(74, 158, 255, 0.4));
      }
    }

    &.danger:hover {
      background: rgba(74, 158, 255, 0.25);
      color: #3a7ed5;
    }

    // 活动状态样式（浅色模式）
    &.active {
      background: linear-gradient(90deg, rgba(74, 158, 255, 0.25), rgba(122, 184, 255, 0.15));
      border-left: 3px solid @gura-blue;
      color: @gura-text;
      font-weight: 600;
      .shadow-menu-item-light();

      &::before {
        opacity: 1;
        background: linear-gradient(90deg, rgba(74, 158, 255, 0.25), transparent);
      }

      .el-icon {
        color: @gura-blue;
        transform: scale(1.15);
        filter: drop-shadow(0 2px 6px rgba(74, 158, 255, 0.5));
      }

      &:hover {
        background: linear-gradient(90deg, rgba(74, 158, 255, 0.3), rgba(122, 184, 255, 0.2));
      }
    }
  }

  .main-content {
    background: linear-gradient(135deg, @gura-bg-mid 0%, #d4e8ff 100%);
    .shadow-content-light();
  }

  .drag-region {
    &:hover {
      background: rgba(74, 158, 255, 0.03);
    }
  }

  // Windows 控制按钮
  .control-btn {
    &.minimize {
      color: @gura-text;

      &:hover {
        background: rgba(74, 158, 255, 0.3);
        color: @gura-blue;

        .el-icon {
          transform: scale(1.15);
        }
      }

      &:active {
        background: rgba(74, 158, 255, 0.4);
        color: @gura-blue;
      }
    }

    &.maximize {
      color: @gura-text;

      &:hover {
        background: rgba(74, 158, 255, 0.3);
        color: @gura-blue;

        .el-icon {
          transform: scale(1.15);
        }
      }

      &:active {
        background: rgba(74, 158, 255, 0.4);
        color: @gura-blue;
      }
    }

    &.close {
      color: @gura-text;

      &:hover {
        background: #e81123;
        color: white;

        .el-icon {
          transform: scale(1.15);
        }
      }

      &:active {
        background: #c50f1f;
        color: rgba(255, 255, 255, 0.8);
      }
    }
  }
}

// ========== 响应式设计 ==========
@media (max-width: 768px) {
  .sidebar {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 1000;

    &.collapsed {
      transform: translateX(-100%);
    }
  }

  .main-content {
    width: 100% !important;
  }
}
</style>
