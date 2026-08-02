import { ref, computed, watch } from 'vue'
import { tauriInvoke } from './useTauri'
import { useI18n } from '../i18n/index.js'

const STORAGE_KEY = 'foundation-desktop-apps'

export function useApps() {
  const { t } = useI18n()
  const proxyUrl = ref('http://localhost:48081')
  const apps = ref([])
  const loading = ref(true)
  const loadFailed = ref(false)
  const searchQuery = ref('')
  const launchingApp = ref(null)
  const failedImages = ref(new Set())
  const coverVersions = ref({}) // 封面版本号，用于强制刷新图片缓存

  // 视图选项（持久化）
  const viewMode = ref(localStorage.getItem(`${STORAGE_KEY}-view`) || 'grid')
  const gridSize = ref(localStorage.getItem(`${STORAGE_KEY}-grid`) || 'medium')
  const sortMode = ref(localStorage.getItem(`${STORAGE_KEY}-sort`) || 'name')
  const activeFilter = ref('all')

  // 收藏和最近启动（持久化）
  const favorites = ref(JSON.parse(localStorage.getItem(`${STORAGE_KEY}-favorites`) || '[]'))
  const recentHistory = ref(JSON.parse(localStorage.getItem(`${STORAGE_KEY}-recent`) || '[]'))
  let loadRequestId = 0

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
    { id: 'all', label: t.value.apps.filters.all, count: apps.value.length },
    { id: 'favorites', label: t.value.apps.filters.favorites, count: favorites.value.length },
    { id: 'recent', label: t.value.apps.filters.recent, count: recentHistory.value.length },
  ])

  const sortLabel = computed(() => {
    switch (sortMode.value) {
      case 'name': return t.value.apps.sort.name
      case 'recent': return t.value.apps.sort.recent
      default: return t.value.apps.sort.name
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

  function isDirectImageUrl(imagePath) {
    return /^(https?:|data:|blob:)/i.test(imagePath)
  }

  function toProxyPath(path) {
    const normalizedProxy = proxyUrl.value.replace(/\/$/, '')
    const normalizedPath = path.startsWith('/') ? path : `/${path}`
    return `${normalizedProxy}${normalizedPath}`
  }

  function encodeBoxArtName(name) {
    const value = String(name || '')
    try {
      return encodeURIComponent(decodeURIComponent(value))
    } catch {
      return encodeURIComponent(value)
    }
  }

  function getAppImageUrl(app) {
    if (failedImages.value.has(app.name)) return null
    const ver = coverVersions.value[app.name]
    // 如果刚上传了新封面，直接用 appName.png（上传 API 按 app name 保存）
    if (ver) {
      return `${proxyUrl.value}/boxart/${encodeBoxArtName(app.name)}.png?v=${ver}`
    }
    const imagePath = app['image-path']
    let url
    if (!imagePath) {
      url = `${proxyUrl.value}/boxart/${encodeBoxArtName(app.name)}.png`
    } else if (isDirectImageUrl(imagePath)) {
      url = imagePath.startsWith('http') ? `${proxyUrl.value}/boxart/${encodeBoxArtName(app.name)}.png` : imagePath
    } else if (imagePath === 'desktop') {
      url = `${proxyUrl.value}/boxart/desktop.png`
    } else if (imagePath.startsWith('/boxart/') || imagePath.startsWith('boxart/')) {
      url = toProxyPath(imagePath)
    } else if (imagePath.startsWith('/')) {
      url = toProxyPath(imagePath)
    } else if (!/[/\\]/.test(imagePath)) {
      url = `${proxyUrl.value}/boxart/${encodeBoxArtName(imagePath)}`
    } else {
      url = `${proxyUrl.value}/boxart/${encodeBoxArtName(imagePath.split(/[/\\]/).pop())}`
    }
    return url
  }

  function handleImageError(event, app) {
    failedImages.value.add(app.name)
    failedImages.value = new Set(failedImages.value)
  }

  function invalidateAppImage(appName) {
    failedImages.value.delete(appName)
    failedImages.value = new Set(failedImages.value)
    coverVersions.value = { ...coverVersions.value, [appName]: Date.now() }
    // 浅拷贝 apps 触发子组件重新渲染（getAppImageUrl 作为 Function prop 不会触发更新）
    apps.value = [...apps.value]
  }

  async function loadApps() {
    const requestId = ++loadRequestId
    loading.value = true
    loadFailed.value = false
    try {
      const requestProxyUrl = await tauriInvoke('wait_for_proxy_ready')
      if (requestId !== loadRequestId) return

      proxyUrl.value = requestProxyUrl
      const resp = await fetch(`${requestProxyUrl}/api/apps`)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)

      const data = await resp.json()
      if (requestId !== loadRequestId) return

      apps.value = data.apps || data || []
    } catch (e) {
      if (requestId !== loadRequestId) return

      console.error('Failed to load apps:', e)
      loadFailed.value = true
    } finally {
      if (requestId === loadRequestId) loading.value = false
    }
  }

  const launchError = ref('')

  async function launchApp(app) {
    launchError.value = ''

    const hasDetached = Array.isArray(app.detached) && app.detached.some((cmd) => String(cmd || '').trim())
    if (!app.cmd && !hasDetached) {
      launchError.value = t.value.apps.noCommand.replace('{name}', app.name)
      console.warn('[useApps] app has no cmd:', app.name)
      setTimeout(() => { launchError.value = '' }, 4000)
      return
    }

    if (launchingApp.value) return
    launchingApp.value = app.name
    addToRecent(app.name)

    try {
      await tauriInvoke('launch_app', {
        app,
        cmd: app.cmd || '',
        workingDir: app['working-dir'] || null,
        elevated: app.elevated === true || app.elevated === 'true',
      })
    } catch (e) {
      console.error('Failed to launch app:', e, '\ncmd:', app.cmd, '\nworking-dir:', app['working-dir'])
      launchError.value = `${t.value.apps.launchFailed.replace('{name}', app.name).replace('{error}', e)}\ncmd: ${app.cmd}`
      setTimeout(() => { launchError.value = '' }, 8000)
    } finally {
      setTimeout(() => { launchingApp.value = null }, 1500)
    }
  }

  return {
    proxyUrl,
    apps,
    loading,
    loadFailed,
    searchQuery,
    launchingApp,
    launchError,
    failedImages,
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
    invalidateAppImage,
    loadApps,
    launchApp,
  }
}
