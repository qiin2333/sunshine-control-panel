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
      <span>加载应用列表...</span>
    </div>

    <!-- 空状态 -->
    <div v-else-if="displayApps.length === 0 && !loading" class="apps-empty">
      <svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <rect x="2" y="2" width="8" height="8" rx="2"/><rect x="14" y="2" width="8" height="8" rx="2"/>
        <rect x="2" y="14" width="8" height="8" rx="2"/><rect x="14" y="14" width="8" height="8" rx="2"/>
      </svg>
      <p v-if="searchQuery">没有找到匹配的应用</p>
      <p v-else-if="activeFilter === 'favorites'">还没有收藏的应用</p>
      <p v-else>还没有配置任何应用</p>
      <span class="empty-hint" v-if="!searchQuery && activeFilter === 'all'">在 Web 控制台中添加应用</span>
    </div>

    <AppGridView
      v-else-if="viewMode === 'grid'"
      :apps="displayApps"
      :gridSize="gridSize"
      :launchingApp="launchingApp"
      :isFavorite="isFavorite"
      :getAppImageUrl="getAppImageUrl"
      :handleImageError="handleImageError"
      :helperIcons="getActiveHelperIcons"
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
    />

    <LaunchHelperPanel
      :open="helperPanel.open"
      :appName="helperPanel.appName"
      :app="helperPanel.app"
      :proxyUrl="proxyUrl"
      @close="helperPanel.open = false"
      @saved="loadApps"
    />
  </div>
</template>

<script setup>
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { useApps } from '../composables/useApps'
import { useLaunchHelpers } from '../composables/useLaunchHelpers'
import AppToolbar from '../components/AppToolbar.vue'
import AppRecentStrip from '../components/AppRecentStrip.vue'
import AppGridView from '../components/AppGridView.vue'
import AppListView from '../components/AppListView.vue'
import AppContextMenu from '../components/AppContextMenu.vue'
import LaunchHelperPanel from '../components/LaunchHelperPanel.vue'

const {
  loading,
  searchQuery,
  launchingApp,
  viewMode,
  gridSize,
  activeFilter,
  filterTabs,
  sortLabel,
  recentApps,
  displayApps,
  isFavorite,
  toggleFavorite,
  cycleSortMode,
  cycleGridSize,
  getAppImageUrl,
  handleImageError,
  loadApps,
  launchApp,
  initProxy,
} = useApps()

// 右键菜单
const contextMenu = ref({ visible: false, x: 0, y: 0, app: null })

// 启动助手面板
const helperPanel = ref({ open: false, appName: '', app: null })
const { proxyUrl } = useApps()
const { getActiveHelperIcons, helperPanelOpen } = useLaunchHelpers()

// 同步面板状态到共享 composable（供 DesktopApp 控制器返回使用）
watch(() => helperPanel.value.open, (v) => { helperPanelOpen.value = v })
watch(helperPanelOpen, (v) => { if (!v) helperPanel.value.open = false })

function openContextMenu(event, app) {
  contextMenu.value = {
    visible: true,
    x: Math.min(event.clientX, window.innerWidth - 200),
    y: Math.min(event.clientY, window.innerHeight - 200),
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
      const tauri = await import('@tauri-apps/api/core')
      await tauri.invoke('open_external_url', { url: dir })
    } catch (e) {
      console.error('Open dir failed:', e)
    }
  }
  closeContextMenu()
}

function onDocClick() {
  closeContextMenu()
}

onMounted(async () => {
  document.addEventListener('click', onDocClick)
  await initProxy()
  await loadApps()
})

onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
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
}
</style>
