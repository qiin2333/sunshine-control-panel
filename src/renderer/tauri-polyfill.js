/**
 * Tauri 兼容性 Polyfill
 * 为了兼容原有的代码，提供与 Electron 类似的全局 API
 */

import { darkMode, openExternalUrl, vdd, sunshine, tools } from './tauri-adapter.js'

// IPC 通道映射表
const IPC_HANDLERS = {
  'vdd:loadSettings': () => vdd.loadSettings(),
  'vdd:saveSettings': (data) => vdd.saveSettings(data),
  'vdd:getGPUs': () => vdd.getGPUs(),
  'vdd:execPipeCmd': (data) => vdd.execPipeCmd(data),
  'dark-mode:toggle': () => darkMode.toggle(),
  'dark-mode:system': () => darkMode.system(),
  openExternalUrl: (data) => openExternalUrl(data),
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

// 主题切换功能
export function initTheme() {
  if (typeof document === 'undefined') return

  const body = document.body
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

  const updateTheme = (isDark) => {
    body.setAttribute('data-bs-theme', isDark ? 'dark' : 'light')
  }

  updateTheme(mediaQuery.matches)
  mediaQuery.addEventListener('change', (e) => updateTheme(e.matches))
}

// 自动初始化
if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initTheme)
  } else {
    initTheme()
  }
}

export default {
  initTheme,
  darkMode,
  vdd,
  sunshine,
  tools,
  openExternalUrl,
}
