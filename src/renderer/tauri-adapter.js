/**
 * Tauri API 适配层
 * 将 Electron IPC 调用迁移到 Tauri invoke 调用
 */

import { invoke } from '@tauri-apps/api/core'

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

export function isAllowedExternalUrl(url) {
  return /^(https?:\/\/|ms-windows-store:\/\/)/i.test(String(url || '').trim())
}

export async function openExternalUrl(url) {
  try {
    if (!isAllowedExternalUrl(url)) return false
    return await invoke('open_external_url', { url })
  } catch (error) {
    console.error('打开外部URL失败:', error)
    return false
  }
}

// ─── VDD 设置 ────────────────────────────────────────────

export const vdd = {
  getStatus: () => wrapResult('get_vdd_status'),
  install: () => wrapResult('install_vdd_driver'),
  uninstall: () => wrapResult('uninstall_vdd_driver'),
  setKeepEnabled: (enabled) => wrapResult('set_vdd_keep_enabled', { enabled }),
  setHeadlessCreateEnabled: (enabled) => wrapResult('set_vdd_headless_create_enabled', { enabled }),
  getGPUs: () => wrapResult('get_gpus'),
  loadSettings: () => wrapResult('load_vdd_settings'),
  saveSettings: (settings) => wrapResult('save_vdd_settings', { settings }),
  getEdidFilePath: () => wrapResult('get_vdd_edid_file_path'),
  uploadEdidFile: (fileData) => wrapResult('upload_edid_file', { fileData }),
  readEdidFile: () => wrapResult('read_edid_file'),
  deleteEdidFile: () => wrapResult('delete_edid_file'),
  getTraceStatus: () => wrapResult('get_vdd_trace_status'),
  startTrace: () => wrapResult('start_vdd_trace'),
  stopTrace: () => wrapResult('stop_vdd_trace'),
  openTraceFolder: () => wrapResult('open_vdd_trace_folder'),
  launchHdrCalibration: () => wrapResult('launch_windows_hdr_calibration'),

  async execVddCmd(command) {
    try {
      await invoke('exec_vdd_cmd', { command })
      return true
    } catch (error) {
      console.error('执行 VDD 命令失败:', error)
      throw error instanceof Error
        ? error
        : new Error(typeof error === 'string' && error.trim() ? error : '执行 VDD 命令失败')
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

// ─── ViGEmBus Virtual Gamepad ──────────────────────

export const vigem = {
  getStatus: () => wrapResult('get_vigem_status'),
  install: (force = false) => wrapResult('install_vigem_driver', { force: !!force }),
  uninstall: () => wrapResult('uninstall_vigem_driver'),
}

// ─── 控制器中心（仿真模式 / DS4 行为 / DSU） ───────────────

export const controllerHub = {
  getConfig: () => wrapResult('get_controller_hub_config'),
  saveConfig: (patch = {}) => wrapResult('save_controller_hub_config', {
    gamepad: patch.gamepad,
    motionAsDs4: patch.motion_as_ds4,
    touchpadAsDs4: patch.touchpad_as_ds4,
    ds4BackAsTouchpadClick: patch.ds4_back_as_touchpad_click,
    enableDsuServer: patch.enable_dsu_server,
    dsuServerPort: patch.dsu_server_port,
  }),
}

export const virtualMicrophone = {
  getStatus: () => wrapResult('get_virtual_microphone_status'),
  test: () => wrapResult('test_virtual_microphone'),
}

export const dualsense = {
  getStatus: () => wrapResult('dualsense_get_status'),
  logPanelOpened: () => wrapResult('dualsense_log_panel_opened'),
  install: (packagePaths = []) => wrapResult('dualsense_install', { packagePaths }),
  uninstall: () => wrapResult('dualsense_uninstall'),
  setConfig: (enabled, audioHaptics, genshinCompatibility) => wrapResult('dualsense_set_config', {
    enabled: !!enabled,
    audioHaptics: !!audioHaptics,
    genshinCompatibility: !!genshinCompatibility,
  }),
  setHapticsTuning: (strength, curve, noiseGate) => wrapResult('dualsense_set_haptics_tuning', {
    strength,
    curve,
    noiseGate,
  }),
  selfTest: (profile) => wrapResult('dualsense_self_test', { profile }),
}

// ─── Sunshine 配置 ───────────────────────────────────────

export const sunshine = {
  getVersion: () => wrapDefault('get_sunshine_version', 'Unknown'),
  parseConfig: () => wrapDefault('parse_sunshine_config', {}),
  getUrl: () => wrapDefault('get_sunshine_url', 'https://localhost:47990/'),
  getCommandLineUrl: () => wrapDefault('get_command_line_url', null),
  getProxyUrl: () => wrapDefault('get_proxy_url_command', 'http://localhost:48081'),
  refreshTarget: () => wrapDefault('refresh_sunshine_target', null),
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

// Folder Sharing

export const fileMapping = {
  list: () => invoke('list_file_mappings'),
  quickShareFolder: (path) => invoke('quick_share_folder', { path }),
  remove: (id) => invoke('delete_file_mapping', { id }),
  update: (id, patch) => invoke('update_file_mapping', { id, patch }),
  installMenu: () => invoke('install_file_mapping_menu'),
  uninstallMenu: () => invoke('uninstall_file_mapping_menu'),
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
  return wrapDefault('read_directory', [], { path })
}

// ─── 导出 ────────────────────────────────────────────────

export default {
  darkMode,
  openExternalUrl,
  vdd,
  vmouse,
  vigem,
  controllerHub,
  virtualMicrophone,
  dualsense,
  sunshine,
  tools,
  fileMapping,
  moonlightWeb,
  controllerMeta,
  readDirectory,
}
