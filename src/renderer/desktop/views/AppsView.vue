<template>
  <div class="apps-view">
    <AppToolbar
      :filterTabs="filterTabs"
      :activeFilter="activeFilter"
      :searchQuery="searchQuery"
      :sortLabel="sortLabel"
      :gridSize="gridSize"
      :viewMode="viewMode"
      :totalCount="displayApps.length"
      @update:activeFilter="activeFilter = $event"
      @update:searchQuery="searchQuery = $event"
      @cycleSortMode="cycleSortMode"
      @cycleGridSize="cycleGridSize"
      @toggleViewMode="viewMode = viewMode === 'grid' ? 'list' : 'grid'"
    />

    <AppRecentStrip
      v-if="recentApps.length > 0 && !searchQuery && activeFilter === 'all'"
      :apps="recentApps"
      :getAppImageUrl="getAppImageUrl"
      :handleImageError="handleImageError"
      @launch="launchApp"
      @contextmenu="openContextMenu"
    />

    <!-- 加载状态 -->
    <div v-if="loading" class="apps-loading">
      <div class="loading-spinner"></div>
      <span>{{ t.apps.loading }}</span>
    </div>

    <!-- 加载失败状态 -->
    <div v-else-if="loadFailed" class="apps-empty apps-error" role="alert">
      <svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <circle cx="12" cy="12" r="9"/><path d="M12 7v6"/><path d="M12 17h.01"/>
      </svg>
      <p>{{ t.apps.loadFailed }}</p>
      <button class="retry-btn" type="button" @click="loadApps">{{ t.apps.retry }}</button>
    </div>

    <!-- 空状态 -->
    <div v-else-if="displayApps.length === 0 && !loading" class="apps-empty">
      <svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <rect x="2" y="2" width="8" height="8" rx="2"/><rect x="14" y="2" width="8" height="8" rx="2"/>
        <rect x="2" y="14" width="8" height="8" rx="2"/><rect x="14" y="14" width="8" height="8" rx="2"/>
      </svg>
      <p v-if="searchQuery">{{ t.apps.searchNoMatch }}</p>
      <p v-else-if="activeFilter === 'favorites'">{{ t.apps.noFavorites }}</p>
      <p v-else>{{ t.apps.noApps }}</p>
      <span class="empty-hint" v-if="!searchQuery && activeFilter === 'all'">{{ t.apps.addHint }}</span>
    </div>

    <AppGridView
      v-else-if="viewMode === 'grid'"
      :apps="displayApps"
      :gridSize="gridSize"
      :launchingApp="launchingApp"
      :isFavorite="isFavorite"
      :getAppImageUrl="getAppImageUrl"
      :handleImageError="handleImageError"
      :helperIds="getActiveHelperIds"
      :stats="gameStats"
      @launch="launchApp"
      @contextmenu="openContextMenu"
      @toggleFavorite="toggleFavorite"
    />

    <AppListView
      v-else
      :apps="displayApps"
      :launchingApp="launchingApp"
      :isFavorite="isFavorite"
      :getAppImageUrl="getAppImageUrl"
      :handleImageError="handleImageError"
      :stats="gameStats"
      @launch="launchApp"
      @contextmenu="openContextMenu"
    />

    <AppContextMenu
      :visible="contextMenu.visible"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :isFavorited="isFavorite(contextMenu.app?.name)"
      :hasCmd="!!contextMenu.app?.cmd"
      :hasWorkingDir="!!contextMenu.app?.['working-dir']"
      @launch="ctxLaunch"
      @toggleFavorite="ctxToggleFavorite"
      @copyCmd="ctxCopyCmd"
      @openDir="ctxOpenDir"
      @configHelpers="ctxConfigHelpers"
      @updateCover="ctxUpdateCover"
      @close="closeContextMenu"
    />

    <LaunchHelperPanel
      :open="helperPanel.open"
      :appName="helperPanel.appName"
      :app="helperPanel.app"
      :proxyUrl="proxyUrl"
      @close="helperPanel.open = false"
      @saved="loadApps"
    />

    <CoverPickerModal
      :open="coverPicker.open"
      :appName="coverPicker.appName"
      :proxyUrl="proxyUrl"
      @close="coverPicker.open = false"
      @updated="onCoverUpdated"
    />

    <!-- 启动错误提示 -->
    <Transition name="toast">
      <div v-if="launchError" class="launch-error-toast">
        <span class="toast-icon">⚠️</span>
        <span class="toast-msg">{{ launchError }}</span>
        <button class="toast-close" @click="launchError = ''">✕</button>
      </div>
    </Transition>
  </div>
