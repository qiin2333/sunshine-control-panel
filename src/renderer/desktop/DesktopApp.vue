<template>
  <DesktopWindow
    :title="appTitle"
    :icon="sunshineIcon"
    :has-sidebar="true"
    :show-title-bar="false"
    :class="{ 'gamepad-active': gamepadActive, 'cursor-mode': cursorEnabled }"
  >
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

    <template #footer>
      <!-- 两者都是底部居中的浮层，必须放在同一个 flex 列里堆叠。
           各自绝对定位时会完全重叠，而运行条的 z-index 更高，会把按键提示整条盖掉
           —— 而「游戏在跑 + 手柄模式」正是沙发场景下最常见的状态。 -->
      <div class="desktop-footer-dock">
        <RunningGameBar
          :game="runningGame"
          :elapsed="elapsedSeconds"
          :stat="runningStat"
          @resume="resumeGame"
          @stop="confirmStopGame"
        />

        <GamepadLegend v-if="gamepadActive" :cursor-mode="cursorEnabled" />
      </div>
    </template>
  </DesktopWindow>

  <ThemeEditor
    :open="themeEditorOpen"
    :vars="themeVars"
    :activePreset="activePreset"
    :presets="presets"
    :wallpaper="wallpaper"
    :wallpaperColors="wallpaperColors"
    :wallpaperSeeds="wallpaperSeeds"
    :activeWallpaperSeed="activeWallpaperSeed"
    @close="themeEditorOpen = false"
    @setVar="setVar"
    @applyPreset="applyPreset"
    @export="handleThemeExport"
    @import="handleThemeImport"
    @setWallpaper="setWallpaper"
    @applySeed="applySeedColor"
    @removeWallpaper="removeWallpaper"
  />

  <OnScreenKeyboard />

  <BackHoldRing :progress="backHoldProgress" />

  <ConfirmDialog />

  <LaunchOverlay
    :state="launchState"
    @dismiss="dismissLaunchState"
    @stop="confirmStopGame"
  />

  <div
    v-if="cursorEnabled"
    class="gamepad-cursor"
    :style="{ transform: `translate(${cursorX}px, ${cursorY}px)` }"
    aria-hidden="true"
  >
    <svg viewBox="0 0 24 24">
      <path d="M5 2 L5 19 L10 14.5 L13 21.5 L16 20 L13 13.5 L19.5 13 Z" />
    </svg>
  </div>

  <SplashScreen
    :visible="showSplash"
    @done="showSplash = false"
  />
</template>

<script setup>
import { defineAsyncComponent, ref, computed, nextTick, onMounted, onUnmounted, watch } from 'vue'

// 桌面 UI 组件
import DesktopWindow from './components/DesktopWindow.vue'
import DesktopSidebar from './components/DesktopSidebar.vue'
import ThemeEditor from './components/ThemeEditor.vue'
import SplashScreen from './components/SplashScreen.vue'
import OnScreenKeyboard from './components/OnScreenKeyboard.vue'
import ConfirmDialog from './components/ConfirmDialog.vue'
import LaunchOverlay from './components/LaunchOverlay.vue'
import RunningGameBar from './components/RunningGameBar.vue'
import GamepadLegend from './components/GamepadLegend.vue'
import BackHoldRing from './components/BackHoldRing.vue'

