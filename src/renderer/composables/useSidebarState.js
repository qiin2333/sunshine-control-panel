import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'

/**
 * 侧边栏状态管理 Composable
 */
export function useSidebarState() {
  // 状态定义
  const isCollapsed = ref(false)
  const isDark = ref(true)
  const isMaximized = ref(false)
  const isAdmin = ref(true)
  const showVddSettings = ref(false)
  const showUpdateDialog = ref(false)
  const updateInfo = ref(null)
  const currentVersion = ref('0.0.0')

  /**
   * 切换主题
   */
  const toggleTheme = async () => {
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

  /**
   * 切换折叠状态
   */
  const toggleCollapse = () => {
    isCollapsed.value = !isCollapsed.value
  }

  /**
   * 打开 VDD 设置
   */
  const openVddSettings = () => {
    showVddSettings.value = true
  }

  /**
   * 初始化状态
   */
  const initState = async () => {
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

    // 获取当前 Sunshine 版本
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const sunshineVersion = await invoke('get_sunshine_version')
      currentVersion.value = sunshineVersion || 'Unknown'
    } catch (error) {
      console.error('获取 Sunshine 版本失败:', error)
      currentVersion.value = 'Unknown'
    }

    // 监听自动更新检查事件
    const { listen } = await import('@tauri-apps/api/event')
    listen('update-available', (event) => {
      console.log('收到更新可用事件:', event.payload)
      updateInfo.value = event.payload
      showUpdateDialog.value = true
    })

    // 监听更新检查结果事件（来自托盘菜单）
    listen('update-check-result', (event) => {
      const data = event.payload
      if (data.is_latest) {
        ElMessage.success(data.message || '已是最新版本')
      } else if (data.error) {
        ElMessage.error('检查更新失败: ' + data.error)
      }
    })
  }

  // 初始化
  onMounted(initState)

  return {
    // 状态
    isCollapsed,
    isDark,
    isMaximized,
    isAdmin,
    showVddSettings,
    showUpdateDialog,
    updateInfo,
    currentVersion,

    // 方法
    toggleTheme,
    toggleCollapse,
    openVddSettings,
  }
}

