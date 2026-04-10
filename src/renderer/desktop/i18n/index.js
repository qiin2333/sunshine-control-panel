import { ref, computed } from 'vue'
import { zh } from './zh.js'
import { en } from './en.js'

const messages = { zh, en }

const currentLocale = ref(localStorage.getItem('language') || 'zh')

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
    locale.value = locale.value === 'zh' ? 'en' : 'zh'
  }
  return { t, locale, toggleLocale }
}
