import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter, ROUTES } from './useRouter.js'

const STORAGE_KEYS = {
  SKIPPED_VERSION: 'sunshine-skipped-version',
  INCLUDE_PRERELEASE: 'sunshine-include-prerelease',
  THEME: 'sunshine-theme',
}

const THEME = {
  DARK: 'dark',
  LIGHT: 'light',
}

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
 * 获取 Tauri invoke 函数
 */
const getInvoke = async () => {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke
}

/**
 * 安全地从 localStorage 读取布尔值
 */
const getStoredBoolean = (key, defaultValue = false) => {
  const value = localStorage.getItem(key)
  return value !== null ? value === 'true' : defaultValue
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
  const skippedVersion = ref(localStorage.getItem(STORAGE_KEYS.SKIPPED_VERSION) || '')
  const includePrerelease = ref(false)

  // 清理函数存储
  const cleanupFns = []

  // 计算属性
  const showVddSettings = computed(() => router.isRoute(ROUTES.VDD_SETTINGS))
  const showWelcome = computed(() => router.isRoute(ROUTES.WELCOME))
  const currentTheme = computed(() => (isDark.value ? THEME.DARK : THEME.LIGHT))

  /**
   * 同步主题到 DOM 和 localStorage
   */
  const syncTheme = () => {
    document.documentElement?.setAttribute('data-bs-theme', currentTheme.value)
    localStorage.setItem(STORAGE_KEYS.THEME, currentTheme.value)
  }

  /**
   * 切换主题
   */
  const toggleTheme = () => {
    isDark.value = !isDark.value
    syncTheme()
    postMessageToIframes({ type: 'theme-sync', theme: currentTheme.value })
    ElMessage.success(isDark.value ? '已切换到深色模式' : '已切换到浅色模式')
  }

  /**
   * 切换折叠状态
   */
  const toggleCollapse = () => {
    isCollapsed.value = !isCollapsed.value
  }

  // 导航方法
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
    localStorage.setItem(STORAGE_KEYS.SKIPPED_VERSION, normalized)
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
   * 初始化 beta 更新偏好
   */
  const initIncludePrerelease = async () => {
    try {
      const invoke = await getInvoke()
      const savedPreference = localStorage.getItem(STORAGE_KEYS.INCLUDE_PRERELEASE)

      if (savedPreference !== null) {
        const value = savedPreference === 'true'
        includePrerelease.value = value
        await invoke('set_include_prerelease_preference', { include: value })
      } else {
        includePrerelease.value = await invoke('get_include_prerelease_preference')
        localStorage.setItem(STORAGE_KEYS.INCLUDE_PRERELEASE, includePrerelease.value.toString())
      }
    } catch (error) {
      console.error('加载内测偏好设置失败:', error)
      includePrerelease.value = getStoredBoolean(STORAGE_KEYS.INCLUDE_PRERELEASE)
    }
  }

  /**
   * 设置是否包含预发布版本的偏好
   */
  const setIncludePrerelease = async (value) => {
    includePrerelease.value = value
    localStorage.setItem(STORAGE_KEYS.INCLUDE_PRERELEASE, value.toString())

    try {
      const invoke = await getInvoke()
      await invoke('set_include_prerelease_preference', { include: value })
    } catch (error) {
      console.error('保存内测偏好设置失败:', error)
    }
  }

  /**
   * 初始化管理员状态
   */
  const initAdminStatus = async () => {
    try {
      const invoke = await getInvoke()
      isAdmin.value = await invoke('is_running_as_admin')
      console.log(isAdmin.value ? '✅ 当前以管理员权限运行' : '⚠️ 当前未以管理员权限运行')
    } catch (error) {
      console.error('检测管理员权限失败:', error)
    }
  }

  /**
   * 初始化主题
   */
  const initTheme = () => {
    const savedTheme = localStorage.getItem(STORAGE_KEYS.THEME)
    isDark.value = savedTheme
      ? savedTheme === THEME.DARK
      : window.matchMedia('(prefers-color-scheme: dark)').matches
    syncTheme()
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
      const invoke = await getInvoke()
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
    const handleMessage = ({ origin, data }) => {
      const isLocalhost = origin.includes('localhost') || origin.includes('127.0.0.1')
      if (isLocalhost && data.type === 'request-theme') {
        postMessageToIframes({ type: 'theme-sync', theme: currentTheme.value })
      }
    }
    window.addEventListener('message', handleMessage)
    cleanupFns.push(() => window.removeEventListener('message', handleMessage))

    // Tauri 事件监听
    const { listen } = await import('@tauri-apps/api/event')

    const unlistenUpdate = await listen('update-available', ({ payload }) => {
      if (!isVersionSkipped(payload?.version)) {
        updateInfo.value = payload
        showUpdateDialog.value = true
      }
    })
    cleanupFns.push(unlistenUpdate)

    const unlistenCheckResult = await listen('update-check-result', ({ payload }) => {
      const { is_latest, message, error } = payload
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
    initTheme()
    await Promise.all([
      initAdminStatus(),
      initWindowState(),
      initVersion(),
      initIncludePrerelease(),
      initEventListeners(),
    ])
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
    includePrerelease,
    router,

    // 方法
    toggleTheme,
    toggleCollapse,
    openVddSettings,
    openWelcome,
    goHome,
    skipVersion,
    isVersionSkipped,
    setIncludePrerelease,
  }
}
