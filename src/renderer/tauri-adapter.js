/**
 * Tauri API 适配层
 * 将 Electron IPC 调用迁移到 Tauri invoke 调用
 */

import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'

// 主题相关
export const darkMode = {
  async toggle() {
    return await invoke('toggle_dark_mode')
  },
  async system() {
    // Tauri 使用系统主题
    return true
  }
}

// 外部 URL
export async function openExternalUrl(url) {
  try {
    await open(url)
    return true
  } catch (error) {
    console.error('打开外部URL失败:', error)
    return false
  }
}

// VDD 设置相关
export const vdd = {
  async getGPUs() {
    try {
      const gpus = await invoke('get_gpus')
      return { success: true, data: gpus }
    } catch (error) {
      return { success: false, message: error }
    }
  },
  
  async loadSettings() {
    try {
      const settings = await invoke('load_vdd_settings')
      return { success: true, data: settings }
    } catch (error) {
      return { success: false, data: null, message: error }
    }
  },
  
  async saveSettings(settings) {
    try {
      await invoke('save_vdd_settings', { settings })
      return { success: true }
    } catch (error) {
      return { success: false, message: error }
    }
  },
  
  async execPipeCmd(command) {
    try {
      await invoke('exec_pipe_cmd', { command })
      return true
    } catch (error) {
      console.error('执行管道命令失败:', error)
      return false
    }
  }
}

// Sunshine 配置相关
export const sunshine = {
  async getVersion() {
    try {
      return await invoke('get_sunshine_version')
    } catch (error) {
      console.error('获取 Sunshine 版本失败:', error)
      return 'Unknown'
    }
  },
  
  async parseConfig() {
    try {
      return await invoke('parse_sunshine_config')
    } catch (error) {
      console.error('解析配置失败:', error)
      return {}
    }
  },
  
  async getUrl() {
    try {
      return await invoke('get_sunshine_url')
    } catch (error) {
      console.error('获取 Sunshine URL 失败:', error)
      return 'https://localhost:47990/'
    }
  },
  
  async getCommandLineUrl() {
    try {
      return await invoke('get_command_line_url')
    } catch (error) {
      console.error('获取命令行 URL 失败:', error)
      return null
    }
  }
}

// 系统工具相关
export const tools = {
  async restartGraphicsDriver() {
    try {
      return await invoke('restart_graphics_driver')
    } catch (error) {
      console.error('重启显卡驱动失败:', error)
      throw error
    }
  },
  
  async restartSunshineService() {
    try {
      return await invoke('restart_sunshine_service')
    } catch (error) {
      console.error('重启 Sunshine 服务失败:', error)
      throw error
    }
  },
  
  async uninstallVddDriver() {
    try {
      return await invoke('uninstall_vdd_driver')
    } catch (error) {
      console.error('卸载 VDD 驱动失败:', error)
      throw error
    }
  }
}

// 文件系统相关（如果需要）
export async function readDirectory(path) {
  // Tauri 使用 fs API
  return []
}

// 导出统一的 API 对象
export default {
  darkMode,
  openExternalUrl,
  vdd,
  sunshine,
  tools,
  readDirectory
}


