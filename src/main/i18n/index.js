import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'node:url'
import { app } from 'electron'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

// 支持的语言列表
export const SUPPORTED_LANGUAGES = [
  { code: 'zh-CN', name: '简体中文', nativeName: '简体中文' },
  { code: 'en-US', name: 'English', nativeName: 'English' },
  { code: 'ja-JP', name: '日本語', nativeName: '日本語' },
  { code: 'ko-KR', name: '한국어', nativeName: '한국어' },
  { code: 'fr-FR', name: 'Français', nativeName: 'Français' },
  { code: 'de-DE', name: 'Deutsch', nativeName: 'Deutsch' }
]

// 默认语言
const DEFAULT_LANGUAGE = 'zh-CN'

// 语言包缓存
const translations = new Map()

/**
 * 获取系统语言
 * @returns {string} 语言代码
 */
export function getSystemLanguage() {
  const locale = app.getLocale()
  
  // 映射系统语言到支持的语言
  const languageMap = {
    'zh': 'zh-CN',
    'zh-CN': 'zh-CN',
    'zh-TW': 'zh-CN', // 繁体中文映射到简体中文
    'en': 'en-US',
    'en-US': 'en-US',
    'en-GB': 'en-US',
    'ja': 'ja-JP',
    'ja-JP': 'ja-JP',
    'ko': 'ko-KR',
    'ko-KR': 'ko-KR',
    'fr': 'fr-FR',
    'fr-FR': 'fr-FR',
    'de': 'de-DE',
    'de-DE': 'de-DE'
  }
  
  return languageMap[locale] || DEFAULT_LANGUAGE
}

// 简单的本地存储实现
let languageStorage = null

/**
 * 获取当前语言设置
 * @returns {string} 当前语言代码
 */
export function getCurrentLanguage() {
  if (!languageStorage) {
    try {
      const userDataPath = app.getPath('userData')
      const configPath = join(userDataPath, 'language.json')
      
      if (existsSync(configPath)) {
        const data = readFileSync(configPath, 'utf8')
        languageStorage = JSON.parse(data)
      } else {
        languageStorage = { language: getSystemLanguage() }
      }
    } catch (error) {
      console.warn('Failed to load language config:', error)
      languageStorage = { language: getSystemLanguage() }
    }
  }
  
  return languageStorage.language || getSystemLanguage()
}

/**
 * 设置语言
 * @param {string} languageCode 语言代码
 */
export function setLanguage(languageCode) {
  try {
    const userDataPath = app.getPath('userData')
    const configPath = join(userDataPath, 'language.json')
    
    languageStorage = { language: languageCode }
    writeFileSync(configPath, JSON.stringify(languageStorage, null, 2))
  } catch (error) {
    console.warn('Failed to save language config:', error)
  }
}

/**
 * 加载语言包
 * @param {string} languageCode 语言代码
 * @returns {Object} 语言包对象
 */
export function loadTranslations(languageCode) {
  if (translations.has(languageCode)) {
    return translations.get(languageCode)
  }
  
  try {
    const localePath = join(__dirname, 'locales', `${languageCode}.json`)
    const translationData = JSON.parse(readFileSync(localePath, 'utf8'))
    translations.set(languageCode, translationData)
    return translationData
  } catch (error) {
    console.warn(`Failed to load translations for ${languageCode}:`, error)
    // 回退到默认语言
    if (languageCode !== DEFAULT_LANGUAGE) {
      return loadTranslations(DEFAULT_LANGUAGE)
    }
    return {}
  }
}

/**
 * 翻译函数
 * @param {string} key 翻译键，支持点号分隔的嵌套键
 * @param {Object} params 参数对象，用于替换占位符
 * @param {string} languageCode 语言代码，可选
 * @returns {string} 翻译后的文本
 */
export function t(key, params = {}, languageCode = null) {
  const lang = languageCode || getCurrentLanguage()
  const translation = loadTranslations(lang)
  
  // 支持嵌套键，如 'menu.window'
  const keys = key.split('.')
  let value = translation
  
  for (const k of keys) {
    if (value && typeof value === 'object' && k in value) {
      value = value[k]
    } else {
      // 如果找不到翻译，回退到默认语言
      if (lang !== DEFAULT_LANGUAGE) {
        return t(key, params, DEFAULT_LANGUAGE)
      }
      return key // 最后回退到键名本身
    }
  }
  
  if (typeof value !== 'string') {
    return key
  }
  
  // 替换参数占位符
  return value.replace(/\{(\w+)\}/g, (match, paramKey) => {
    return params[paramKey] !== undefined ? params[paramKey] : match
  })
}

/**
 * 获取所有支持的语言
 * @returns {Array} 支持的语言列表
 */
export function getSupportedLanguages() {
  return SUPPORTED_LANGUAGES
}

/**
 * 检查语言是否支持
 * @param {string} languageCode 语言代码
 * @returns {boolean} 是否支持
 */
export function isLanguageSupported(languageCode) {
  return SUPPORTED_LANGUAGES.some(lang => lang.code === languageCode)
}

/**
 * 初始化国际化
 */
export function initI18n() {
  const currentLang = getCurrentLanguage()
  console.log(`Initializing i18n with language: ${currentLang}`)
  
  // 预加载当前语言包
  loadTranslations(currentLang)
}
