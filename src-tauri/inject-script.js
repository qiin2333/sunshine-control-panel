// Sunshine Web UI 注入脚本
// 用于主题同步、导航检测和拖放功能

;(function () {
  window.isTauri = true
  window.electron ??= {}

  // 消息处理器映射
  const messageHandlers = {
    'theme-sync': ({ theme }) => {
      document.documentElement.setAttribute('data-bs-theme', theme)
      localStorage.setItem('theme', theme)
    },
    'set-background': ({ dataUrl, filePath }) => {
      document.body.style.backgroundImage = `url("${dataUrl}")`
      filePath && localStorage.setItem('WEBUI-BGSRC-PATH', filePath)
    },
  }

  // 消息监听器
  window.addEventListener('message', ({ data }) => {
    data?.type && messageHandlers[data.type]?.(data)
  })

  // 发送消息到父窗口
  const postToParent = (type, payload = {}) => {
    window.parent.postMessage({ type, ...payload }, '*')
  }

  // Tauri API 辅助函数
  const invoke = (cmd, args) => window.__TAURI__?.core?.invoke?.(cmd, args)
  const openDialog = (options) => window.__TAURI__?.dialog?.open?.(options)

  // Electron 兼容 API（使用 Tauri API）
  window.electron.webUtils = {
    async getPathForFile(file) {
      if (!file) return ''
      if (file.path) return file.path

      try {
        const selected = await openDialog({
          title: '选择文件',
          filters: [{ name: '所有文件', extensions: ['*'] }],
          multiple: false,
          directory: false,
        })
        return selected || file.name
      } catch {
        return file.name
      }
    },
  }

  // ICC 文件列表 API
  const getIccFileList = async (callback) => {
    try {
      const result = await invoke('get_icc_file_list')
      if (result) {
        callback?.(result)
        return result
      }
    } catch {
      // 忽略错误
    }
    callback?.([])
    return []
  }
  window.getIccFileList = window.electron.getIccFileList = getIccFileList

  // 读取目录 API
  const emptyDir = { files: [], dirs: [] }
  window.readDirectory = async (path, callback) => {
    try {
      const result = await invoke('read_directory', { path })
      if (result) {
        callback?.(result)
        return result
      }
    } catch {
      // 忽略错误
    }
    callback?.(emptyDir)
    return emptyDir
  }

  // 环境检测
  const isProduction = () => window.TAURI_PRODUCTION === true && window.isTauri === true

  // 禁用右键菜单和开发者工具快捷键（仅在生产环境）
  const disableContextMenu = () => {
    if (!isProduction()) return

    const preventDefault = (e) => (e.preventDefault(), false)

    document.addEventListener('contextmenu', preventDefault, true)
    document.addEventListener(
      'keydown',
      (e) => {
        const { keyCode, ctrlKey, shiftKey } = e
        // F12 | Ctrl+Shift+I/J | Ctrl+U
        if (
          keyCode === 123 ||
          (ctrlKey && shiftKey && (keyCode === 73 || keyCode === 74)) ||
          (ctrlKey && !shiftKey && keyCode === 85)
        ) {
          return preventDefault(e)
        }
      },
      true
    )
  }

  // 导航检测
  const initNavigation = () => {
    const getFullPath = () => location.pathname + location.search + location.hash
    let lastPath = location.pathname + location.search

    postToParent('path-update', { path: getFullPath() })

    window.navigation?.addEventListener('navigate', (e) => {
      if (!e.canIntercept || e.hashChange || e.downloadRequest) return

      const { pathname, search, hash } = new URL(e.destination.url)
      const newPath = pathname + search

      if (newPath !== lastPath) {
        postToParent('navigation-start', { path: newPath + hash })
        lastPath = newPath
        setTimeout(disableContextMenu, 200)
      }
    })
  }

  // 初始化
  const init = () => {
    disableContextMenu()
    initNavigation()
    postToParent('request-theme')
  }

  document.readyState === 'loading' ? document.addEventListener('DOMContentLoaded', init) : init()
})()