</template>

<script setup>
import { ref, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { useApps } from '../composables/useApps'
import { useLaunchHelpers } from '../composables/useLaunchHelpers'
import { tauriInvoke } from '../composables/useTauri'
import { gamepadConnected, onGamepadAction } from '../composables/useGamepad.js'
import { editInputWithOsk } from '../composables/useOsk.js'
import { focusElement } from '../composables/useFocusNav.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()
import AppToolbar from '../components/AppToolbar.vue'
import AppRecentStrip from '../components/AppRecentStrip.vue'
import AppGridView from '../components/AppGridView.vue'
import AppListView from '../components/AppListView.vue'
import AppContextMenu from '../components/AppContextMenu.vue'
import LaunchHelperPanel from '../components/LaunchHelperPanel.vue'
import CoverPickerModal from '../components/CoverPickerModal.vue'

const {
  loading,
  loadFailed,
  searchQuery,
  launchingApp,
  launchError,
  viewMode,
  gridSize,
  activeFilter,
  filterTabs,
  sortLabel,
  proxyUrl,
  recentApps,
  displayApps,
  gameStats,
  isFavorite,
  toggleFavorite,
  cycleSortMode,
  cycleGridSize,
  getAppImageUrl,
  handleImageError,
  invalidateAppImage,
  loadApps,
  launchApp,
} = useApps()

// 右键菜单
const contextMenu = ref({ visible: false, x: 0, y: 0, app: null })

// 启动助手面板
const helperPanel = ref({ open: false, appName: '', app: null })
const { getActiveHelperIds, helperPanelOpen } = useLaunchHelpers(t)

// 同步面板状态到共享 composable（供 DesktopApp 控制器返回使用）
watch(() => helperPanel.value.open, (v) => { helperPanelOpen.value = v })
watch(helperPanelOpen, (v) => { if (!v) helperPanel.value.open = false })

function openContextMenu(event, app) {
  contextMenu.value = {
    visible: true,
    x: event.clientX,
    y: event.clientY,
    app,
  }
}

function closeContextMenu() {
  contextMenu.value.visible = false
}

function ctxLaunch() {
  if (contextMenu.value.app) launchApp(contextMenu.value.app)
  closeContextMenu()
}

function ctxToggleFavorite() {
  if (contextMenu.value.app) toggleFavorite(contextMenu.value.app.name)
  closeContextMenu()
}

function ctxConfigHelpers() {
  if (contextMenu.value.app) {
    helperPanel.value = {
      open: true,
      appName: contextMenu.value.app.name,
      app: contextMenu.value.app,
    }
  }
  closeContextMenu()
}

async function ctxCopyCmd() {
  if (contextMenu.value.app?.cmd) {
    try {
      await navigator.clipboard.writeText(contextMenu.value.app.cmd)
    } catch (e) {
      console.error('Copy failed:', e)
    }
  }
  closeContextMenu()
}

async function ctxOpenDir() {
  const dir = contextMenu.value.app?.['working-dir']
  if (dir) {
    try {
      const opened = await tauriInvoke('open_local_path', {
        path: dir,
        appName: contextMenu.value.app.name,
      })
      if (!opened) throw new Error('Open directory returned false')
    } catch (e) {
      console.error('Open dir failed:', e)
      launchError.value = `${t.value.appContext.openDirectory}: ${e}`
      setTimeout(() => { launchError.value = '' }, 4000)
    }
  }
  closeContextMenu()
}

// 封面选择弹窗
const coverPicker = ref({ open: false, appName: '' })

async function ctxUpdateCover() {
  const app = contextMenu.value.app
  closeContextMenu()
  if (!app) return

  coverPicker.value = { open: true, appName: app.name }
}

function onCoverUpdated(appName) {
  invalidateAppImage(appName)
}

function onDocClick() {
  closeContextMenu()
}

// === 手柄动作 ===

/** 当前焦点落在哪个应用卡片上。网格、列表、最近条都带 data-app-name。 */
function focusedApp() {
  const host = document.activeElement?.closest?.('[data-app-name]')
  if (!host) return null
  const name = host.dataset.appName
  return {
    element: host,
    app: displayApps.value.find((entry) => entry.name === name)
      || recentApps.value.find((entry) => entry.name === name)
      || null,
  }
}

/** Y 键：在焦点卡片下方打开上下文菜单，不再依赖鼠标坐标。 */
function openContextMenuForFocus() {
  const focused = focusedApp()
  if (!focused?.app) return false
  const rect = focused.element.getBoundingClientRect()
  contextMenu.value = {
    visible: true,
    x: rect.left,
    y: rect.bottom + 6,
    app: focused.app,
  }
  return true
}

function toggleFavoriteForFocus() {
  const focused = focusedApp()
  if (!focused?.app) return false
  toggleFavorite(focused.app.name)
  return true
}

async function focusSearchWithOsk() {
  const input = document.querySelector('[data-focus-key="apps-search"]')
  if (!input) return
  focusElement(input)
  await editInputWithOsk(input, { title: t.value.apps.searchPlaceholder })
}

function cycleFilter(step) {
  const tabs = filterTabs.value
  if (tabs.length === 0) return
  const index = tabs.findIndex((tab) => tab.id === activeFilter.value)
  const next = (index + step + tabs.length) % tabs.length
  activeFilter.value = tabs[next].id
}

function handleGamepadAction(action) {
  // B 键：从内到外逐层关闭本视图拥有的弹层
  if (action === 'back' || action === 'backRoot') {
    if (contextMenu.value.visible) closeContextMenu()
    else if (coverPicker.value.open) coverPicker.value.open = false
    else if (helperPanel.value.open) helperPanel.value.open = false
    return
  }

  // 弹层打开时其余按键留给弹层自己处理
  if (contextMenu.value.visible || coverPicker.value.open || helperPanel.value.open) return

  switch (action) {
    case 'menu':
      openContextMenuForFocus()
      break
    case 'favorite':
      toggleFavoriteForFocus()
      break
    case 'search':
      focusSearchWithOsk()
      break
    case 'filterPrev':
      cycleFilter(-1)
      break
    case 'filterNext':
      cycleFilter(1)
      break
  }
}

/**
 * 首屏把焦点放在第一张卡片上（有「最近启动」时就是上次玩的那个），
 * 这样手柄第一次按方向键是从有意义的位置开始移动，而不是跳到元素 0。
 * 没插手柄时不做，避免干扰鼠标用户。
 */
async function focusFirstApp() {
  if (!gamepadConnected.value) return
  await nextTick()
  if (document.activeElement && document.activeElement !== document.body) return
  const first = document.querySelector('.apps-view [data-app-name]')
  if (first) focusElement(first)
}

let disposeGamepadActions = null

onMounted(async () => {
  document.addEventListener('click', onDocClick)
  disposeGamepadActions = onGamepadAction(handleGamepadAction)
  await loadApps()
  await focusFirstApp()
})

onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
  disposeGamepadActions?.()
})
</script>

