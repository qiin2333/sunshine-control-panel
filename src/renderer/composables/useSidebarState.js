import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter, ROUTES } from './useRouter.js'

const SKIPPED_VERSION_KEY = 'sunshine-skipped-version'
const THEME_KEY = 'sunshine-theme'
const THEME_DARK = 'dark'
const THEME_LIGHT = 'light'

/**
 * 规范化版本号（移除 v/V 前缀）
 */
const normalizeVersion = (version) => version?.replace(/^[vV]/, '') || ''

/**
 * 向 iframe 发送消息
 */
const postMessageToIframes = (message) => {
  document.querySelectorAll('iframe').forEach((iframe) => {
    try {
      iframe.contentWindow?.postMessage(message, '*')
    } catch {
      // 跨域限制，忽略错误
    }
  })
}

/**
 * 侧边栏状态管理 Composable
 */
export function useSidebarState() {
  const router = useRouter()

  // 状态定义
  const isCollapsed = ref(false)
  const isDark = ref(true)
  const isMaximized = ref(false)
  const isAdmin = ref(true)
  const showUpdateDialog = ref(false)
  const updateInfo = ref(null)
  const currentVersion = ref('0.0.0')
  const skippedVersion = ref(localStorage.getItem(SKIPPED_VERSION_KEY) || '')

  // 计算属性
  const showVddSettings = computed(() => router.isRoute(ROUTES.VDD_SETTINGS))
  const showWelcome = computed(() => router.isRoute(ROUTES.WELCOME))
  const currentTheme = computed(() => (isDark.value ? THEME_DARK : THEME_LIGHT))

  // 清理函数存储
  const cleanupFns = []

  /**
   * 切换主题
   */
  const toggleTheme = () => {
    isDark.value = !isDark.value
    document.documentElement?.setAttribute('data-bs-theme', currentTheme.value)
    localStorage.setItem(THEME_KEY, currentTheme.value)

    postMessageToIframes({ type: 'theme-sync', theme: currentTheme.value })
    ElMessage.success(isDark.value ? '已切换到深色模式' : '已切换到浅色模式')
  }

  /**
   * 切换折叠状态
   */
  const toggleCollapse = () => {
    isCollapsed.value = !isCollapsed.value
  }

  /**
   * 导航方法
   */
  const openVddSettings = () => router.navigate(ROUTES.VDD_SETTINGS)
  const openWelcome = () => router.navigate(ROUTES.WELCOME)
  const goHome = () => router.goHome()

  /**
   * 忽略指定版本的更新
   */
  const skipVersion = (version) => {
    if (!version) return
    const normalized = normalizeVersion(version)
    skippedVersion.value = normalized
    localStorage.setItem(SKIPPED_VERSION_KEY, normalized)
    ElMessage.info(`已忽略版本 ${version}，下次自动检查更新时将跳过此版本`)
  }

  /**
   * 检查版本是否被忽略
   */
  const isVersionSkipped = (version) => {
    if (!version || !skippedVersion.value) return false
    return normalizeVersion(version) === skippedVersion.value
  }

  /**
   * 初始化管理员状态
   */
  const initAdminStatus = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      isAdmin.value = await invoke('is_running_as_admin')
      console.log(isAdmin.value ? '✅ 当前以管理员权限运行' : '⚠️  当前未以管理员权限运行')
    } catch (error) {
      console.error('检测管理员权限失败:', error)
    }
  }

  /**
   * 初始化主题
   */
  const initTheme = () => {
    const savedTheme = localStorage.getItem(THEME_KEY)
    if (savedTheme) {
      isDark.value = savedTheme === THEME_DARK
    } else {
      isDark.value = window.matchMedia('(prefers-color-scheme: dark)').matches
    }
    document.documentElement?.setAttribute('data-bs-theme', currentTheme.value)
  }

  /**
   * 初始化窗口状态
   */
  const initWindowState = async () => {
    try {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
      isMaximized.value = await getCurrentWebviewWindow().isMaximized()
    } catch (error) {
      console.error('检测窗口状态失败:', error)
    }
  }

  /**
   * 初始化版本信息
   */
  const initVersion = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      currentVersion.value = (await invoke('get_sunshine_version')) || 'Unknown'
    } catch (error) {
      console.error('获取 Sunshine 版本失败:', error)
      currentVersion.value = 'Unknown'
    }
  }

  /**
   * 初始化事件监听
   */
  const initEventListeners = async () => {
    // iframe 主题请求监听
    const handleMessage = (event) => {
      const isLocalhost = event.origin.includes('localhost') || event.origin.includes('127.0.0.1')
      if (isLocalhost && event.data.type === 'request-theme') {
        postMessageToIframes({ type: 'theme-sync', theme: currentTheme.value })
      }
    }
    window.addEventListener('message', handleMessage)
    cleanupFns.push(() => window.removeEventListener('message', handleMessage))

    // Tauri 事件监听
    const { listen } = await import('@tauri-apps/api/event')

    const unlistenUpdate = await listen('update-available', (event) => {
      const newVersion = event.payload?.version
      if (!isVersionSkipped(newVersion)) {
        updateInfo.value = event.payload
        showUpdateDialog.value = true
      }
    })
    cleanupFns.push(unlistenUpdate)

    const unlistenCheckResult = await listen('update-check-result', (event) => {
      const { is_latest, message, error } = event.payload
      if (is_latest) {
        ElMessage.success(message || '已是最新版本')
      } else if (error) {
        ElMessage.error(`检查更新失败: ${error}`)
      }
    })
    cleanupFns.push(unlistenCheckResult)
  }

  /**
   * 初始化状态
   */
  const initState = async () => {
    await Promise.all([initAdminStatus(), initWindowState(), initVersion()])
    initTheme()
    await initEventListeners()
  }

  onMounted(initState)

  onUnmounted(() => {
    cleanupFns.forEach((fn) => fn?.())
    cleanupFns.length = 0
  })

  return {
    // 状态
    isCollapsed,
    isDark,
    isMaximized,
    isAdmin,
    showVddSettings,
    showWelcome,
    showUpdateDialog,
    updateInfo,
    currentVersion,
    skippedVersion,

    // 路由
    router,

    // 方法
    toggleTheme,
    toggleCollapse,
    openVddSettings,
    openWelcome,
    goHome,
    skipVersion,
    isVersionSkipped,
  }
}
