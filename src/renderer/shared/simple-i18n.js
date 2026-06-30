export function normalizeSimpleLocale(locale) {
  if (!locale || typeof locale !== 'string') return null
  return locale.toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

export function getStoredSimpleLocale() {
  try {
    return normalizeSimpleLocale(localStorage.getItem('language') || localStorage.getItem('sunshine-locale'))
  } catch {
    return null
  }
}

export function getSystemSimpleLocale() {
  const browserLocale = navigator.language || navigator.userLanguage || ''
  return normalizeSimpleLocale(browserLocale) || 'en'
}

export function getInitialSimpleLocale() {
  return getStoredSimpleLocale() || getSystemSimpleLocale()
}

export function setDocumentLocale(locale) {
  document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en'
}

export function pickSimpleMessages(messages, locale = getInitialSimpleLocale()) {
  return messages[normalizeSimpleLocale(locale) || 'en'] || messages.en
}

export async function syncSimpleLocaleFromSunshine(messages, apply) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const sunshineLocale = await invoke('get_sunshine_locale')
    const locale = normalizeSimpleLocale(sunshineLocale)
    if (locale && messages[locale]) {
      localStorage.setItem('language', locale)
      localStorage.setItem('sunshine-locale', locale)
      apply(locale)
    }
  } catch {
    // Browser preview or unavailable Sunshine API: keep saved/system locale.
  }
}
