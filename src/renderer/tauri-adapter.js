/**
 * Tauri API 适配层
 * 将 Electron IPC 调用迁移到 Tauri invoke 调用
 */

import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'

// ─── Helpers ────────────────────────────────────────────

/** 调用 command，返回 { success, data, message } */
async function wrapResult(cmd, args) {
  try {
    const data = await invoke(cmd, args)
    return { success: true, data }
  } catch (error) {
    return { success: false, message: error }
  }
}

/** 调用 command，失败时返回 fallback */
async function wrapDefault(cmd, fallback, args) {
  try {
    return await invoke(cmd, args)
  } catch (error) {
    console.warn(`${cmd} failed:`, error)
    return fallback
  }
}

// ─── 主题 ────────────────────────────────────────────────

export const darkMode = {
  toggle: () => invoke('toggle_dark_mode'),
  system: async () => true,
}

// ─── 外部 URL ────────────────────────────────────────────

export async function openExternalUrl(url) {
  try {
    await open(url)
    return true
  } catch (error) {
    console.error('打开外部URL失败:', error)
    return false
  }
}

// ─── VDD 设置 ────────────────────────────────────────────

export const vdd = {
  getGPUs: () => wrapResult('get_gpus'),
  loadSettings: () => wrapResult('load_vdd_settings'),
  saveSettings: (settings) => wrapResult('save_vdd_settings', { settings }),
  getEdidFilePath: () => wrapResult('get_vdd_edid_file_path'),
  uploadEdidFile: (fileData) => wrapResult('upload_edid_file', { fileData }),
  readEdidFile: () => wrapResult('read_edid_file'),
  deleteEdidFile: () => wrapResult('delete_edid_file'),

  async execPipeCmd(command) {
    try {
      await invoke('exec_pipe_cmd', { command })
      return true
    } catch (error) {
      console.error('执行管道命令失败:', error)
      return false
    }
  },
}

// ─── Virtual Mouse ───────────────────────────────────────

export const vmouse = {
  getStatus: () => wrapResult('get_vmouse_status'),
  install: () => wrapResult('install_vmouse_driver'),
  uninstall: () => wrapResult('uninstall_vmouse_driver'),
  setConfig: (enabled) => wrapResult('set_vmouse_config', { enabled }),
}

// ─── Sunshine 配置 ───────────────────────────────────────

export const sunshine = {
  getVersion: () => wrapDefault('get_sunshine_version', 'Unknown'),
  parseConfig: () => wrapDefault('parse_sunshine_config', {}),
  getUrl: () => wrapDefault('get_sunshine_url', 'https://localhost:47990/'),
  getCommandLineUrl: () => wrapDefault('get_command_line_url', null),
  getProxyUrl: () => wrapDefault('get_proxy_url_command', 'http://localhost:48081'),
  getActiveSessions: () => wrapDefault('get_active_sessions', []),
  getLocale: () => wrapDefault('get_sunshine_locale', 'en'),
  setLocale: (locale) => invoke('set_sunshine_locale', { locale }),
  changeBitrate: (clientName, bitrate) => invoke('change_bitrate', { clientName, bitrate }),
}

// ─── 系统工具 ────────────────────────────────────────────

export const tools = {
  restartGraphicsDriver: () => invoke('restart_graphics_driver'),
  restartSunshineService: () => invoke('restart_sunshine_service'),
  restartSunshineInUserMode: () => invoke('restart_sunshine_in_user_mode'),
  uninstallVddDriver: () => invoke('uninstall_vdd_driver'),
}

// ─── Moonlight Web ───────────────────────────────────────

export const moonlightWeb = {
  getStatus: () => wrapDefault('moonlight_web_get_status',
    { installed: false, running: false, install_path: '', version: '', access_url: '', port: 8080 }),
  getConfig: () => wrapDefault('moonlight_web_get_config',
    { web_server: { bind_address: '0.0.0.0:8080' }, webrtc: null, default_settings: null }),
  start: () => invoke('moonlight_web_start'),
  stop: () => invoke('moonlight_web_stop'),
  saveConfig: (config) => invoke('moonlight_web_save_config', { config }),
  checkRelease: () => invoke('moonlight_web_check_release'),
  download: (url, version) => invoke('moonlight_web_download', { url, version: version || '' }),
  getInstallPath: () => invoke('moonlight_web_get_install_path'),
  generateCert: () => invoke('moonlight_web_generate_cert'),
}

// ─── ControllerMeta ──────────────────────────────────────

export const controllerMeta = {
  getStatus: () => wrapDefault('controllermeta_get_status',
    { installed: false, running: false, install_path: '', binary_path: '', version: '' }),
  checkRelease: () => invoke('controllermeta_check_release'),
  download: (url, version) => invoke('controllermeta_download', { url, version: version || '' }),
  launch: () => invoke('controllermeta_launch'),
  getInstallPath: () => invoke('controllermeta_get_install_path'),
  uninstall: () => invoke('controllermeta_uninstall'),
}

// ─── 文件系统 ────────────────────────────────────────────

export async function readDirectory(path) {
  return []
}

// ─── 导出 ────────────────────────────────────────────────

export default {
  darkMode,
  openExternalUrl,
  vdd,
  vmouse,
  sunshine,
  tools,
  moonlightWeb,
  controllerMeta,
  readDirectory,
}
