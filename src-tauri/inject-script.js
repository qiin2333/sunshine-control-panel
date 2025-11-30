// Sunshine Web UI 注入脚本
// 用于主题同步、导航检测和拖放功能

;(function () {
  window.isTauri = true
  window.electron = window.electron || {}

  let messageId = 0
  const pendingMessages = new Map()

  // 消息监听器
  window.addEventListener('message', function (event) {
    const { data } = event
    if (!data || !data.type) return

    switch (data.type) {
      case 'api-response': {
        const pending = pendingMessages.get(data.id)
        if (pending) {
          data.error ? pending.reject(new Error(data.error)) : pending.resolve(data.result)
          pendingMessages.delete(data.id)
        }
        break
      }
      case 'theme-sync':
        document.body.setAttribute('data-bs-theme', data.theme)
        break
      case 'set-background':
        document.body.style.backgroundImage = `url("${data.dataUrl}")`
        if (data.filePath) localStorage.setItem('WEBUI-BGSRC-PATH', data.filePath)
        break
    }
  })

  // API 调用
  function callParentApi(command, args = {}) {
    return new Promise((resolve, reject) => {
      const id = messageId++
      pendingMessages.set(id, { resolve, reject })
      window.parent.postMessage({ type: 'tauri-invoke', id, command, args }, '*')
      setTimeout(() => {
        if (pendingMessages.delete(id)) reject(new Error('API call timeout'))
      }, 10000)
    })
  }

  // 文件路径缓存
  let lastSelectedFilePath = null

  // Electron 兼容 API
  window.electron.webUtils = {
    getPathForFile(file) {
      if (!file) return ''
      if (file.path) return file.path
      if (lastSelectedFilePath) {
        const path = lastSelectedFilePath
        lastSelectedFilePath = null
        return path
      }
      return file.name
    },
  }

  // ICC 文件列表 API
  window.getIccFileList = window.electron.getIccFileList = async function (callback) {
    try {
      const result = await callParentApi('get_icc_file_list')
      callback?.(result)
      return result
    } catch {
      callback?.([])
      return []
    }
  }

  // 读取目录 API
  window.readDirectory = async function (path, callback) {
    try {
      const result = await callParentApi('read_directory', { path })
      callback?.(result)
      return result
    } catch {
      const empty = { files: [], dirs: [] }
      callback?.(empty)
      return empty
    }
  }

  // 导航检测
  function initNavigation() {
    const currentPath = window.location.pathname + window.location.search + window.location.hash
    window.parent.postMessage({ type: 'path-update', path: currentPath }, '*')

    let lastPathname = window.location.pathname + window.location.search

    if (window.navigation) {
      window.navigation.addEventListener('navigate', function (e) {
        if (!e.canIntercept || e.hashChange || e.downloadRequest) return
        const url = new URL(e.destination.url)
        const newPathname = url.pathname + url.search
        if (newPathname !== lastPathname) {
          window.parent.postMessage({ type: 'navigation-start', path: newPathname + url.hash }, '*')
          lastPathname = newPathname
        }
      })
    }
  }

  // 初始化
  function init() {
    initNavigation()
    window.parent.postMessage({ type: 'request-theme' }, '*')
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init)
  } else {
    init()
  }
})()
