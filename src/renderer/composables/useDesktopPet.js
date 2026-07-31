import { ref } from 'vue'
import {
  PET_VISION_ENABLED_KEY,
  PET_VISION_INTERVAL_KEY,
  loadVisionEnabled,
  loadVisionIntervalSec,
  notifyPetConfigChanged,
  requestPetVision,
  MIN_INTERVAL_SEC,
  MAX_INTERVAL_SEC,
} from './petSpeechConfig.js'

const isObserving = ref(false)
const petEnabled = ref(loadVisionEnabled())
const observeInterval = ref(loadVisionIntervalSec() * 1000)
const pokeFailed = ref(false)

function persistSetting(key, value) {
  try {
    localStorage.setItem(key, value)
  } catch (error) {
    console.warn('[桌宠] 配置持久化失败:', error)
  }
}

function startObserving() {
  petEnabled.value = true
  persistSetting(PET_VISION_ENABLED_KEY, 'true')
  notifyPetConfigChanged()
}

function stopObserving() {
  petEnabled.value = false
  persistSetting(PET_VISION_ENABLED_KEY, 'false')
  notifyPetConfigChanged()
}

function setIntervalSeconds(seconds) {
  const parsed = Math.round(Number(seconds))
  const clamped = Number.isFinite(parsed)
    ? Math.min(MAX_INTERVAL_SEC, Math.max(MIN_INTERVAL_SEC, parsed))
    : MIN_INTERVAL_SEC
  observeInterval.value = clamped * 1000
  persistSetting(PET_VISION_INTERVAL_KEY, String(observeInterval.value))
  notifyPetConfigChanged()
}

async function poke() {
  if (isObserving.value) return
  isObserving.value = true
  pokeFailed.value = false
  try {
    const result = await requestPetVision({ ensureToolbar: true })
    pokeFailed.value = !result?.success
  } catch (error) {
    console.warn('[桌宠] 立即观察失败:', error)
    pokeFailed.value = true
  } finally {
    isObserving.value = false
  }
}

let storageSyncInitialized = false
function initStorageSync() {
  if (storageSyncInitialized) return
  storageSyncInitialized = true
  window.addEventListener('storage', (event) => {
    if (event.key === PET_VISION_ENABLED_KEY) {
      petEnabled.value = loadVisionEnabled()
    } else if (event.key === PET_VISION_INTERVAL_KEY) {
      observeInterval.value = loadVisionIntervalSec() * 1000
    }
  })
}

export function useDesktopPet() {
  initStorageSync()
  return {
    petEnabled,
    isObserving,
    observeInterval,
    pokeFailed,
    startObserving,
    stopObserving,
    setIntervalSeconds,
    poke,
  }
}
