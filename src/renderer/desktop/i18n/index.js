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
    const { sunshine } = await import('../../tauri-adapter.js')
    const sunshineLocale = await sunshine.getLocale()
    // Sunshine 用 'zh'/'zh_TW' 等，桌面 GUI 只有 'zh'/'en'
    const guiLocale = sunshineLocale.startsWith('zh') ? 'zh' : 'en'
    if (guiLocale !== currentLocale.value) {
      currentLocale.value = guiLocale
      localStorage.setItem('language', guiLocale)
    }
  } catch {
    // 非 Tauri 环境或 API 不可用，忽略
  }
}
syncLocaleFromSunshine()

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
