import { ref, computed, watch, onMounted, onUnmounted } from 'vue'

const STORAGE_KEY = 'foundation-desktop-apps'

export function useApps() {
  const proxyUrl = ref('http://localhost:48081')
  const apps = ref([])
  const loading = ref(true)
  const searchQuery = ref('')
  const launchingApp = ref(null)
  const failedImages = ref(new Set())

  // 视图选项（持久化）
  const viewMode = ref(localStorage.getItem(`${STORAGE_KEY}-view`) || 'grid')
  const gridSize = ref(localStorage.getItem(`${STORAGE_KEY}-grid`) || 'medium')
  const sortMode = ref(localStorage.getItem(`${STORAGE_KEY}-sort`) || 'name')
  const activeFilter = ref('all')

  // 收藏和最近启动（持久化）
  const favorites = ref(JSON.parse(localStorage.getItem(`${STORAGE_KEY}-favorites`) || '[]'))
  const recentHistory = ref(JSON.parse(localStorage.getItem(`${STORAGE_KEY}-recent`) || '[]'))

  // 持久化（防抖批量写入）
  let persistTimer = null
  function persistSettings() {
    clearTimeout(persistTimer)
    persistTimer = setTimeout(() => {
      localStorage.setItem(`${STORAGE_KEY}-view`, viewMode.value)
      localStorage.setItem(`${STORAGE_KEY}-grid`, gridSize.value)
      localStorage.setItem(`${STORAGE_KEY}-sort`, sortMode.value)
      localStorage.setItem(`${STORAGE_KEY}-favorites`, JSON.stringify(favorites.value))
      localStorage.setItem(`${STORAGE_KEY}-recent`, JSON.stringify(recentHistory.value))
    }, 300)
  }
  watch(viewMode, persistSettings)
  watch(gridSize, persistSettings)
  watch(sortMode, persistSettings)
  watch(favorites, persistSettings, { deep: true })
  watch(recentHistory, persistSettings, { deep: true })

  // 筛选标签
  const filterTabs = computed(() => [
    { id: 'all', label: '全部', count: apps.value.length },
    { id: 'favorites', label: '收藏', count: favorites.value.length },
    { id: 'recent', label: '最近', count: recentHistory.value.length },
  ])

  const sortLabel = computed(() => {
    switch (sortMode.value) {
      case 'name': return '名称'
      case 'recent': return '最近使用'
      default: return '名称'
    }
  })

  function isFavorite(name) {
    return favorites.value.includes(name)
  }

  function toggleFavorite(name) {
    const idx = favorites.value.indexOf(name)
    if (idx >= 0) {
      favorites.value.splice(idx, 1)
    } else {
      favorites.value.push(name)
    }
  }

  function addToRecent(name) {
    recentHistory.value = [name, ...recentHistory.value.filter(n => n !== name)].slice(0, 10)
  }

  const recentApps = computed(() => {
    return recentHistory.value
      .map(name => apps.value.find(a => a.name === name))
      .filter(Boolean)
      .slice(0, 6)
  })

  // 排序 + 筛选 + 搜索
  const displayApps = computed(() => {
    let list = [...apps.value]

    if (activeFilter.value === 'favorites') {
      list = list.filter(a => isFavorite(a.name))
    } else if (activeFilter.value === 'recent') {
      const order = recentHistory.value
      list = list.filter(a => order.includes(a.name))
      list.sort((a, b) => order.indexOf(a.name) - order.indexOf(b.name))
      if (searchQuery.value) {
        const q = searchQuery.value.toLowerCase()
        list = list.filter(a => a.name?.toLowerCase().includes(q))
      }
      return list
    }

    if (searchQuery.value) {
      const q = searchQuery.value.toLowerCase()
      list = list.filter(a => a.name?.toLowerCase().includes(q))
    }

    if (sortMode.value === 'name') {
      list.sort((a, b) => (a.name || '').localeCompare(b.name || ''))
    } else if (sortMode.value === 'recent') {
      const order = recentHistory.value
      list.sort((a, b) => {
        const ai = order.indexOf(a.name)
        const bi = order.indexOf(b.name)
        if (ai === -1 && bi === -1) return (a.name || '').localeCompare(b.name || '')
        if (ai === -1) return 1
        if (bi === -1) return -1
        return ai - bi
      })
    }

    // 收藏置顶
    if (activeFilter.value === 'all') {
      const favSet = new Set(favorites.value)
      list.sort((a, b) => {
        const af = favSet.has(a.name) ? 0 : 1
        const bf = favSet.has(b.name) ? 0 : 1
        return af - bf
      })
    }

    return list
  })

  function cycleSortMode() {
    const modes = ['name', 'recent']
    const idx = modes.indexOf(sortMode.value)
    sortMode.value = modes[(idx + 1) % modes.length]
  }

  function cycleGridSize() {
    const sizes = ['small', 'medium', 'large']
    const idx = sizes.indexOf(gridSize.value)
    gridSize.value = sizes[(idx + 1) % sizes.length]
  }

  function getAppImageUrl(app) {
    if (failedImages.value.has(app.name)) return null
    const imagePath = app['image-path']
    if (!imagePath) {
      return `${proxyUrl.value}/boxart/${encodeURIComponent(app.name)}.png`
    }
    if (imagePath === 'desktop') {
      return `${proxyUrl.value}/boxart/desktop.png`
    }
    if (!/[/\\]/.test(imagePath)) {
      return `${proxyUrl.value}/boxart/${encodeURIComponent(imagePath)}`
    }
    return `${proxyUrl.value}/boxart/${encodeURIComponent(imagePath.split(/[/\\]/).pop())}`
  }

  function handleImageError(event, app) {
    failedImages.value.add(app.name)
    failedImages.value = new Set(failedImages.value)
  }

  async function loadApps() {
    loading.value = true
    try {
      const resp = await fetch(`${proxyUrl.value}/api/apps`)
      if (resp.ok) {
        const data = await resp.json()
        apps.value = data.apps || data || []
      }
    } catch (e) {
      console.error('Failed to load apps:', e)
    } finally {
      loading.value = false
    }
  }

  const launchError = ref('')

  async function launchApp(app) {
    launchError.value = ''

    if (!app.cmd) {
      launchError.value = `"${app.name}" 没有配置启动命令`
      console.warn('[useApps] app has no cmd:', app.name)
      setTimeout(() => { launchError.value = '' }, 4000)
      return
    }

    if (launchingApp.value) return
    launchingApp.value = app.name
    addToRecent(app.name)

    try {
      const tauri = await import('@tauri-apps/api/core')
      await tauri.invoke('launch_app', {
        cmd: app.cmd,
        workingDir: app['working-dir'] || null,
        elevated: app.elevated === true || app.elevated === 'true',
      })
    } catch (e) {
      console.error('Failed to launch app:', e, '\ncmd:', app.cmd, '\nworking-dir:', app['working-dir'])
      launchError.value = `"${app.name}" ${e}\ncmd: ${app.cmd}`
      setTimeout(() => { launchError.value = '' }, 8000)
    } finally {
      setTimeout(() => { launchingApp.value = null }, 1500)
    }
  }

  async function initProxy() {
    try {
      const tauri = await import('@tauri-apps/api/core')
      const url = await tauri.invoke('get_proxy_url_command')
      if (url) proxyUrl.value = url
    } catch (e) {
      console.log('Tauri invoke not available:', e)
    }
  }

  return {
    proxyUrl,
    apps,
    loading,
    searchQuery,
    launchingApp,
    launchError,
    viewMode,
    gridSize,
    sortMode,
    activeFilter,
    favorites,
    recentHistory,
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
  }
}
