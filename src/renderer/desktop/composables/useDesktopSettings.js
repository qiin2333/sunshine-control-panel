import { ref } from 'vue'
import { tauriInvoke } from './useTauri'

export const DESKTOP_SETTINGS_KEY = 'sunshine-desktop-settings'
export const DESKTOP_SETTINGS_UPDATED = 'desktop-settings-updated'

export const defaultDesktopSettings = {
  autoStart: false,
  startMinimized: false,
  autoStartSunshine: true,
  notifications: true,
  connectionNotify: true,
  updateNotify: true,
  devMode: false,
  logLevel: 'info',
}

export const desktopSettings = ref({ ...defaultDesktopSettings })
export const desktopSettingsStatus = ref(null)

function normalize(settings = {}) {
  return {
    ...defaultDesktopSettings,
    ...settings,
    logLevel: ['error', 'warn', 'info', 'debug', 'trace'].includes(settings.logLevel)
      ? settings.logLevel
      : defaultDesktopSettings.logLevel,
  }
}

function persistLocal(settings) {
  try {
    localStorage.setItem(DESKTOP_SETTINGS_KEY, JSON.stringify(settings))
  } catch {
    // ignore storage failures
  }
}

function loadLocal() {
  try {
    const saved = localStorage.getItem(DESKTOP_SETTINGS_KEY)
    return saved ? normalize(JSON.parse(saved)) : { ...defaultDesktopSettings }
  } catch {
    return { ...defaultDesktopSettings }
  }
}

export async function loadDesktopSettings() {
  try {
    const response = await tauriInvoke('get_desktop_settings')
    desktopSettings.value = normalize(response.settings)
    desktopSettingsStatus.value = response.status || null
    persistLocal(desktopSettings.value)
  } catch {
    desktopSettings.value = loadLocal()
  }
  return desktopSettings.value
}

export async function saveDesktopSettings(nextSettings) {
  const normalized = normalize(nextSettings)
  let response = null
  try {
    response = await tauriInvoke('save_desktop_settings', { settings: normalized })
    desktopSettings.value = normalize(response.settings)
    desktopSettingsStatus.value = response.status || null
  } catch (e) {
    desktopSettings.value = normalized
    persistLocal(desktopSettings.value)
    window.dispatchEvent(new CustomEvent(DESKTOP_SETTINGS_UPDATED, { detail: desktopSettings.value }))
    throw e
  }
  persistLocal(desktopSettings.value)
  window.dispatchEvent(new CustomEvent(DESKTOP_SETTINGS_UPDATED, { detail: desktopSettings.value }))
  return response
}

export async function requestNotificationPermission(settings = desktopSettings.value) {
  if (!settings.notifications || !('Notification' in window)) return false
  if (Notification.permission === 'granted') return true
  if (Notification.permission === 'denied') return false
  return (await Notification.requestPermission()) === 'granted'
}

export function showDesktopNotification(title, body, settings = desktopSettings.value) {
  if (!settings.notifications || !('Notification' in window) || Notification.permission !== 'granted') {
    return false
  }
  new Notification(title, { body })
  return true
}