// 手柄 / 焦点 / 屏幕键盘
import { backHoldProgress, emitGamepadAction, gamepadActive, gamepadConnected, useGamepad } from './composables/useGamepad.js'
import { playNavSound } from './composables/useNavSound.js'
import {
  collectFocusables,
  confirmFocused,
  focusElement,
  focusScopeStack,
  navigateFocus,
  rememberFocus,
  restoreFocus,
  scrollActiveScope,
} from './composables/useFocusNav.js'
import {
  cancelOsk,
  editInputWithOsk,
  isTextEntryElement,
  openOsk,
  oskOpen,
} from './composables/useOsk.js'
import { confirmOpen, rejectConfirm, requestConfirm } from './composables/useConfirm.js'
import {
  clickAtCursor,
  contextMenuAtCursor,
  cursorEnabled,
  cursorX,
  cursorY,
  elementAtCursor,
  moveCursor,
  scrollAtCursor,
  setCursorEnabled,
} from './composables/useGamepadCursor.js'
import { bigScreenSettings } from './composables/useBigScreenSettings.js'
import {
  dismissLaunchState,
  elapsedSeconds,
  initGameSession,
  disposeGameSession,
  launchState,
  runningGame,
  statFor,
  stopTrackedGame,
} from './composables/useGameSession.js'
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
const AppsView = defineAsyncComponent(() => import('./views/AppsView.vue'))
const DashboardView = defineAsyncComponent(() => import('./views/DashboardView.vue'))
const DevicesView = defineAsyncComponent(() => import('./views/DevicesView.vue'))
const StreamView = defineAsyncComponent(() => import('./views/StreamView.vue'))
const ToolsView = defineAsyncComponent(() => import('./views/ToolsView.vue'))
const SettingsView = defineAsyncComponent(() => import('./views/SettingsView.vue'))

// 导入图标资源
import sunshineIcon from '../../assets/sunshine.ico'

// i18n
const { t, locale, toggleLocale } = useI18n()

// 应用配置
const appTitle = 'FOUNDATION DESKTOP'

// 主题
const { themeVars, activePreset, presets, setVar, applyPreset, exportTheme, importTheme, wallpaper, wallpaperColors, wallpaperSeeds, activeWallpaperSeed, setWallpaper, applySeedColor, removeWallpaper } = useTheme()
const themeEditorOpen = ref(false)
const showSplash = ref(true)
const { helperPanelOpen } = useLaunchHelpers(t)

function handleThemeExport() {
  const json = exportTheme()
  navigator.clipboard.writeText(json).catch(() => {})
}

async function handleThemeImport() {
  // 用屏幕键盘而不是 prompt()：原生弹窗接不到手柄输入
  const json = await openOsk({ title: `${t.value.nav.theme} JSON` })
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

const runningStat = computed(() => statFor(runningGame.value?.appName))

// Tauri invoke
const invoke = ref(null)
let settingsListener = null
let updateUnlisten = null
let desktopNavigateUnlisten = null

function applyRuntimeSettings() {
  document.documentElement.dataset.fdDevMode = desktopSettings.value.devMode ? 'true' : 'false'
  if (desktopSettings.value.notifications) {
    requestNotificationPermission(desktopSettings.value).catch(() => {})
  }
}

function settingsText(key, fallback, replacements = {}) {
  let text = t.value.settings?.[key] || fallback
  for (const [name, value] of Object.entries(replacements)) {
    text = text.replace(`{${name}}`, value)
  }
  return text
}

async function setupUpdateNotifications() {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    updateUnlisten = await listen('update-available', (event) => {
      if (!desktopSettings.value.notifications || !desktopSettings.value.updateNotify) return
      const version = event.payload?.version || ''
      showDesktopNotification(
        settingsText('notificationTitle', 'Sunshine'),
        version
          ? settingsText('updateAvailableVersion', 'Update available: {version}', { version })
          : settingsText('updateAvailable', 'Update available'),
        desktopSettings.value
      )
    })
  } catch {
    // event API unavailable outside Tauri
  }
}

