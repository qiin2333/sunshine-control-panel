import { ref, computed } from 'vue'
import { zh } from './zh.js'
import { en } from './en.js'

const messages = { zh, en }

const currentLocale = ref(localStorage.getItem('language') || 'zh')

// 从 Sunshine 配置同步语言设置（初始化时调用一次）
let syncInitialized = false
async function syncLocaleFromSunshine() {
  if (syncInitialized) return
  syncInitialized = true
  try {
    // 优先使用 tray 当前语言（避免新窗口初始化时覆盖 tray 语言）
    const { invoke } = await import('@tauri-apps/api/core')
    const trayLocale = await invoke('get_tray_locale')
    if (trayLocale && (trayLocale === 'zh' || trayLocale === 'en')) {
      if (trayLocale !== currentLocale.value) {
        currentLocale.value = trayLocale
        localStorage.setItem('language', trayLocale)
      }
      return // tray 已有语言状态，不需要再从 Sunshine 配置读取
    }
  } catch {
    // invoke 不可用，继续尝试 Sunshine 配置
  }
  try {
    const { sunshine } = await import('../../tauri-adapter.js')
    const sunshineLocale = await sunshine.getLocale()
    // Sunshine 用 'zh'/'zh_TW' 等，桌面 GUI 只有 'zh'/'en'
    const guiLocale = sunshineLocale.startsWith('zh') ? 'zh' : 'en'
    if (guiLocale !== currentLocale.value) {
      currentLocale.value = guiLocale
      localStorage.setItem('language', guiLocale)
    }
    // 同步当前语言到托盘
    syncLocaleToTray(guiLocale)
  } catch {
    // 非 Tauri 环境或 API 不可用，忽略
  }
}
syncLocaleFromSunshine()

// 监听托盘语言切换事件
async function listenTrayLocaleChanged() {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    listen('tray-locale-changed', (event) => {
      const newLocale = event.payload
      if (newLocale && newLocale !== currentLocale.value) {
        currentLocale.value = newLocale
        localStorage.setItem('language', newLocale)
        // 同步到 Sunshine 配置
        syncLocaleToSunshine(newLocale)
      }
    })
  } catch {
    // 非 Tauri 环境，忽略
  }
}
listenTrayLocaleChanged()

// 同步语言到托盘
async function syncLocaleToTray(locale) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('set_tray_locale', { locale })
  } catch {
    // 忽略
  }
}

export function useI18n() {
  const t = computed(() => messages[currentLocale.value] || messages.zh)
  const locale = computed({
    get: () => currentLocale.value,
    set: (val) => {
      currentLocale.value = val
      localStorage.setItem('language', val)
    },
  })
  const toggleLocale = () => {
    const newLocale = locale.value === 'zh' ? 'en' : 'zh'
    locale.value = newLocale
    // 异步同步到 Sunshine 配置
    syncLocaleToSunshine(newLocale)
    // 同步到托盘
    syncLocaleToTray(newLocale)
  }
  return { t, locale, toggleLocale }
}

async function syncLocaleToSunshine(locale) {
  try {
    const { sunshine } = await import('../../tauri-adapter.js')
    await sunshine.setLocale(locale)
    // 通知 SunshineFrame 刷新 iframe 以应用新语言
    window.dispatchEvent(new CustomEvent('locale-changed', { detail: { locale } }))
  } catch (e) {
    console.warn('Failed to sync locale to Sunshine:', e)
  }
}
