import { createI18n } from 'vue-i18n'

// 导入所有语言文件
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

// 支持的语言列表
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

// 创建 i18n 实例
export const i18n = createI18n({
  legacy: false, // 使用 Composition API 模式
  locale: 'zh', // 默认语言
  fallbackLocale: 'en', // 回退语言
  messages: {
    en,
    en_GB,
    en_US,
    zh,
    zh_TW,
    de,
    fr,
    es,
    it,
    ja,
    ko,
    ru,
    uk,
    pt,
    pt_BR,
    pl,
    sv,
    tr,
    cs,
    bg,
  },
})

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
  const locale = supportedLocales.find((l) => l.code === langCode || l.code === browserLang)
  if (locale) {
    return locale.code
  }

  // 默认返回中文
  return 'zh'
}

// 设置语言
export function setLocale(locale) {
  if (supportedLocales.find((l) => l.code === locale)) {
    i18n.global.locale.value = locale
    localStorage.setItem('sunshine-locale', locale)
    return true
  }
  return false
}

export default i18n