async function setupDesktopNavigationEvents() {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    desktopNavigateUnlisten = await listen('desktop-navigate', (event) => {
      const target = String(event.payload || '')
      if (viewMap[target]) {
        activeNav.value = target
      }
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
  await setupDesktopNavigationEvents()
  await setupUpdateNotifications()
  await initGameSession()
})

onUnmounted(() => {
  if (settingsListener) window.removeEventListener(DESKTOP_SETTINGS_UPDATED, settingsListener)
  if (updateUnlisten) updateUnlisten()
  if (desktopNavigateUnlisten) desktopNavigateUnlisten()
  disposeGameSession()
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

// === 正在运行的游戏 ===

async function resumeGame() {
  try {
    const { invoke: tauriApiInvoke } = await import('@tauri-apps/api/core')
    await tauriApiInvoke('focus_running_game')
  } catch {
    // 拉不到游戏窗口时至少把面板收起来，让游戏露出来
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().minimize()
    } catch {
      // 非 Tauri 环境
    }
  }
}

async function confirmStopGame() {
  const name = runningGame.value?.appName || launchState.value?.message || ''
  if (!name) return
  const accepted = await requestConfirm({
    title: t.value.gameSession.stopRunning,
    message: t.value.gameSession.stopConfirm.replace('{name}', name),
    confirmLabel: t.value.gameSession.stopRunning,
    danger: true,
  })
  if (accepted) stopTrackedGame()
}

// === 视图切换时的焦点记忆 ===

/**
 * 视图是异步组件，第一次切过去要等 chunk 加载完才有可聚焦元素，
 * 单靠 nextTick 会太早。这里轮询等主内容区渲染出来。
 */
async function restoreFocusForView(viewKey) {
  const MAX_ATTEMPTS = 12
  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    await nextTick()
    const main = document.querySelector('.desktop-window-main')
    if (main && collectFocusables(main).length > 0) break
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  restoreFocus(viewKey)
}

watch(activeNav, async (next, previous) => {
  // 没插手柄时不抢焦点，鼠标用户不该看到光标位置被移动
  if (!gamepadConnected.value) return
  if (previous) rememberFocus(previous)
  await restoreFocusForView(next)
})

// === 手柄 ===

/** 是否有弹层在接管输入。有的话全局导航要让位。 */
function hasOverlay() {
  return (
    oskOpen.value ||
    confirmOpen.value ||
    focusScopeStack.value.length > 0 ||
    themeEditorOpen.value ||
    helperPanelOpen.value
  )
}

const navigableIds = computed(() =>
  // LB/RB 只在真正的页面之间切换；语言、主题、退出是一次性动作，不该被翻到
  [...navItems.value, ...bottomNavItems.value]
    .filter((item) => !item.disabled && viewMap[item.id])
    .map((item) => item.id)
)

function switchNav(step) {
  const ids = navigableIds.value
  if (ids.length === 0) return
  const index = ids.indexOf(activeNav.value)
  if (index === -1) {
    activeNav.value = ids[0]
    return
  }
  const next = index + step
  if (next < 0 || next >= ids.length) return
  activeNav.value = ids[next]
}

function handleConfirm() {
  if (cursorEnabled.value) {
    const target = elementAtCursor()
    if (bigScreenSettings.value.oskAutoOpen && isTextEntryElement(target)) {
      target.focus({ preventScroll: true })
      editInputWithOsk(target)
      return
    }
    clickAtCursor()
    return
  }

  const focused = document.activeElement
  if (bigScreenSettings.value.oskAutoOpen && isTextEntryElement(focused)) {
    editInputWithOsk(focused)
    return
  }
  confirmFocused()
}

function handleBack(deep) {
  if (oskOpen.value) {
    cancelOsk()
    return
  }
  if (confirmOpen.value) {
    rejectConfirm()
    return
  }
  // 启动过场现在会压入焦点栈，所以 B 必须能关掉它，否则会把用户困在遮罩里。
  // launching 是唯一不可取消的状态（进程已经起来了，取消没有意义）。
  if (launchState.value && launchState.value.status !== 'launching') {
    dismissLaunchState()
    return
  }
  if (focusScopeStack.value.length > 0 || themeEditorOpen.value || helperPanelOpen.value) {
    // DesktopApp 只关自己拥有的面板，其余转发给视图（右键菜单、封面选择器）
    if (themeEditorOpen.value) {
      themeEditorOpen.value = false
      return
    }
    if (helperPanelOpen.value) {
      helperPanelOpen.value = false
      return
    }
    emitGamepadAction(deep ? 'backRoot' : 'back')
    return
  }
  if (cursorEnabled.value) {
    setCursorEnabled(false)
    return
  }
  if (deep || activeNav.value !== 'apps') {
    activeNav.value = 'apps'
    return
  }
  // 已在应用库顶层：短按 B 给一个出口（退出大屏模式），而不是无声无息
  requestExitShell()
}

function handleCursorToggle() {
  if (!bigScreenSettings.value.gamepadCursorEnabled) return
  setCursorEnabled(!cursorEnabled.value)
}

function focusSidebar() {
  const active = document.querySelector('.desktop-sidebar .nav-item.active')
  focusElement(active || document.querySelector('.desktop-sidebar .nav-item'))
}

/** 顶层短按 B：主机惯例是「有出口」——确认后退出大屏模式。 */
async function requestExitShell() {
  const accepted = await requestConfirm({
    title: t.value.gamepadLegend.exitTitle,
    message: t.value.gamepadLegend.exitMessage,
    confirmLabel: t.value.gamepadLegend.exitConfirm,
    danger: false,
  })
  if (!accepted) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().close()
  } catch {
    window.close()
  }
}

function handleAction(action) {
  switch (action) {
    case 'confirm':
      playNavSound('confirm')
      handleConfirm()
      return
    case 'back':
      playNavSound('back')
      handleBack(false)
      return
    case 'backRoot':
      playNavSound('home')
      handleBack(true)
      return
    case 'cursorToggle':
      handleCursorToggle()
      return
  }

  if (cursorEnabled.value && action === 'menu') {
    contextMenuAtCursor()
    return
  }

  if (hasOverlay()) {
    emitGamepadAction(action)
    return
  }

  switch (action) {
    case 'tabPrev':
      switchNav(-1)
      return
    case 'tabNext':
      switchNav(1)
      return
    case 'home':
      activeNav.value = 'apps'
      return
    default:
      // search / favorite / menu 是视图级动作，由各视图决定是否响应
      emitGamepadAction(action)
  }
}

// 手柄状态是模块级单例，这里只负责驱动它
useGamepad({
  onNavigate(direction) {
    const moved = navigateFocus(direction)
    if (moved) playNavSound('tick')
  },
  onScroll(deltaY, deltaX) {
    if (cursorEnabled.value) scrollAtCursor(deltaY, deltaX)
    else scrollActiveScope(deltaY, deltaX)
  },
  onCursorMove(dx, dy) {
    moveCursor(dx, dy)
  },
  onAction: handleAction,
  isCursorMode: () => cursorEnabled.value,
  // 游戏在跑且面板不在前台时不响应手柄，避免在后台悄悄移动焦点
  enabled: () => !runningGame.value || document.hasFocus(),
})

// 弹层是 Teleport 到 body 的，焦点环样式必须挂在 <html> 上才能覆盖到它们
watch(
  gamepadActive,
  (active) => {
    document.documentElement.dataset.fdGamepad = active ? 'true' : 'false'
  },
  { immediate: true }
)

// 首次拿起手柄：如果焦点还没建立（首次输入前 focus 在 body），把它放到侧边栏
// 当前项上——第一次按方向键就是有意义的位置，而不是等一帧「跳到元素 0」
watch(gamepadActive, (active) => {
  if (!active) {
    if (cursorEnabled.value) setCursorEnabled(false)
    return
  }
  if (!document.activeElement || document.activeElement === document.body) {
    focusSidebar()
  }
})
</script>

<style lang="less" scoped>
.desktop-footer-dock {
  position: absolute;
  left: 50%;
  bottom: 12px;
  transform: translateX(-50%);
  z-index: 900;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  max-width: calc(100vw - 140px);
  // 空档区不能吃掉点击；子元素各自恢复
  pointer-events: none;

  > * {
    pointer-events: auto;
  }
}

.gamepad-cursor {
  position: fixed;
  top: 0;
  left: 0;
  width: 26px;
  height: 26px;
  z-index: 30000;
  pointer-events: none;
  will-change: transform;

  svg {
    width: 100%;
    height: 100%;
    fill: var(--fd-accent, #00fff5);
    stroke: rgba(0, 0, 0, 0.85);
    stroke-width: 1.4;
    filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.7));
  }
}
</style>