<style lang="less" scoped>
.apps-view {
  max-width: 1600px;
  margin: 0 auto;
}

// === 加载 & 空状态 ===
.apps-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 100px 0;
  gap: 16px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);

  .loading-spinner {
    width: 36px;
    height: 36px;
    border: 3px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    border-top-color: var(--fd-accent, #00fff5);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
}

@keyframes spin { to { transform: rotate(360deg); } }

.apps-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 100px 0;
  gap: 10px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);

  .empty-icon { width: 56px; height: 56px; opacity: 0.25; margin-bottom: 8px; }
  p { margin: 0; font-size: 16px; }
  .empty-hint { font-size: 13px; color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.2); }

  .retry-btn {
    margin-top: 8px;
    padding: 8px 20px;
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.45);
    border-radius: 8px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    color: var(--fd-accent, #00fff5);
    cursor: pointer;
    transition: background 0.2s, border-color 0.2s;

    &:hover,
    &:focus-visible {
      border-color: var(--fd-accent, #00fff5);
      background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.18);
      outline: none;
    }
  }
}

.apps-error { color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.55); }

// === 启动错误 Toast ===
.launch-error-toast {
  position: fixed;
  bottom: 32px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 9999;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 20px;
  background: rgba(220, 50, 50, 0.92);
  color: #fff;
  border-radius: 10px;
  font-size: 14px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(8px);

  .toast-icon { font-size: 18px; }
  .toast-msg { max-width: 600px; word-break: break-all; white-space: pre-line; font-size: 13px; line-height: 1.5; }
  .toast-close {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    font-size: 16px;
    padding: 0 4px;
    &:hover { color: #fff; }
  }
}

.toast-enter-active { transition: all 0.3s ease-out; }
.toast-leave-active { transition: all 0.25s ease-in; }
.toast-enter-from { opacity: 0; transform: translateX(-50%) translateY(20px); }
.toast-leave-to { opacity: 0; transform: translateX(-50%) translateY(10px); }
</style>
