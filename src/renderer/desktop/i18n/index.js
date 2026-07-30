import { ref, computed, watch } from 'vue'
import { zh } from './zh.js'
import { en } from './en.js'

const messages = { zh, en }

function normalizeDesktopLocale(locale) {
  return locale?.toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

function getDefaultLocale() {
  try {
    const saved = localStorage.getItem('language')
    if (saved === 'zh' || saved === 'en') return saved
  } catch {
    // Fall back to the browser locale when storage is unavailable.
  }

  const browserLang = navigator.language || navigator.userLanguage || ''
  return normalizeDesktopLocale(browserLang)
}

const currentLocale = ref(getDefaultLocale())
let localeRevision = 0

function updateCurrentLocale(locale, persist = false) {
  const normalized = normalizeDesktopLocale(locale)
  localeRevision += 1
  if (normalized !== currentLocale.value) {
    currentLocale.value = normalized
  }
  if (persist) {
    try {
      localStorage.setItem('language', normalized)
    } catch {
      // Keep the in-memory locale when storage is unavailable.
    }
  }
}

function setDocumentLanguage(locale) {
  document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en'
}

watch(currentLocale, setDocumentLanguage, { immediate: true })

// 启动时恢复桌面 UI；托盘会在创建菜单前从同一个 locale 同步初始化。
let syncInitialized = false
async function syncLocaleFromSunshine() {
  if (syncInitialized) return
  syncInitialized = true
  const revision = localeRevision
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const config = await invoke('parse_sunshine_config')
    if (!config?.locale || revision !== localeRevision) return
    // Sunshine 用 'zh'/'zh_TW' 等，桌面 GUI 只有 'zh'/'en'
    const guiLocale = normalizeDesktopLocale(config.locale)
    updateCurrentLocale(guiLocale, true)
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
      updateCurrentLocale(event.payload, true)
    })
  } catch {
    // 非 Tauri 环境，忽略
  }
}
listenTrayLocaleChanged()

// localStorage is shared by the app webviews, but each webview owns its Vue
// state. Keep already-open toolbar and tool windows in sync with another
// window changing the language.
window.addEventListener('storage', (event) => {
  if (event.key !== 'language' || !event.newValue) return
  updateCurrentLocale(event.newValue)
})

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
      updateCurrentLocale(val, true)
    },
  })
  const toggleLocale = () => {
    const newLocale = locale.value === 'zh' ? 'en' : 'zh'
    locale.value = newLocale
    syncLocalePreferences(newLocale)
  }
  return { t, locale, toggleLocale }
}
