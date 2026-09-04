import { computed } from 'vue'
import { useI18n } from '../desktop/i18n/index.js'
import { rtxHdrMessages } from './rtxHdrMessages.js'

export function useRtxHdrI18n() {
  const { locale } = useI18n()
  return computed(() => locale.value === 'zh' ? rtxHdrMessages.zh : rtxHdrMessages.en)
}
