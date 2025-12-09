/**
 * Tauri 兼容性 Polyfill
 * 为了兼容原有的代码，提供与 Electron 类似的全局 API
 */

import { darkMode, openExternalUrl, vdd, sunshine, tools } from './tauri-adapter.js'

// IPC 通道映射表
const IPC_HANDLERS = {
  'vdd:loadSettings': vdd.loadSettings,
  'vdd:saveSettings': vdd.saveSettings,
  'vdd:getGPUs': vdd.getGPUs,
  'vdd:execPipeCmd': vdd.execPipeCmd,
  'dark-mode:toggle': darkMode.toggle,
  'dark-mode:system': darkMode.system,
  openExternalUrl,
}

/**
 * 获取文件的本地路径（Electron 兼容 API）
 * @param {File} file - File 对象
 * @returns {string} 文件的 Object URL 或路径
 */
const getPathForFile = (file) => {
  if (!file) {
    console.error('❌ getPathForFile: file 参数为空')
    return ''
  }

  // 优先使用 File.path（非标准，但某些环境支持）
  if (file.path) {
    return file.path
  }

  // 创建 Object URL 供立即使用
  const objectUrl = URL.createObjectURL(file)

  // 异步转换为 Data URL（更持久）
  const reader = new FileReader()
  reader.onload = ({ target }) => {
    window.dispatchEvent(
      new CustomEvent('file-converted', {
        detail: { name: file.name, dataUrl: target.result },
      })
    )
  }
  reader.readAsDataURL(file)

  return objectUrl
}

// 模拟 Electron 的 window.electron API
if (typeof window !== 'undefined') {
  window.electron = {
    ipcRenderer: {
      invoke: async (channel, data) => {
        const handler = IPC_HANDLERS[channel]
        if (handler) {
          return handler(data)
        }
        console.error(`未知的 IPC channel: ${channel}`)
        return { success: false, message: '未实现的功能' }
      },
    },
    webUtils: { getPathForFile },
  }

  window.darkMode = darkMode
}

// 检测是否为生产环境
const isProductionEnv = () => {
  if (typeof __PROD__ !== 'undefined') return __PROD__ === true
  if (typeof __DEV__ !== 'undefined') return __DEV__ === false
  try {
    return import.meta.env?.PROD === true
  } catch {
    return false
  }
}

// 检测是否为 Tauri 环境
const isTauriEnv = () => 
  typeof window !== 'undefined' && (window.__TAURI__ || window.isTauri)

// 禁用右键菜单的函数（仅在生产环境）
let contextMenuHandler = null
let keydownHandler = null

const disableContextMenu = () => {
  if (typeof document === 'undefined' || !isProductionEnv() || !isTauriEnv()) {
    return
  }
  
  // 移除旧的事件监听器
  if (contextMenuHandler) {
    document.removeEventListener('contextmenu', contextMenuHandler, true)
  }
  if (keydownHandler) {
    document.removeEventListener('keydown', keydownHandler, true)
  }
  
  // 禁用右键菜单
  contextMenuHandler = (e) => {
    e.preventDefault()
    return false
  }
  
  // 禁用开发者工具快捷键
  const blockedKeys = new Set([
    123,  // F12
  ])
  const blockedCtrlShiftKeys = new Set([73, 74])  // I, J
  const blockedCtrlKeys = new Set([85])  // U
  
  keydownHandler = (e) => {
    if (blockedKeys.has(e.keyCode) ||
        (e.ctrlKey && e.shiftKey && blockedCtrlShiftKeys.has(e.keyCode)) ||
        (e.ctrlKey && !e.shiftKey && blockedCtrlKeys.has(e.keyCode))) {
      e.preventDefault()
      return false
    }
  }
  
  document.addEventListener('contextmenu', contextMenuHandler, true)
  document.addEventListener('keydown', keydownHandler, true)
}

// 主题切换功能
export function initTheme() {
  if (typeof document === 'undefined') return

  const html = document.documentElement
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

  const updateTheme = (isDark) => {
    const theme = isDark ? 'dark' : 'light'
    html.setAttribute('data-bs-theme', theme)
  }

  updateTheme(mediaQuery.matches)
  mediaQuery.addEventListener('change', (e) => updateTheme(e.matches))
}

// 监听导航事件（SPA 应用）
const initNavigationListener = () => {
  if (typeof window === 'undefined' || !window.navigation) return
  
  window.navigation.addEventListener('navigate', (e) => {
    if (!e.canIntercept || e.hashChange || e.downloadRequest) return
    setTimeout(disableContextMenu, 200)
  })
}

// 自动初始化
if (typeof document !== 'undefined') {
  const init = () => {
    initTheme()
    disableContextMenu()
    initNavigationListener()
  }
  
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init)
    window.addEventListener('load', disableContextMenu)
  } else {
    init()
  }
}

export default {
  initTheme,
  disableContextMenu,
  darkMode,
  vdd,
  sunshine,
  tools,
  openExternalUrl,
}
