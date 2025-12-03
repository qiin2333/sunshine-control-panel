import { ref, onMounted, onUnmounted } from 'vue'

/**
 * 窗口控制 composable
 * 提供窗口的最小化、最大化、关闭等功能
 */
export function useWindowControls() {
  const tauriWindow = ref(null)
  const isMaximized = ref(false)
  const isMinimized = ref(false)
  const isFocused = ref(true)

  let unlistenResize = null
  let unlistenFocus = null

  // 初始化 Tauri 窗口
  async function initWindow() {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      tauriWindow.value = getCurrentWindow()

      if (tauriWindow.value) {
        isMaximized.value = await tauriWindow.value.isMaximized()
        isMinimized.value = await tauriWindow.value.isMinimized()
        isFocused.value = await tauriWindow.value.isFocused()

        // 监听窗口大小变化
        unlistenResize = await tauriWindow.value.onResized(async () => {
          if (tauriWindow.value) {
            isMaximized.value = await tauriWindow.value.isMaximized()
            isMinimized.value = await tauriWindow.value.isMinimized()
          }
        })

        // 监听窗口焦点变化
        unlistenFocus = await tauriWindow.value.onWindowEvent((event) => {
          if (event.event === 'tauri://focus') {
            isFocused.value = true
          } else if (event.event === 'tauri://blur') {
            isFocused.value = false
          }
        })
      }
    } catch (e) {
      console.log('Tauri API not available, running in browser mode:', e)
    }
  }

  // 最小化窗口
  async function minimize() {
    if (tauriWindow.value) {
      await tauriWindow.value.minimize()
      isMinimized.value = true
    }
  }

  // 最大化/还原窗口
  async function maximize() {
    if (tauriWindow.value) {
      await tauriWindow.value.maximize()
      isMaximized.value = true
    }
  }

  // 还原窗口
  async function unmaximize() {
    if (tauriWindow.value) {
      await tauriWindow.value.unmaximize()
      isMaximized.value = false
    }
  }

  // 切换最大化状态
  async function toggleMaximize() {
    if (tauriWindow.value) {
      await tauriWindow.value.toggleMaximize()
      isMaximized.value = await tauriWindow.value.isMaximized()
    }
  }

  // 关闭窗口
  async function close() {
    if (tauriWindow.value) {
      await tauriWindow.value.close()
    }
  }

  // 显示窗口
  async function show() {
    if (tauriWindow.value) {
      await tauriWindow.value.show()
      isMinimized.value = false
    }
  }

  // 隐藏窗口
  async function hide() {
    if (tauriWindow.value) {
      await tauriWindow.value.hide()
    }
  }

  // 聚焦窗口
  async function setFocus() {
    if (tauriWindow.value) {
      await tauriWindow.value.setFocus()
      isFocused.value = true
    }
  }

  // 居中窗口
  async function center() {
    if (tauriWindow.value) {
      await tauriWindow.value.center()
    }
  }

  // 设置窗口大小
  async function setSize(width, height) {
    if (tauriWindow.value) {
      await tauriWindow.value.setSize({ width, height })
    }
  }

  // 获取窗口大小
  async function getSize() {
    if (tauriWindow.value) {
      return await tauriWindow.value.innerSize()
    }
    return null
  }

  onMounted(() => {
    initWindow()
  })

  onUnmounted(() => {
    if (unlistenResize) {
      unlistenResize()
    }
    if (unlistenFocus) {
      unlistenFocus()
    }
  })

  return {
    tauriWindow,
    isMaximized,
    isMinimized,
    isFocused,
    minimize,
    maximize,
    unmaximize,
    toggleMaximize,
    close,
    show,
    hide,
    setFocus,
    center,
    setSize,
    getSize,
  }
}
