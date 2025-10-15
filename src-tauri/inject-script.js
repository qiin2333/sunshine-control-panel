// Sunshine Web UI 注入脚本
// 用于主题同步、导航检测和拖放功能

;(function () {
  // 标记为 Tauri 环境
  window.isTauri = true
  window.electron = window.electron || {} // For compatibility

  // 创建消息通道用于与 parent 窗口通信
  let messageId = 0
  const pendingMessages = new Map()

  // ========== 消息监听器 ==========
  window.addEventListener('message', function (event) {
    // 处理 API 调用响应
    if (event.data && event.data.type === 'api-response') {
      const { id, result, error } = event.data
      const pending = pendingMessages.get(id)
      if (pending) {
        if (error) {
          pending.reject(new Error(error))
        } else {
          pending.resolve(result)
        }
        pendingMessages.delete(id)
      }
      return
    }

    // 处理主题同步
    if (event.data && event.data.type === 'theme-sync') {
      const theme = event.data.theme
      console.log('🎨 收到主题:', theme)
      document.body.setAttribute('data-bs-theme', theme)
      return
    }

    // 处理背景图片设置
    if (event.data && event.data.type === 'set-background') {
      const dataUrl = event.data.dataUrl
      const filePath = event.data.filePath
      document.body.style.backgroundImage = 'url("' + dataUrl + '")'
      if (filePath) {
        localStorage.setItem('WEBUI-BGSRC-PATH', filePath)
      }
      return
    }
  })

  // ========== API 调用函数 ==========
  function callParentApi(command, args = {}) {
    return new Promise((resolve, reject) => {
      const id = messageId++
      pendingMessages.set(id, { resolve, reject })

      window.parent.postMessage(
        {
          type: 'tauri-invoke',
          id: id,
          command: command,
          args: args,
        },
        '*'
      )

      // 超时处理
      setTimeout(() => {
        if (pendingMessages.has(id)) {
          pendingMessages.delete(id)
          reject(new Error('API call timeout'))
        }
      }, 10000)
    })
  }

  // ========== 暴露给 Sunshine 的 API ==========

  // 存储最后选择的文件路径
  let lastSelectedFilePath = null

  // webUtils API（Electron 兼容）
  window.electron.webUtils = {
    /**
     * 获取文件路径（Electron 兼容 API）
     * 在 Tauri 中，我们返回通过文件对话框选择的路径
     * @param {File} file - File 对象
     * @returns {string} 文件路径
     */
    getPathForFile: function (file) {
      if (!file) {
        console.error('❌ getPathForFile: file 参数为空')
        return ''
      }

      // 如果 File 对象有 path 属性（非标准，某些环境支持）
      if (file.path) {
        console.log('✅ 使用 File.path:', file.path)
        return file.path
      }

      // 如果之前通过对话框选择了文件，返回该路径
      if (lastSelectedFilePath) {
        console.log('✅ 使用缓存的文件路径:', lastSelectedFilePath)
        const path = lastSelectedFilePath
        lastSelectedFilePath = null // 清除缓存
        return path
      }

      // 否则返回文件名（降级方案）
      console.warn('⚠️  无法获取文件路径，返回文件名:', file.name)
      console.warn('    提示：在 Tauri 中应使用文件对话框选择文件')
      return file.name
    },
  }

  // ICC 文件列表 API
  window.getIccFileList = window.electron.getIccFileList = async function (callback) {
    try {
      const result = await callParentApi('get_icc_file_list')
      if (callback) callback(result)
      return result
    } catch (error) {
      console.error('获取 ICC 文件列表失败:', error)
      if (callback) callback([])
      return []
    }
  }

  // 读取目录 API
  window.readDirectory = async function (path, callback) {
    try {
      const result = await callParentApi('read_directory', { path: path })
      if (callback) callback(result)
      return result
    } catch (error) {
      console.error('读取目录失败:', error)
      if (callback) callback({ files: [], dirs: [] })
      return { files: [], dirs: [] }
    }
  }

  // ========== 背景图片功能 ==========

  const initBgImg = function () {
    // 清理旧的 Base64 存储（如果存在）
    const oldDataUrl = localStorage.getItem('WEBUI-BGSRC')
    if (oldDataUrl) {
      localStorage.removeItem('WEBUI-BGSRC')
    }

    // 从路径恢复背景图片
    const savedPath = localStorage.getItem('WEBUI-BGSRC-PATH')
    if (savedPath) {
      window.parent.postMessage(
        {
          type: 'restore-background',
          path: savedPath,
        },
        '*'
      )
    }
  }

  // 页面加载完成后初始化背景图片功能
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initBgImg)
  } else {
    initBgImg()
  }

  // ========== 导航检测 ==========

  // 立即报告当前路径（页面加载完成时）
  const currentPath = window.location.pathname + window.location.search + window.location.hash
  window.parent.postMessage(
    {
      type: 'path-update',
      path: currentPath,
    },
    '*'
  )

  let lastPathname = window.location.pathname + window.location.search
  let isNavigating = false // 导航标志，防止重复触发

  // 方案1: 监听 Bootstrap Tab 事件（明确排除 tab 切换）
  document.addEventListener('shown.bs.tab', function (e) {
    console.log('📑 Tab 切换（不触发 loading）:', e.target)
    isNavigating = false // 确保 tab 切换不触发 loading
  })

  document.addEventListener('hide.bs.tab', function (e) {
    isNavigating = false // tab 开始切换时也重置标志
  })

  // 方案3: 使用 Navigation API（现代浏览器）
  if (window.navigation) {
    window.navigation.addEventListener('navigate', function (e) {
      // 跳过：拦截失败、hash 变化、下载、已在导航中
      if (!e.canIntercept || e.hashChange || e.downloadRequest || isNavigating) {
        return
      }

      const url = new URL(e.destination.url)
      const newPathname = url.pathname + url.search

      if (newPathname !== lastPathname) {
        console.log('🧭 Navigation API 检测到导航:', newPathname)
        isNavigating = true
        window.parent.postMessage(
          {
            type: 'navigation-start',
            path: newPathname + url.hash,
          },
          '*'
        )
        lastPathname = newPathname
      }
    })
  }

  // 页面加载完成后请求当前主题
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () {
      window.parent.postMessage({ type: 'request-theme' }, '*')
    })
  } else {
    window.parent.postMessage({ type: 'request-theme' }, '*')
  }

  console.log('✅ Sunshine Tauri 注入脚本已加载')
})()
