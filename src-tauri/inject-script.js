// Sunshine Web UI 注入脚本
// 用于主题同步、导航检测和拖放功能

;(function () {
  window.isTauri = true
  window.electron = window.electron || {}

  const TIMEOUT_MS = 10000
  let messageId = 0
  const pendingMessages = new Map()
  let lastSelectedFilePath = null

  // 消息处理器映射
  const messageHandlers = {
    'api-response': ({ id, error, result }) => {
      const pending = pendingMessages.get(id)
      if (!pending) return
      pendingMessages.delete(id)
      error ? pending.reject(new Error(error)) : pending.resolve(result)
    },
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

  // API 调用
  const callParentApi = (command, args = {}) =>
    new Promise((resolve, reject) => {
      const id = messageId++
      const timeoutId = setTimeout(() => {
        pendingMessages.delete(id) && reject(new Error(`API call timeout: ${command}`))
      }, TIMEOUT_MS)

      pendingMessages.set(id, {
        resolve: (result) => {
          clearTimeout(timeoutId)
          resolve(result)
        },
        reject: (error) => {
          clearTimeout(timeoutId)
          reject(error)
        },
      })
      postToParent('tauri-invoke', { id, command, args })
    })

  // Electron 兼容 API
  window.electron.webUtils = {
    async getPathForFile(file) {
      if (!file) return ''
      if (file.path) return file.path
      if (lastSelectedFilePath) {
        const cached = lastSelectedFilePath
        lastSelectedFilePath = null
        return cached
      }
      const { path } = await callParentApi('request_file_path', { file })
      return path || file.name
    },
  }

  // ICC 文件列表 API
  const getIccFileList = async (callback) => {
    try {
      const result = await callParentApi('get_icc_file_list')
      callback?.(result)
      return result
    } catch {
      callback?.([])
      return []
    }
  }
  window.getIccFileList = window.electron.getIccFileList = getIccFileList

  // 读取目录 API
  window.readDirectory = async (path, callback) => {
    const empty = { files: [], dirs: [] }
    try {
      const result = await callParentApi('read_directory', { path })
      callback?.(result)
      return result
    } catch {
      callback?.(empty)
      return empty
    }
  }

  // 注入 __TAURI__ 兼容对象
  window.__TAURI__ = window.__TAURI__ || {
    core: { invoke: callParentApi },
    event: { TauriEvent: { FileDrop: null } },
  }

  // 环境检测
  const isProductionEnv = () => window.TAURI_PRODUCTION === true
  const isTauriEnv = () => window.isTauri === true

  // 禁用右键菜单和开发者工具快捷键（仅在生产环境）
  const disableContextMenu = () => {
    if (!isProductionEnv() || !isTauriEnv()) return

    const preventDefault = (e) => {
      e.preventDefault()
      return false
    }

    // 禁用开发者工具快捷键
    const blockedKeys = new Set([123]) // F12
    const blockedCtrlShiftKeys = new Set([73, 74]) // I, J
    const blockedCtrlKeys = new Set([85]) // U

    const keydownHandler = (e) => {
      if (
        blockedKeys.has(e.keyCode) ||
        (e.ctrlKey && e.shiftKey && blockedCtrlShiftKeys.has(e.keyCode)) ||
        (e.ctrlKey && !e.shiftKey && blockedCtrlKeys.has(e.keyCode))
      ) {
        return preventDefault(e)
      }
    }

    document.addEventListener('contextmenu', preventDefault, true)
    document.addEventListener('keydown', keydownHandler, true)
  }

  // 导航检测
  const initNavigation = () => {
    const getFullPath = () => location.pathname + location.search + location.hash
    let lastPath = location.pathname + location.search

    postToParent('path-update', { path: getFullPath() })

    window.navigation?.addEventListener('navigate', (e) => {
      if (!e.canIntercept || e.hashChange || e.downloadRequest) return

      const url = new URL(e.destination.url)
      const newPath = url.pathname + url.search

      if (newPath !== lastPath) {
        postToParent('navigation-start', { path: newPath + url.hash })
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
