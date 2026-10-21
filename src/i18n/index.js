import { ref, watch } from 'vue'
import en from './locales/en.js'
import en_GB from './locales/en_GB.js'
import en_US from './locales/en_US.js'
import zh from './locales/zh.js'
import zh_TW from './locales/zh_TW.js'
import de from './locales/de.js'
import fr from './locales/fr.js'
import es from './locales/es.js'
import it from './locales/it.js'
import ja from './locales/ja.js'
import ko from './locales/ko.js'
import ru from './locales/ru.js'
import uk from './locales/uk.js'
import pt from './locales/pt.js'
import pt_BR from './locales/pt_BR.js'
import pl from './locales/pl.js'
import sv from './locales/sv.js'
import tr from './locales/tr.js'
import cs from './locales/cs.js'
import bg from './locales/bg.js'

// 方案乙（去 vue-i18n 依赖、保留 20 语言数据）：welcome 页只用到
// t(key) 平铺查表 + 语言切换，这里保持原对外行为（en 兜底、ref 响应式）。
export const supportedLocales = [
  { code: 'en', name: 'English' },
  { code: 'en_GB', name: 'English (UK)' },
  { code: 'en_US', name: 'English (US)' },
  { code: 'zh', name: '简体中文' },
  { code: 'zh_TW', name: '繁體中文' },
  { code: 'de', name: 'Deutsch' },
  { code: 'fr', name: 'Français' },
  { code: 'es', name: 'Español' },
  { code: 'it', name: 'Italiano' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
  { code: 'ru', name: 'Русский' },
  { code: 'uk', name: 'Українська' },
  { code: 'pt', name: 'Português' },
  { code: 'pt_BR', name: 'Português (Brasil)' },
  { code: 'pl', name: 'Polski' },
  { code: 'sv', name: 'Svenska' },
  { code: 'tr', name: 'Türkçe' },
  { code: 'cs', name: 'Čeština' },
  { code: 'bg', name: 'Български' },
]

const messages = {
  en, en_GB, en_US, zh, zh_TW, de, fr, es, it, ja,
  ko, ru, uk, pt, pt_BR, pl, sv, tr, cs, bg,
}

const locale = ref(getDefaultLocale())

function setDocumentLanguage(value) {
  if (typeof document === 'undefined') return
  // 语言码即 BCP 47 标签（en_GB → en-GB、zh_TW → zh-TW）；zh 特判为
  // zh-CN 保留屏幕阅读器的变体提示，未知值兜底 en。
  const tag = String(value || 'en').replace(/_/g, '-')
  document.documentElement.lang = tag === 'zh' ? 'zh-CN' : tag
}
watch(locale, setDocumentLanguage, { immediate: true })

// 获取默认语言
export function getDefaultLocale() {
  // 尝试从 localStorage 读取
  const saved = localStorage.getItem('sunshine-locale')
  if (saved && supportedLocales.find((l) => l.code === saved)) {
    return saved
  }

  // 尝试从浏览器语言获取
  const browserLang = navigator.language || navigator.userLanguage
  const langCode = browserLang.split('-')[0]
  const matched = supportedLocales.find((l) => l.code === langCode || l.code === browserLang)
  if (matched) {
    return matched.code
  }

  // 默认返回英文，避免英语系统用户首次打开时看到中文界面
  return 'en'
}

// 设置语言
export function setLocale(value) {
  if (!supportedLocales.find((l) => l.code === value)) return false
  locale.value = value
  localStorage.setItem('sunshine-locale', value)
  return true
}

// 平铺查表：当前语言缺失的键回退到 en，再缺失则原样返回键名。
export function t(key) {
  const table = messages[locale.value] || {}
  return table[key] ?? messages.en[key] ?? key
}

export function useWelcomeI18n() {
  return { t, locale }
}
