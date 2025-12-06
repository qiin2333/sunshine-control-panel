// Sunshine Web UI 注入脚本
// 用于主题同步、导航检测和拖放功能

;(function () {
  'use strict'

  window.isTauri = true
  window.electron = window.electron || {}

  const TIMEOUT_MS = 10000
  let messageId = 0
  const pendingMessages = new Map()

  // 消息处理器映射
  const messageHandlers = {
    'api-response': ({ id, error, result }) => {
      const pending = pendingMessages.get(id)
      if (!pending) return
      error ? pending.reject(new Error(error)) : pending.resolve(result)
      pendingMessages.delete(id)
    },
    'theme-sync': ({ theme }) => {
      document.body.setAttribute('data-bs-theme', theme)
    },
    'set-background': ({ dataUrl, filePath }) => {
      document.body.style.backgroundImage = `url("${dataUrl}")`
      if (filePath) localStorage.setItem('WEBUI-BGSRC-PATH', filePath)
    },
  }

  // 消息监听器
  window.addEventListener('message', ({ data }) => {
    if (!data?.type) return
    messageHandlers[data.type]?.(data)
  })

  // 发送消息到父窗口
  const postToParent = (type, payload = {}) => {
    window.parent.postMessage({ type, ...payload }, '*')
  }

  // API 调用
  const callParentApi = (command, args = {}) =>
    new Promise((resolve, reject) => {
      const id = messageId++
      pendingMessages.set(id, { resolve, reject })
      postToParent('tauri-invoke', { id, command, args })
      setTimeout(() => {
        if (pendingMessages.delete(id)) {
          reject(new Error(`API call timeout: ${command}`))
        }
      }, TIMEOUT_MS)
    })

  // 文件路径缓存
  let lastSelectedFilePath = null

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
    core: {
      invoke: callParentApi,
    },
    event: {
      TauriEvent: { FileDrop: null },
    },
  }

  // 导航检测
  const initNavigation = () => {
    const getPathWithQuery = () => window.location.pathname + window.location.search
    let lastPathname = getPathWithQuery()

    postToParent('path-update', { path: lastPathname + window.location.hash })

    window.navigation?.addEventListener('navigate', (e) => {
      if (!e.canIntercept || e.hashChange || e.downloadRequest) return

      const url = new URL(e.destination.url)
      const newPathname = url.pathname + url.search

      if (newPathname !== lastPathname) {
        postToParent('navigation-start', { path: newPathname + url.hash })
        lastPathname = newPathname
      }
    })
  }

  // 初始化
  const init = () => {
    initNavigation()
    postToParent('request-theme')
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init)
  } else {
    init()
  }
})()
