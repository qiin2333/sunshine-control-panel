import { ref, computed, watch } from 'vue'
import { zh } from './zh.js'
import { en } from './en.js'

const messages = { zh, en }

function normalizeDesktopLocale(locale) {
  return locale?.toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

function getDefaultLocale() {
  const saved = localStorage.getItem('language')
  if (saved === 'zh' || saved === 'en') return saved

  const browserLang = navigator.language || navigator.userLanguage || ''
  return normalizeDesktopLocale(browserLang)
}

const currentLocale = ref(getDefaultLocale())

function setDocumentLanguage(locale) {
  document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en'
}

watch(currentLocale, setDocumentLanguage, { immediate: true })

// 启动时恢复桌面 UI；托盘会在创建菜单前从同一个 locale 同步初始化。
let syncInitialized = false
async function syncLocaleFromSunshine() {
  if (syncInitialized) return
  syncInitialized = true
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const config = await invoke('parse_sunshine_config')
    if (!config?.locale) return
    // Sunshine 用 'zh'/'zh_TW' 等，桌面 GUI 只有 'zh'/'en'
    const guiLocale = normalizeDesktopLocale(config.locale)
    if (guiLocale !== currentLocale.value) {
      currentLocale.value = guiLocale
      localStorage.setItem('language', guiLocale)
    }
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
      if (!event.payload) return
      const newLocale = normalizeDesktopLocale(event.payload)
      if (newLocale !== currentLocale.value) {
        currentLocale.value = newLocale
        localStorage.setItem('language', newLocale)
      }
    })
  } catch {
    // 非 Tauri 环境，忽略
  }
}
listenTrayLocaleChanged()

// 用户明确切换时，通过同一条命令更新 Sunshine UI 和托盘语言。
async function syncLocalePreferences(locale) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('set_locale_preferences', { locale })
    window.dispatchEvent(new CustomEvent('locale-changed', { detail: { locale } }))
  } catch (error) {
    console.warn('Failed to sync locale preferences:', error)
  }
}

export function useI18n() {
  const t = computed(() => messages[currentLocale.value] || messages.en)
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
    syncLocalePreferences(newLocale)
  }
  return { t, locale, toggleLocale }
}
