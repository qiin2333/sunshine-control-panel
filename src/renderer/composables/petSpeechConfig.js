/**
 * 桌宠设置配置（独立于桌面主面板的 useDesktopPet 状态）
 *
 * 三层结构：
 *   1. 总开关：sunshine-pet-speech-enabled  控制桌宠是否说话/观察
 *   2. 随机对话：sunshine-pet-random-* （本地预设对话，不需要 AI）
 *   3. 桌面观察：sunshine-pet-enabled / sunshine-pet-interval （AI 视觉评论，复用 useDesktopPet 的键以与桌面 UI 同步）
 */

// 总开关
export const PET_MASTER_KEY = 'sunshine-pet-speech-enabled'

// 随机对话（本地）
export const PET_RANDOM_ENABLED_KEY = 'sunshine-pet-random-enabled'
export const PET_RANDOM_INTERVAL_KEY = 'sunshine-pet-speech-interval' // 复用旧键，避免历史用户配置丢失
export const PET_RANDOM_JITTER_KEY = 'sunshine-pet-speech-jitter'

// 桌面观察（视觉 AI）— 与 useDesktopPet.js 一致
export const PET_VISION_ENABLED_KEY = 'sunshine-pet-enabled'
export const PET_VISION_INTERVAL_KEY = 'sunshine-pet-interval' // 注意：useDesktopPet 中存储的是毫秒

// 范围与默认值
export const MIN_INTERVAL_SEC = 15
export const MAX_INTERVAL_SEC = 3600
export const DEFAULT_RANDOM_INTERVAL_SEC = 30
export const DEFAULT_VISION_INTERVAL_SEC = 60
export const DEFAULT_JITTER_PERCENT = 30
export const MAX_JITTER_PERCENT = 100

// === 加载器 ===
function readBool(key, fallback) {
  try {
    const v = localStorage.getItem(key)
    if (v === null) return fallback
    return v === 'true'
  } catch {
    return fallback
  }
}

function readInt(key, fallback, min, max) {
  try {
    const raw = localStorage.getItem(key)
    if (raw === null) return fallback
    const n = parseInt(raw, 10)
    if (!Number.isFinite(n)) return fallback
    return Math.min(max, Math.max(min, n))
  } catch {
    return fallback
  }
}

export function loadMasterEnabled() {
  return readBool(PET_MASTER_KEY, true)
}

export function loadRandomEnabled() {
  return readBool(PET_RANDOM_ENABLED_KEY, true)
}

export function loadRandomIntervalSec() {
  return readInt(PET_RANDOM_INTERVAL_KEY, DEFAULT_RANDOM_INTERVAL_SEC, MIN_INTERVAL_SEC, MAX_INTERVAL_SEC)
}

export function loadJitterPercent() {
  return readInt(PET_RANDOM_JITTER_KEY, DEFAULT_JITTER_PERCENT, 0, MAX_JITTER_PERCENT)
}

export function loadVisionEnabled() {
  return readBool(PET_VISION_ENABLED_KEY, false)
}

export function loadVisionIntervalSec() {
  // useDesktopPet 中以毫秒存储，向下兼容：> 1000 视为毫秒
  try {
    const raw = localStorage.getItem(PET_VISION_INTERVAL_KEY)
    if (raw === null) return DEFAULT_VISION_INTERVAL_SEC
    const n = parseInt(raw, 10)
    if (!Number.isFinite(n) || n <= 0) return DEFAULT_VISION_INTERVAL_SEC
    const sec = n > 1000 ? Math.round(n / 1000) : n
    return Math.min(MAX_INTERVAL_SEC, Math.max(MIN_INTERVAL_SEC, sec))
  } catch {
    return DEFAULT_VISION_INTERVAL_SEC
  }
}

// === 桌面观察历史回看 ===
export const PET_VISION_HISTORY_KEY = 'sunshine-pet-vision-history'
export const VISION_HISTORY_MAX = 5

export function loadVisionHistory() {
  try {
    const raw = localStorage.getItem(PET_VISION_HISTORY_KEY)
    if (!raw) return []
    const arr = JSON.parse(raw)
    if (!Array.isArray(arr)) return []
    return arr
      .filter((it) => it && typeof it.text === 'string' && it.text.trim())
      .slice(0, VISION_HISTORY_MAX)
  } catch {
    return []
  }
}

export function pushVisionHistory(text) {
  const trimmed = String(text || '').trim()
  if (!trimmed) return
  try {
    const arr = loadVisionHistory()
    arr.unshift({ text: trimmed, timestamp: Date.now() })
    const sliced = arr.slice(0, VISION_HISTORY_MAX)
    localStorage.setItem(PET_VISION_HISTORY_KEY, JSON.stringify(sliced))
  } catch {
    // ignore
  }
}

export function clearVisionHistory() {
  try {
    localStorage.removeItem(PET_VISION_HISTORY_KEY)
  } catch {
    // ignore
  }
}
