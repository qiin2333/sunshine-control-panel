/**
 * 桌宠视觉观察模块
 * 定时截取桌面截图，发送给多模态 LLM，让米塔根据用户桌面内容生成吐槽/调侃文本
 */

import { ref } from 'vue'
import { callVisionLLM, isApiKeyRequired } from './aiClient.js'
import { STORAGE_KEY, DEFAULT_CONFIG } from './aiProviders.js'
import { useI18n } from '../desktop/i18n/index.js'

// ===== 模块级共享状态（单例） =====
const petMessage = ref('')
const isObserving = ref(false)
const lastObserveTime = ref(0)
const petEnabled = ref(loadPetEnabled())
const observeInterval = ref(loadObserveInterval())
let timer = null
let initialized = false

function loadPetEnabled() {
  try {
    return localStorage.getItem('sunshine-pet-enabled') === 'true'
  } catch {
    return false
  }
}

function loadObserveInterval() {
  try {
    const saved = localStorage.getItem('sunshine-pet-interval')
    return saved ? parseInt(saved, 10) : 60000
  } catch {
    return 60000
  }
}

function savePetConfig() {
  localStorage.setItem('sunshine-pet-enabled', String(petEnabled.value))
  localStorage.setItem('sunshine-pet-interval', String(observeInterval.value))
}

async function captureScreen() {
  const tauri = window.__TAURI__
  if (!tauri?.core?.invoke) {
    throw new Error('需要在 Tauri 环境中运行')
  }
  return await tauri.core.invoke('capture_screenshot')
}

function getAiConfig() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    return saved ? { ...DEFAULT_CONFIG, ...JSON.parse(saved) } : { ...DEFAULT_CONFIG }
  } catch {
    return { ...DEFAULT_CONFIG }
  }
}

function getRuntimeText() {
  const { t } = useI18n()
  return t.value.petTool.runtime
}

async function observe() {
  const config = getAiConfig()
  const runtime = getRuntimeText()
  if (!config.enabled || (!config.apiKey && isApiKeyRequired(config))) {
    console.warn('[桌宠] AI 未启用或未配置 API Key，跳过观察')
    petMessage.value = runtime.visionNotConfigured
    return
  }

  isObserving.value = true
  try {
    console.log('[桌宠] 开始截屏...')
    const screenshot = await captureScreen()
    console.log('[桌宠] 截屏完成，调用 Vision LLM...')
    const response = await callVisionLLM(
      config,
      runtime.visionPrompt,
      runtime.visionUserMsg,
      screenshot,
      150
    )

    if (response && response.trim()) {
      console.log('[桌宠] LLM 回复:', response.trim())
      petMessage.value = response.trim()
      lastObserveTime.value = Date.now()
    } else {
      petMessage.value = runtime.visionEmpty
    }
  } catch (err) {
    const errMsg = typeof err === 'string' ? err : (err?.message || JSON.stringify(err))
    console.warn('[桌宠] 观察失败:', errMsg, err)
    petMessage.value = `${runtime.visionErrorPrefix}${errMsg || runtime.visionUnknownError}`
  } finally {
    isObserving.value = false
  }
}

function startObserving() {
  if (timer) clearInterval(timer)
  petEnabled.value = true
  savePetConfig()

  observe()

  timer = setInterval(() => {
    if (!isObserving.value) {
      observe()
    }
  }, observeInterval.value)
}

function stopObserving() {
  petEnabled.value = false
  savePetConfig()
  if (timer) {
    clearInterval(timer)
    timer = null
  }
}

function setIntervalSeconds(seconds) {
  observeInterval.value = Math.max(15, seconds) * 1000
  savePetConfig()
  if (petEnabled.value) {
    startObserving()
  }
}

async function poke() {
  await observe()
}

function dismissMessage() {
  petMessage.value = ''
}

/**
 * 桌宠视觉观察 Composable（共享单例状态）
 */
export function useDesktopPet() {
  // 首次调用时自动恢复之前的启用状态
  if (!initialized) {
    initialized = true
    if (petEnabled.value) {
      startObserving()
    }
  }

  return {
    petMessage,
    petEnabled,
    isObserving,
    lastObserveTime,
    observeInterval,
    startObserving,
    stopObserving,
    setIntervalSeconds,
    poke,
    dismissMessage,
  }
}
