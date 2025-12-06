/**
 * Tauri 兼容性 Polyfill
 * 为了兼容原有的代码，提供与 Electron 类似的全局 API
 */

import { darkMode, openExternalUrl, vdd, sunshine, tools } from './tauri-adapter.js'

// 模拟 Electron 的 window.electron API（如果需要）
if (typeof window !== 'undefined') {
  // 为旧代码提供兼容性
  window.electron = {
    ipcRenderer: {
      invoke: async (channel, data) => {
        console.warn('请迁移到 Tauri API，不要使用 window.electron.ipcRenderer.invoke')

        // 提供基本的兼容性映射
        switch (channel) {
          case 'vdd:loadSettings':
            return await vdd.loadSettings()
          case 'vdd:saveSettings':
            return await vdd.saveSettings(data)
          case 'vdd:getGPUs':
            return await vdd.getGPUs()
          case 'vdd:execPipeCmd':
            return await vdd.execPipeCmd(data)
          case 'dark-mode:toggle':
            return await darkMode.toggle()
          case 'dark-mode:system':
            return await darkMode.system()
          case 'openExternalUrl':
            return await openExternalUrl(data)
          default:
            console.error(`未知的 IPC channel: ${channel}`)
            return { success: false, message: '未实现的功能' }
        }
      },
    },
    webUtils: {
      /**
       * 获取文件的本地路径（Electron 兼容 API）
       * 在 Electron 中，这会将 File 对象转换为文件系统路径
       * 在 Tauri 中，我们返回 Object URL 以便在 Web UI 中使用
       * @param {File} file - File 对象
       * @returns {string} 文件的 Object URL 或路径
       */
      getPathForFile: (file) => {
        if (!file) {
          console.error('❌ getPathForFile: file 参数为空')
          return ''
        }

        // 如果 File 对象有 path 属性（非标准，但某些环境支持）
        if (file.path) {
          console.log('✅ 使用 File.path:', file.path)
          return file.path
        }

        // 在 Tauri/Web 环境中，创建 Object URL
        // 这样可以在 img 元素中直接使用
        console.log('📄 为文件创建 Object URL:', file.name)

        const reader = new FileReader()
        const objectUrl = URL.createObjectURL(file)

        // 异步读取文件为 Data URL（更持久）
        reader.onload = (e) => {
          console.log('✅ 文件已转换为 Data URL')
          // 触发自定义事件通知应用
          window.dispatchEvent(
            new CustomEvent('file-converted', {
              detail: {
                name: file.name,
                dataUrl: e.target.result,
              },
            })
          )
        }
        reader.readAsDataURL(file)

        // 返回临时 Object URL（立即可用）
        return objectUrl
      },
    },
  }

  // 提供 darkMode 全局 API
  window.darkMode = darkMode
}

export default {
  darkMode,
  vdd,
  sunshine,
  tools,
  openExternalUrl,
}
