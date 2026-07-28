<template>
  <div id="toolbar-container" :class="{ 'menu-open': menuVisible }"
       @click.self="handleOutsideClick"
       @pointerdown.self="onContainerDragStart"
       @contextmenu.prevent>
    <!-- 气泡菜单 -->
    <transition name="bubble">
      <div v-if="menuVisible" class="bubble-menu" @click.stop>
        <div v-for="(item, index) in menuItems" :key="item.id" class="bubble-wrapper" :style="getBubbleStyle(index)">
          <div
            class="bubble-item"
            :class="{ danger: item.danger }"
            :style="{ animationDelay: `${index * 100}ms` }"
            @click="handleMenuItem(item.id)"
            :title="item.label"
          >
            <div class="bubble-icon" v-html="item.icon"></div>
          </div>
        </div>
      </div>
    </transition>

    <!-- 话术气泡 -->
    <div v-if="speechVisible" class="speech-bubble" role="status" aria-live="polite">
      {{ speechText }}
    </div>

    <!-- 中心工具栏图标 -->
    <div
      class="toolbar-icon"
      :class="{ active: menuVisible }"
      @pointerdown="onDragStart"
      @click.stop="onIconClick"
      @contextmenu.prevent="toggleMenu"
    >
      <!-- PixiJS Canvas 容器 -->
      <canvas ref="pixiCanvas" class="icon-canvas"></canvas>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onUnmounted, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { cursorPosition } from '@tauri-apps/api/window'
import { PhysicalPosition } from '@tauri-apps/api/dpi'
import { useI18n } from '../desktop/i18n/index.js'
import { callVisionLLM, isApiKeyRequired } from '../composables/aiClient.js'
import { STORAGE_KEY, DEFAULT_CONFIG } from '../composables/aiProviders.js'
import {
  loadMasterEnabled,
  loadRandomEnabled,
  loadRandomIntervalSec,
  loadJitterPercent,
  loadVisionEnabled,
  loadVisionIntervalSec,
  pushVisionHistory,
  MIN_INTERVAL_SEC,
} from '../composables/petSpeechConfig.js'

const { t, locale, toggleLocale } = useI18n()

const menuVisible = ref(false)
const speechVisible = ref(false)
const speechSource = ref('random')
const speechText = ref('')
let speechTimer = null
let speechInterval = null

// PixiJS 相关
const pixiCanvas = ref(null)
let pixiApp = null
let spriteFrames = []
let currentSprite = null
let animationTimer = null
let pixiModulePromise = null

const loadPixi = async () => {
  if (!pixiModulePromise) {
    pixiModulePromise = import('pixi.js')
  }
  return pixiModulePromise
}

// 精灵图集 URL
const SPRITESHEET_URL =
  'https://assets.alkaidlab.com/toolbar-spritesheet.webp'

// IndexedDB 缓存配置
const DB_NAME = 'toolbar-cache'
const DB_VERSION = 2
const DB_STORE = 'resources'
const CACHE_KEY_SPRITE = 'spritesheet'
const CACHE_KEY_PHRASES = 'phrases'

// 打开 IndexedDB
const openDB = () => {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION)
    request.onerror = () => reject(request.error)
    request.onsuccess = () => resolve(request.result)
    request.onupgradeneeded = (event) => {
      const db = event.target.result
      // v1→v2: 删除旧 store 'images'，创建新 'resources' store（含 ETag）
      if (db.objectStoreNames.contains('images')) {
        db.deleteObjectStore('images')
      }
      if (!db.objectStoreNames.contains(DB_STORE)) {
        db.createObjectStore(DB_STORE)
      }
    }
  })
}

// 缓存读取（返回 { data, etag } 或 null）
const getCacheEntry = async (key) => {
  try {
    const db = await openDB()
    const tx = db.transaction([DB_STORE], 'readonly')
    const store = tx.objectStore(DB_STORE)
    return new Promise((resolve, reject) => {
      const request = store.get(key)
      request.onsuccess = () => {
        const entry = request.result
        if (!entry || !entry.data) return resolve(null)
        resolve(entry)
      }
      request.onerror = () => reject(request.error)
    })
  } catch (error) {
    console.warn('⚠️  IndexedDB 读取失败:', error)
    return null
  }
}

// 缓存写入（data + etag）
const setCacheEntry = async (key, data, etag) => {
  try {
    const db = await openDB()
    const tx = db.transaction([DB_STORE], 'readwrite')
    const store = tx.objectStore(DB_STORE)
    await new Promise((resolve, reject) => {
      const request = store.put({ data, etag: etag || null }, key)
      request.onsuccess = () => resolve()
      request.onerror = () => reject(request.error)
    })
  } catch (error) {
    console.warn('⚠️  IndexedDB 写入失败:', error)
  }
}

// 默认话术（fallback）：从 i18n 读取，跟随 locale 切换
const defaultPhrases = computed(() => t.value.petTool?.runtime?.defaultPhrases || [])

// 响应式话术列表（后端拉到的话术优先；未拉到时回退到 i18n fallback）
const speechPhrases = ref([])

// 加载话术（ETag 条件请求：有缓存则先用缓存，同时带 ETag 请求确认是否需要更新）
const loadSpeechPhrases = async () => {
  try {
    const cached = await getCacheEntry(CACHE_KEY_PHRASES)
    // 先应用缓存数据（如果有）
    if (cached && Array.isArray(cached.data) && cached.data.length > 0) {
      speechPhrases.value = cached.data
    }
    // 带 ETag 条件请求后端
    const result = await invoke('fetch_speech_phrases', {
      ifNoneMatch: cached?.etag || null,
    })
    if (result) {
      // 有新数据（200）
      speechPhrases.value = result.phrases
      await setCacheEntry(CACHE_KEY_PHRASES, result.phrases, result.etag)
    }
    // result 为 null 表示 304 未修改，不需要更新
  } catch (error) {
    console.warn('⚠️  话术加载失败，使用默认话术:', error)
  }
}

const showSpeech = () => {
  if (speechVisible.value) return
  // 后端对话为空时回落到 i18n 内置话术
  const phrases = speechPhrases.value.length > 0 ? speechPhrases.value : defaultPhrases.value
  if (phrases.length === 0) return
  const text = phrases[Math.floor(Math.random() * phrases.length)]
  speechText.value = text
  speechVisible.value = true
  if (speechTimer) {
    clearTimeout(speechTimer)
    speechTimer = null
  }
  speechTimer = setTimeout(() => {
    speechVisible.value = false
  }, 2600)
}

// 直接显示一段固定文本（用于反馈失败/状态提示）
// 与 showSpeech 共用同一气泡组件与入场动画
const showSpeechRaw = (text, durationMs) => {
  if (!text) return
  // 先关再开，确保 v-if 重新挂载、走 bubble-enter 过渡
  speechVisible.value = false
  if (speechTimer) {
    clearTimeout(speechTimer)
    speechTimer = null
  }
  // 文本中可能含换行/缩进，统一压成单行避免气泡顶部高度异常
  const normalized = String(text).replace(/\s+/g, ' ').trim()
  const auto = Math.min(12000, 4000 + Math.floor(normalized.length / 8) * 1000)
  const ms = typeof durationMs === 'number' ? durationMs : auto

  // 下一帧再设为 true，让 Vue 触发离开→进入过渡
  requestAnimationFrame(() => {
    speechSource.value = 'raw'
    speechText.value = normalized
    speechVisible.value = true
    speechTimer = setTimeout(() => {
      speechVisible.value = false
    }, ms)
  })
}

// ===== Vision 桌面观察 =====

// 运行时文案统一从 i18n 读取（zh.js / en.js 中 petTool.runtime.*）
const rt = () => t.value.petTool?.runtime || {}

const getAiConfig = () => {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    return saved ? { ...DEFAULT_CONFIG, ...JSON.parse(saved) } : { ...DEFAULT_CONFIG }
  } catch {
    return { ...DEFAULT_CONFIG }
  }
}

const isPetVisionEnabled = () => {
  // 总开关 + 桌面观察开关 双条件
  return loadMasterEnabled() && loadVisionEnabled()
}

// 从各种原始错误中提取人可读的 message
const extractErrorMessage = (err) => {
  if (!err) return rt().visionUnknownError || ''
  let raw = typeof err === 'string' ? err : err?.message || String(err)
  const jsonMatch = raw.match(/\{[\s\S]*\}/)
  if (jsonMatch) {
    try {
      const parsed = JSON.parse(jsonMatch[0])
      const msg =
        parsed?.error?.message ||
        parsed?.message ||
        parsed?.data?.message ||
        parsed?.error
      if (typeof msg === 'string' && msg.trim()) return msg.trim()
    } catch {
      // 解析失败则回退到原始文本
    }
  }
  return raw
}

// 错误特征 → i18n key 映射；命中即返回对应短提示
const VISION_ERROR_PATTERNS = [
  [/not a vlm|vision language model|text-only|no vision|不支持视觉|不支持图像/, 'notVlm'],
  [/401|unauthor|invalid api key/, 'unauthorized'],
  [/403|forbidden|permission/, 'forbidden'],
  [/404|not found|no such model/, 'notFound'],
  [/429|rate limit|too many|quota/, 'rateLimit'],
  [/timeout|timed out|abort/, 'timeout'],
  [/network|fetch failed|econn|enot|failed to fetch/, 'network'],
]

const friendlyVisionError = (msg) => {
  const m = (msg || '').toLowerCase()
  const errors = rt().errors || {}
  for (const [re, key] of VISION_ERROR_PATTERNS) {
    if (re.test(m)) return errors[key] || null
  }
  return null
}

// 剥离 LLM 特殊 token / 思维链 / Markdown 代码块包裹
// 例如 GLM 系列会输出 <|begin_of_box|>...<|end_of_box|>，思维模型会输出 <think>...</think>
const sanitizeLlmReply = (raw) => {
  if (!raw) return ''
  let s = String(raw)
  // 思维链：DeepSeek-R1 / Claude thinking / Qwen
  s = s.replace(/<think>[\s\S]*?<\/think>/gi, '')
  s = s.replace(/<reasoning>[\s\S]*?<\/reasoning>/gi, '')
  // 包裹型特殊 token：<|...|>...<|...|>，先尝试提取 begin_of_box / answer 内的内容
  const boxed = s.match(/<\|begin_of_box\|>([\s\S]*?)<\|end_of_box\|>/i)
  if (boxed) s = boxed[1]
  const answered = s.match(/<\|begin_of_answer\|>([\s\S]*?)<\|end_of_answer\|>/i)
  if (answered) s = answered[1]
  // 去除残余的 <|...|> 标记
  s = s.replace(/<\|[^|]*\|>/g, '')
  // 去除 Markdown 代码块围栏（少数模型会包代码块）
  s = s.replace(/^```[a-z]*\s*|```\s*$/gim, '')
  // 去除前后引号
  s = s.trim().replace(/^["'「『]+|["'」』]+$/g, '')
  return s.trim()
}

// 防止用户连点 / 多 trigger 同时跑
let visionInflight = false
// 戳一下成功/失败后的冷静期，避免连按
const VISION_COOLDOWN_MS = 3000
let visionCooldownUntil = 0
// AI 请求 120s 客户端超时（即使后端继续处理，前端也释放锁）
const VISION_REQUEST_TIMEOUT_MS = 120000

// Promise.race 实现的客户端超时
const withTimeout = (promise, ms, timeoutErr) => {
  let timer
  const t = new Promise((_, reject) => {
    timer = setTimeout(() => reject(timeoutErr || new Error('timeout')), ms)
  })
  return Promise.race([promise, t]).finally(() => clearTimeout(timer))
}

const tryVisionSpeech = async (isManual = false) => {
  const config = getAiConfig()
  const r = rt()
  if (!config.enabled || (!config.apiKey && isApiKeyRequired(config)) || !isPetVisionEnabled()) {
    if (isManual) showSpeechRaw(r.visionNotConfigured || '')
    return false
  }

  // 冷静期：仅对 manual 触发提示，auto 静默跳过
  const now = Date.now()
  if (now < visionCooldownUntil) {
    if (isManual) {
      console.info('[桌宠Vision] 冷静期内，跳过', { remainMs: visionCooldownUntil - now })
      showSpeechRaw(r.visionCooldown || '')
    }
    return false
  }

  if (visionInflight) {
    if (isManual) {
      console.info('[桌宠Vision] 已有请求进行中，跳过本次触发')
      showSpeechRaw(r.visionBusy || r.visionLoading || '')
    }
    return false
  }
  visionInflight = true

  // 手动触发：立即抢占当前气泡，显示加载提示，避免用户以为没生效
  if (isManual && r.visionLoading) {
    showSpeechRaw(r.visionLoading, 30000) // 30s 占位，下面成功/失败会覆盖
  }

  const trigger = isManual ? 'manual' : 'auto'
  const t0 = performance.now()
  console.info(`[桌宠Vision] 开始捕获屏幕 trigger=${trigger} model=${config.model || '(default)'}`)

  try {
    const screenshot = await invoke('capture_screenshot')
    const shotMs = Math.round(performance.now() - t0)
    const shotLen = typeof screenshot === 'string' ? screenshot.length : (screenshot?.byteLength || 0)
    console.info(`[桌宠Vision] 截图完成 ${shotMs}ms size=${shotLen}`)

    const t1 = performance.now()
    const timeoutErr = new Error('timeout')
    timeoutErr.isTimeout = true
    const response = await withTimeout(
      callVisionLLM(config, r.visionPrompt, r.visionUserMsg, screenshot, 150),
      VISION_REQUEST_TIMEOUT_MS,
      timeoutErr
    )
    const llmMs = Math.round(performance.now() - t1)
    console.info(`[桌宠Vision] AI 响应 ${llmMs}ms reply=${JSON.stringify(response)}`)

    const cleaned = sanitizeLlmReply(response)
    if (cleaned) {
      speechSource.value = 'vision'
      speechText.value = cleaned
      speechVisible.value = true
      if (speechTimer) clearTimeout(speechTimer)
      // 桌面观察的内容信息量较大，按文本长度自适应：基础 7s，每 8 字 +1s，最长 15s
      const showMs = Math.min(15000, 7000 + Math.floor(cleaned.length / 8) * 1000)
      speechTimer = setTimeout(() => { speechVisible.value = false }, showMs)
      // 写入历史（仅成功评论，不记录错误提示）
      pushVisionHistory(cleaned)
      return true
    }
    console.warn('[桌宠Vision] AI 返回空内容（清洗后）')
    if (isManual) showSpeechRaw(r.visionEmpty || '')
  } catch (err) {
    if (err?.isTimeout) {
      console.warn('[桌宠Vision] 请求超时', VISION_REQUEST_TIMEOUT_MS, 'ms')
      if (isManual) showSpeechRaw(r.visionTimeout || '')
    } else {
      const raw = extractErrorMessage(err)
      console.warn('[桌宠Vision] 失败:', raw)
      if (isManual) {
        const friendly = friendlyVisionError(raw)
        const short = friendly || (raw.length > 35 ? raw.slice(0, 35) + '…' : raw)
        showSpeechRaw(`${r.visionErrorPrefix || ''}${short}`)
      }
    }
  } finally {
    visionInflight = false
    if (isManual) visionCooldownUntil = Date.now() + VISION_COOLDOWN_MS
  }
  return false
}

// 调度状态：模块级，便于 config-changed 时重排
let nextRandomAt = 0
let nextVisionAt = 0

// 配置变化时根据"已等待时长"与新间隔取较小者，避免一改就等满整段
const rescheduleSpeech = () => {
  const now = Date.now()
  const randomIntervalMs = Math.max(MIN_INTERVAL_SEC, loadRandomIntervalSec()) * 1000
  const visionIntervalMs = Math.max(MIN_INTERVAL_SEC, loadVisionIntervalSec()) * 1000
  if (nextRandomAt > now + randomIntervalMs) nextRandomAt = now + randomIntervalMs
  if (nextVisionAt > now + visionIntervalMs) nextVisionAt = now + visionIntervalMs
  console.info('[桌宠] 配置变化，重新调度', {
    randomInMs: nextRandomAt - now,
    visionInMs: nextVisionAt - now,
  })

  // 当前可见气泡若与"被关掉的功能"对应，立即收起
  if (!speechVisible.value) return
  const masterOn = loadMasterEnabled()
  if (!masterOn) {
    // 总开关关掉 → 任何来源的气泡都收起
    speechVisible.value = false
    if (speechTimer) { clearTimeout(speechTimer); speechTimer = null }
    return
  }
  if (speechSource.value === 'vision' && !loadVisionEnabled()) {
    speechVisible.value = false
    if (speechTimer) { clearTimeout(speechTimer); speechTimer = null }
    return
  }
  if (speechSource.value === 'random' && !loadRandomEnabled()) {
    speechVisible.value = false
    if (speechTimer) { clearTimeout(speechTimer); speechTimer = null }
  }
}

const startSpeechLoop = () => {
  // 首次延迟随机出现（随机对话）
  const firstDelay = 4000 + Math.random() * 6000
  setTimeout(() => {
    if (loadMasterEnabled() && loadRandomEnabled()) showSpeech()
  }, firstDelay)

  // 随机对话与桌面观察使用两个独立的下一次调度点，
  // 在一个定时器中轮询，避免互相干扰。
  const TICK_MS = 1000
  nextRandomAt = Date.now() + firstDelay
  nextVisionAt = Date.now() + firstDelay + 8000 // 视觉首次略后，避免启动后立即被截屏

  const computeNextRandom = () => {
    const intervalSec = Math.max(MIN_INTERVAL_SEC, loadRandomIntervalSec())
    const jitterPct = Math.max(0, loadJitterPercent()) / 100
    const jitter = (Math.random() - 0.5) * 2 * intervalSec * jitterPct
    return Date.now() + Math.max(MIN_INTERVAL_SEC, intervalSec + jitter) * 1000
  }

  const computeNextVision = () => {
    const intervalSec = Math.max(MIN_INTERVAL_SEC, loadVisionIntervalSec())
    return Date.now() + intervalSec * 1000
  }

  speechInterval = setInterval(async () => {
    if (!loadMasterEnabled()) return
    if (menuVisible.value) return
    const now = Date.now()

    // 桌面观察（优先调度，如果到点则尝试截屏）
    if (loadVisionEnabled() && now >= nextVisionAt) {
      nextVisionAt = computeNextVision()
      const used = await tryVisionSpeech()
      if (used) {
        // 视觉说过话了，同时把随机对话下次点往后推一点避免接踵
        if (loadRandomEnabled()) {
          nextRandomAt = Math.max(nextRandomAt, Date.now() + Math.max(5, MIN_INTERVAL_SEC / 2) * 1000)
        }
        return
      }
    }

    // 随机对话
    if (loadRandomEnabled() && now >= nextRandomAt) {
      nextRandomAt = computeNextRandom()
      showSpeech()
    }
  }, TICK_MS)
}

// 设置弹窗跨窗口通信
let speechBroadcast = null
const initSpeechBroadcast = () => {
  try {
    speechBroadcast = new BroadcastChannel('sunshine-pet-speech')
    speechBroadcast.addEventListener('message', async (e) => {
      const type = e?.data?.type
      if (type === 'config-changed') {
        // 设置面板修改了间隔等：根据新值收紧已有调度
        rescheduleSpeech()
      } else if (type === 'poke-vision') {
        // 强制触发桌面观察（设置面板的"立即观察"按钮）
        await tryVisionSpeech(true)
      } else if (type === 'poke') {
        // 兼容旧消息：优先视觉，失败则随机
        if (!(await tryVisionSpeech())) showSpeech()
      }
    })
  } catch (_) {}
}

const menuItems = computed(() => [
  {
    id: 'main',
    label: t.value.toolbar.controlPanel,
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/></svg>',
  },
  {
    id: 'vdd',
    label: t.value.toolbar.virtualDisplay,
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z"/></svg>',
  },
  {
    id: 'dpi',
    label: t.value.toolbar.adjustDpi,
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M21 3H3c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H3V5h18v14zM5 7h5v5H5zm6 0h8v2h-8zm0 3h8v2h-8zM5 13h5v5H5zm6 0h8v2h-8z"/></svg>',
  },
  {
    id: 'bitrate',
    label: t.value.toolbar.bitrateAdjust,
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M9 3L5 6.99h3V14h2V6.99h3L9 3zm7 14.01V10h-2v7.01h-3L15 21l4-3.99h-3z"/></svg>',
  },
  {
    id: 'shortcuts',
    label: t.value.toolbar.shortcutGuide,
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M20 5H4c-1.1 0-1.99.9-1.99 2L2 17c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm-9 3h2v2h-2V8zm0 3h2v2h-2v-2zM8 8h2v2H8V8zm0 3h2v2H8v-2zm-1 2H5v-2h2v2zm0-3H5V8h2v2zm9 7H8v-2h8v2zm0-4h-2v-2h2v2zm0-3h-2V8h2v2zm3 3h-2v-2h2v2zm0-3h-2V8h2v2z"/></svg>',
  },
  {
    id: 'pet-settings',
    label: t.value.toolbar.petSettings,
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.488.488 0 0 0-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.484.484 0 0 0-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94 0 .31.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/></svg>',
  },
  {
    id: 'close',
    label: t.value.toolbar.close,
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>',
    danger: true,
  },
])

const toggleMenu = () => {
  menuVisible.value = !menuVisible.value
}

const handleOutsideClick = () => {
  // 点击容器空白区域时关闭菜单
  if (menuVisible.value) {
    menuVisible.value = false
  }
}

const handleMenuItem = async (action) => {
  menuVisible.value = false

  if (action === 'pet-settings') {
    try {
      await invoke('handle_toolbar_menu_action', { action: 'pet' })
    } catch (error) {
      console.error('Open pet settings failed:', error)
    }
    return
  }

  try {
    await invoke('handle_toolbar_menu_action', { action })
  } catch (error) {
    console.error('Menu action failed:', error)
  }
}

// 计算气泡位置（六角星布局：固定六个顶点分布）
const getBubbleStyle = (index) => {
  const outerRadius = 80
  const baseAngles = [-90, -30, 30, 90, 150, -150]
  const angle = baseAngles[index % 6]
  const rad = (angle * Math.PI) / 180

  return {
    transform: `translate(${Math.cos(rad) * outerRadius}px, ${Math.sin(rad) * outerRadius}px)`,
    transitionDelay: `${index * 200}ms`,
  }
}

// 后台更新精灵图缓存（ETag 条件请求，静默失败）
const updateSpritesheetCacheInBackground = async (cachedEtag) => {
  try {
    const result = await invoke('fetch_remote_bytes', {
      url: SPRITESHEET_URL,
      ifNoneMatch: cachedEtag || null,
    })
    if (result) {
      // 有新数据
      const resp = await fetch(result.data_url)
      const blob = await resp.blob()
      await setCacheEntry(CACHE_KEY_SPRITE, blob, result.etag)
      return true // 表示有更新
    }
    return false // 304 未修改
  } catch (_) {
    return false
  }
}

// 生成本地 fallback 精灵图（当网络和缓存都不可用时）
// 用 Canvas 绘制 4x4 共 16 帧简单表情图案
const generateFallbackSpritesheet = () => {
  const frameSize = 64
  const cols = 4
  const rows = 4
  const canvas = document.createElement('canvas')
  canvas.width = frameSize * cols
  canvas.height = frameSize * rows
  const ctx = canvas.getContext('2d')

  // 16 种颜色/表情变体
  const colors = [
    '#FFB6C1', '#FF69B4', '#DDA0DD', '#BA55D3',
    '#87CEEB', '#4FC3F7', '#81C784', '#AED581',
    '#FFD54F', '#FFB74D', '#FF8A65', '#F48FB1',
    '#CE93D8', '#9FA8DA', '#80CBC4', '#A5D6A7',
  ]
  const expressions = ['◕‿◕', '≧ω≦', '✧ω✧', '◠‿◠', '≧◡≦', '♡ω♡', 'ↂ‿ↂ', '◕ᴗ◕',
                        'ᵔᴥᵔ', '◕‸◕', 'ᓚᘏᗢ', 'ᘡᘏᗢ', '◔‸◔', '◠ω◠', '✦‿✦', '◕△◕']

  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const i = row * cols + col
      const x = col * frameSize
      const y = row * frameSize
      const cx = x + frameSize / 2
      const cy = y + frameSize / 2

      // 画圆形背景
      ctx.beginPath()
      ctx.arc(cx, cy, frameSize * 0.4, 0, Math.PI * 2)
      ctx.fillStyle = colors[i]
      ctx.fill()
      ctx.strokeStyle = 'rgba(255,255,255,0.6)'
      ctx.lineWidth = 2
      ctx.stroke()

      // 画表情文字
      ctx.fillStyle = '#fff'
      ctx.font = `${frameSize * 0.25}px sans-serif`
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      ctx.fillText(expressions[i], cx, cy)
    }
  }

  console.info('🎨 [桌宠] 已生成本地 fallback 精灵图:', canvas.width, 'x', canvas.height)
  return canvas
}

// 初始化 PixiJS 精灵动画
const initPixiApp = async () => {
  if (!pixiCanvas.value) {
    console.error('❌ [桌宠] pixiCanvas ref 为空，无法初始化')
    return
  }

  const PIXI = await loadPixi()

  // 创建 PixiJS 应用
  pixiApp = new PIXI.Application()
  await pixiApp.init({
    canvas: pixiCanvas.value,
    width: 80,
    height: 80,
    backgroundColor: 0x000000,
    backgroundAlpha: 0,
    antialias: true,
    resolution: window.devicePixelRatio || 1,
    autoDensity: true,
  })

  let spritesheet = null
  let cachedEtag = null

  const cachedEntry = await getCacheEntry(CACHE_KEY_SPRITE)
  if (cachedEntry && cachedEntry.data) {
    try {
      const imageBitmap = await createImageBitmap(cachedEntry.data)
      const texture = PIXI.Texture.from(imageBitmap)
      spritesheet = {
        width: imageBitmap.width,
        height: imageBitmap.height,
        source: texture.source,
      }
      cachedEtag = cachedEntry.etag || null
      console.info('⚡ [桌宠] 缓存精灵图加载成功:', spritesheet.width, 'x', spritesheet.height, 'etag:', cachedEtag)
    } catch (error) {
      console.warn('⚠️ [桌宠] 缓存精灵图加载失败，将从远程下载:', error?.message || String(error))
      spritesheet = null
    }
  }

  if (!spritesheet) {
    // 无缓存，全量下载
    console.info('📥 [桌宠] 通过 Rust 代理下载精灵图...')
    try {
      const result = await invoke('fetch_remote_bytes', { url: SPRITESHEET_URL, ifNoneMatch: null })
      if (result) {
        const img = new Image()
        await new Promise((resolve, reject) => {
          img.onload = resolve
          img.onerror = reject
          img.src = result.data_url
        })
        const texture = PIXI.Texture.from(img)
        spritesheet = {
          width: img.width,
          height: img.height,
          source: texture.source,
        }
        console.info('✅ [桌宠] 精灵图下载成功:', spritesheet.width, 'x', spritesheet.height)

        // 缓存到 IndexedDB（含 ETag）
        try {
          const resp = await fetch(result.data_url)
          const blob = await resp.blob()
          await setCacheEntry(CACHE_KEY_SPRITE, blob, result.etag)
        } catch (cacheErr) {
          console.warn('⚠️ [桌宠] 缓存写入失败（不影响显示）:', cacheErr?.message || String(cacheErr))
        }
      }
    } catch (proxyErr) {
      console.error('❌ [桌宠] 精灵图下载失败:', proxyErr?.message || String(proxyErr))
    }
  } else {
    // 有缓存 → 延迟后台 ETag 条件请求检查更新
    setTimeout(() => {
      updateSpritesheetCacheInBackground(cachedEtag)
    }, 3000)
  }

  // 如果远程和缓存都失败了，使用本地生成的 fallback
  if (!spritesheet) {
    console.warn('⚠️ [桌宠] 远程和缓存均不可用，使用本地 fallback 精灵图')
    try {
      const fallbackCanvas = generateFallbackSpritesheet()
      const texture = PIXI.Texture.from(fallbackCanvas)
      spritesheet = {
        width: fallbackCanvas.width,
        height: fallbackCanvas.height,
        source: texture.source,
      }
    } catch (fallbackErr) {
      console.error('❌ [桌宠] Fallback 精灵图也失败:', fallbackErr?.message || String(fallbackErr))
      return
    }
  }

  // 4列x4行 (16帧)
  const frameWidth = spritesheet.width / 4
  const frameHeight = spritesheet.height / 4

  for (let row = 0; row < 4; row++) {
    for (let col = 0; col < 4; col++) {
      const rect = new PIXI.Rectangle(col * frameWidth, row * frameHeight, frameWidth, frameHeight)
      const texture = new PIXI.Texture({
        source: spritesheet.source,
        frame: rect,
      })
      spriteFrames.push(texture)
    }
  }
  console.info('🐾 [桌宠] 纹理帧创建完毕，共', spriteFrames.length, '帧')

  // 创建精灵并添加到舞台
  currentSprite = new PIXI.Sprite(spriteFrames[0])

  const scale = Math.min(80 / frameWidth, 80 / frameHeight) * 0.9
  currentSprite.scale.set(scale)
  currentSprite.anchor.set(0.5)
  currentSprite.x = 40
  currentSprite.y = 40

  pixiApp.stage.addChild(currentSprite)

  startIdleAnimation()
  console.info('✅ [桌宠] PixiJS 精灵动画已启动')
}

// 随机切换表情/动作帧（静态显示，不连续播放）
const startIdleAnimation = () => {
  // 随机切换表情的定时器
  const switchRandomFrame = () => {
    if (!currentSprite || !spriteFrames.length) return

    // 随机选择一帧显示
    const randomFrame = Math.floor(Math.random() * spriteFrames.length)
    currentSprite.texture = spriteFrames[randomFrame]

    // 下次切换的随机延迟：5-10秒
    const nextDelay = 5000 + Math.random() * 5000
    animationTimer = setTimeout(switchRandomFrame, nextDelay)
  }

  // 首次随机延迟 3-5 秒后开始
  const firstDelay = 3000 + Math.random() * 2000
  animationTimer = setTimeout(switchRandomFrame, firstDelay)
}

// 清理 PixiJS
const cleanupPixiApp = () => {
  if (animationTimer) {
    clearTimeout(animationTimer)
    animationTimer = null
  }
  if (pixiApp) {
    pixiApp.destroy(true, { children: true, texture: true, baseTexture: true })
    pixiApp = null
  }
  spriteFrames = []
  currentSprite = null
}

// 定时刷新间隔（30 分钟）
const REFRESH_INTERVAL_MS = 30 * 60 * 1000
let refreshTimer = null

// 后台定时检查资源更新（ETag 条件请求，无变化不下载）
const startResourceRefresh = () => {
  refreshTimer = setInterval(async () => {
    // 刷新话术（ETag 条件请求）
    try {
      const cachedPhrases = await getCacheEntry(CACHE_KEY_PHRASES)
      const result = await invoke('fetch_speech_phrases', {
        ifNoneMatch: cachedPhrases?.etag || null,
      })
      if (result) {
        speechPhrases.value = result.phrases
        await setCacheEntry(CACHE_KEY_PHRASES, result.phrases, result.etag)
      }
    } catch (_) {}

    // 刷新精灵图缓存（ETag 条件请求，下次启动时生效）
    try {
      const cachedSprite = await getCacheEntry(CACHE_KEY_SPRITE)
      await updateSpritesheetCacheInBackground(cachedSprite?.etag)
    } catch (_) {}
  }, REFRESH_INTERVAL_MS)
}

// 自定义拖拽（使用 PointerEvent 统一处理鼠标和触摸，替代 data-tauri-drag-region）
const appWindow = getCurrentWindow()
let isDragging = false
let hasMoved = false
let dragPointerType = ''
let dragPointerId = null
let dragPointerTarget = null
let dragEnding = false
let dragDisposed = false
let dragGeneration = 0
let dragStartScreenX = 0
let dragStartScreenY = 0

let touchStartClientX = 0
let touchStartClientY = 0
let touchLatestClientX = 0
let touchLatestClientY = 0
let touchBasePhysicalX = Number.NaN
let touchBasePhysicalY = Number.NaN
let touchPendingPhysicalX = Number.NaN
let touchPendingPhysicalY = Number.NaN
let touchScaleFactor = 1
let touchScaleFactorVersion = 0
let touchInitialized = false
let touchPreparing = false
let touchPreparationPromise = null
let touchScaleRebasing = false
let touchScaleRebasePromise = null
let touchIsSettingPos = false
let touchFailed = false
let touchRafId = null
let unlistenDragScaleChanged = null
let dragScaleListenerPromise = null
let nativeDragPollTimer = null
let nativeDragPollResolve = null
let toolbarPositionSaveGeneration = 0
let toolbarPositionSavePromise = Promise.resolve()
const DRAG_THRESHOLD = 3
const NATIVE_DRAG_POLL_INTERVAL_MS = 32

const normalizeDragScaleFactor = (value) => (
  Number.isFinite(value) && value > 0 ? value : 1
)

const isCurrentDrag = (generation, pointerId) => (
  !dragDisposed &&
  generation === dragGeneration &&
  dragPointerId === pointerId
)

const removeDragListeners = () => {
  document.removeEventListener('pointermove', onDragMove)
  document.removeEventListener('pointerup', onDragEnd)
  document.removeEventListener('pointercancel', onDragEnd)
}

const releaseDragPointerCapture = () => {
  if (dragPointerTarget && dragPointerId !== null) {
    try {
      if (dragPointerTarget.hasPointerCapture(dragPointerId)) {
        dragPointerTarget.releasePointerCapture(dragPointerId)
      }
    } catch {}
  }
  dragPointerTarget = null
}

const cancelNativeDragPoll = () => {
  if (nativeDragPollTimer !== null) {
    clearTimeout(nativeDragPollTimer)
    nativeDragPollTimer = null
  }
  if (nativeDragPollResolve) {
    const resolve = nativeDragPollResolve
    nativeDragPollResolve = null
    resolve()
  }
}

const clearDragState = ({ preserveMoved = false } = {}) => {
  dragGeneration += 1
  if (touchRafId !== null) {
    cancelAnimationFrame(touchRafId)
    touchRafId = null
  }
  cancelNativeDragPoll()
  removeDragListeners()
  releaseDragPointerCapture()

  isDragging = false
  dragEnding = false
  dragPointerType = ''
  dragPointerId = null
  touchInitialized = false
  touchPreparing = false
  touchPreparationPromise = null
  touchScaleRebasing = false
  touchScaleRebasePromise = null
  touchIsSettingPos = false
  touchFailed = false
  touchBasePhysicalX = Number.NaN
  touchBasePhysicalY = Number.NaN
  touchPendingPhysicalX = Number.NaN
  touchPendingPhysicalY = Number.NaN
  if (!preserveMoved) {
    hasMoved = false
  }
}

const resetMovedAfterClick = () => {
  const generation = dragGeneration
  requestAnimationFrame(() => {
    if (generation === dragGeneration) {
      hasMoved = false
    }
  })
}

const finishDrag = (generation, pointerId, preserveMoved = false) => {
  if (!isCurrentDrag(generation, pointerId)) return

  clearDragState({ preserveMoved })
  if (preserveMoved) {
    resetMovedAfterClick()
  }
}

const commitTouchPosition = (physicalX, physicalY) => (
  appWindow.setPosition(new PhysicalPosition(
    Math.round(physicalX),
    Math.round(physicalY),
  ))
)

const queueToolbarPositionSave = (position) => {
  const generation = ++toolbarPositionSaveGeneration
  toolbarPositionSavePromise = toolbarPositionSavePromise
    .catch(() => {})
    .then(async () => {
      if (generation !== toolbarPositionSaveGeneration) return

      await invoke('save_toolbar_position', { x: position.x, y: position.y })
    })
    .catch(() => {})
}

const markTouchDragFailed = (generation, pointerId) => {
  if (!isCurrentDrag(generation, pointerId)) return

  touchFailed = true
  if (touchRafId !== null) {
    cancelAnimationFrame(touchRafId)
    touchRafId = null
  }
}

const waitForNativeDragPoll = () => new Promise((resolve) => {
  nativeDragPollResolve = resolve
  nativeDragPollTimer = setTimeout(() => {
    nativeDragPollTimer = null
    nativeDragPollResolve = null
    resolve()
  }, NATIVE_DRAG_POLL_INTERVAL_MS)
})

const finishNativeDrag = async (generation, pointerId) => {
  // startDragging() resolves after dispatching the native command, not when
  // the Windows move loop ends. Keep the session active until the button lifts.
  while (isCurrentDrag(generation, pointerId)) {
    const pressed = await invoke('is_primary_mouse_button_pressed')
    if (!isCurrentDrag(generation, pointerId)) return
    if (pressed !== true) break

    await waitForNativeDragPoll()
  }
  if (!isCurrentDrag(generation, pointerId)) return

  const finalPos = await appWindow.outerPosition()
  if (!isCurrentDrag(generation, pointerId)) return

  queueToolbarPositionSave(finalPos)
  finishDrag(generation, pointerId, true)
}

const rebaseTouchDragForScaleChange = (scaleFactor) => {
  touchScaleFactorVersion += 1
  touchScaleFactor = normalizeDragScaleFactor(scaleFactor)
  if (
    dragPointerType !== 'touch' ||
    dragPointerId === null ||
    dragEnding ||
    touchFailed ||
    !touchInitialized ||
    touchScaleRebasePromise
  ) {
    return
  }

  const generation = dragGeneration
  const pointerId = dragPointerId
  touchScaleRebasing = true
  if (touchRafId !== null) {
    cancelAnimationFrame(touchRafId)
    touchRafId = null
  }

  let rebasePromise
  rebasePromise = (async () => {
    while (touchIsSettingPos) {
      await new Promise((resolve) => setTimeout(resolve, 0))
      if (!isCurrentDrag(generation, pointerId)) return
    }

    await new Promise((resolve) => requestAnimationFrame(resolve))
    if (!isCurrentDrag(generation, pointerId) || dragEnding) return

    const position = await appWindow.outerPosition()
    if (!isCurrentDrag(generation, pointerId) || dragEnding) return

    touchBasePhysicalX = position.x
    touchBasePhysicalY = position.y
    touchPendingPhysicalX = position.x
    touchPendingPhysicalY = position.y
    touchStartClientX = touchLatestClientX
    touchStartClientY = touchLatestClientY
  })()
    .catch(() => {
      markTouchDragFailed(generation, pointerId)
    })
    .finally(() => {
      if (isCurrentDrag(generation, pointerId)) {
        touchScaleRebasing = false
      }
      if (touchScaleRebasePromise === rebasePromise) {
        touchScaleRebasePromise = null
      }
    })

  touchScaleRebasePromise = rebasePromise
}

const ensureDragScaleListener = () => {
  if (dragDisposed || unlistenDragScaleChanged || dragScaleListenerPromise) return

  dragScaleListenerPromise = appWindow
    .onScaleChanged(({ payload }) => {
      rebaseTouchDragForScaleChange(payload.scaleFactor)
    })
    .then((unlisten) => {
      dragScaleListenerPromise = null
      if (dragDisposed) {
        unlisten()
        return
      }
      unlistenDragScaleChanged = unlisten
    })
    .catch(() => {
      dragScaleListenerPromise = null
    })
}

const queueTouchPosition = () => {
  // WebView2 touch coordinates are relative to the moving viewport. Keep only
  // one position IPC in flight so the next pointer sample uses the new baseline.
  if (
    dragPointerId === null ||
    dragEnding ||
    touchFailed ||
    touchPreparing ||
    touchScaleRebasing ||
    touchIsSettingPos ||
    !touchInitialized ||
    !Number.isFinite(touchBasePhysicalX) ||
    !Number.isFinite(touchBasePhysicalY)
  ) {
    return
  }

  updatePendingTouchPosition()

  if (touchRafId === null) {
    touchRafId = requestAnimationFrame(touchApplyPosition)
  }
}

const updatePendingTouchPosition = () => {
  if (
    !touchInitialized ||
    !Number.isFinite(touchBasePhysicalX) ||
    !Number.isFinite(touchBasePhysicalY)
  ) {
    return false
  }

  touchPendingPhysicalX = touchBasePhysicalX +
    (touchLatestClientX - touchStartClientX) * touchScaleFactor
  touchPendingPhysicalY = touchBasePhysicalY +
    (touchLatestClientY - touchStartClientY) * touchScaleFactor
  return Number.isFinite(touchPendingPhysicalX) &&
    Number.isFinite(touchPendingPhysicalY)
}

const prepareTouchDrag = async (generation, pointerId) => {
  const initialScaleFactorVersion = touchScaleFactorVersion
  const [position, scaleFactor] = await Promise.all([
    appWindow.outerPosition(),
    appWindow.scaleFactor(),
  ])
  if (!isCurrentDrag(generation, pointerId)) return

  if (touchScaleFactorVersion === initialScaleFactorVersion) {
    touchScaleFactor = normalizeDragScaleFactor(scaleFactor)
  }
  touchBasePhysicalX = position.x
  touchBasePhysicalY = position.y
  touchPendingPhysicalX = position.x
  touchPendingPhysicalY = position.y
  touchInitialized = true

  if (hasMoved && !dragEnding) {
    queueTouchPosition()
  }
}

const onContainerDragStart = (e) => {
  if (menuVisible.value) {
    onDragStart(e)
  }
}

const onDragStart = (e) => {
  if (
    e.button !== 0 ||
    isDragging ||
    dragPointerId !== null ||
    (e.pointerType === 'touch' && !e.isPrimary)
  ) {
    return
  }

  const generation = ++dragGeneration
  const pointerId = e.pointerId
  e.preventDefault()
  isDragging = true
  hasMoved = false
  dragEnding = false
  dragPointerType = e.pointerType
  dragPointerId = pointerId
  dragPointerTarget = e.currentTarget
  dragStartScreenX = e.screenX
  dragStartScreenY = e.screenY

  try {
    dragPointerTarget.setPointerCapture(dragPointerId)
  } catch {}

  document.addEventListener('pointermove', onDragMove, { passive: false })
  document.addEventListener('pointerup', onDragEnd)
  document.addEventListener('pointercancel', onDragEnd)

  if (e.pointerType === 'touch') {
    ensureDragScaleListener()
    touchStartClientX = e.clientX
    touchStartClientY = e.clientY
    touchLatestClientX = e.clientX
    touchLatestClientY = e.clientY
    touchBasePhysicalX = Number.NaN
    touchBasePhysicalY = Number.NaN
    touchPendingPhysicalX = Number.NaN
    touchPendingPhysicalY = Number.NaN
    touchInitialized = false
    touchPreparing = true
    touchIsSettingPos = false
    touchFailed = false
    touchScaleRebasing = false

    let preparationPromise
    preparationPromise = prepareTouchDrag(generation, pointerId)
      .catch(() => {
        markTouchDragFailed(generation, pointerId)
      })
      .finally(() => {
        if (isCurrentDrag(generation, pointerId)) {
          touchPreparing = false
        }
        if (touchPreparationPromise === preparationPromise) {
          touchPreparationPromise = null
        }
      })
    touchPreparationPromise = preparationPromise
  }
}

const touchApplyPosition = async () => {
  touchRafId = null
  if (
    !isDragging ||
    dragEnding ||
    touchFailed ||
    touchPreparing ||
    touchScaleRebasing ||
    touchIsSettingPos ||
    !touchInitialized ||
    !Number.isFinite(touchPendingPhysicalX) ||
    !Number.isFinite(touchPendingPhysicalY)
  ) {
    return
  }

  const generation = dragGeneration
  const pointerId = dragPointerId
  const nextPhysicalX = touchPendingPhysicalX
  const nextPhysicalY = touchPendingPhysicalY
  touchIsSettingPos = true

  try {
    await commitTouchPosition(nextPhysicalX, nextPhysicalY)
    if (!isCurrentDrag(generation, pointerId)) return

    touchBasePhysicalX = nextPhysicalX
    touchBasePhysicalY = nextPhysicalY
  } catch {
    markTouchDragFailed(generation, pointerId)
  } finally {
    if (isCurrentDrag(generation, pointerId)) {
      touchIsSettingPos = false
    }
  }
}

const onDragMove = (e) => {
  if (
    !isDragging ||
    e.pointerId !== dragPointerId ||
    e.pointerType !== dragPointerType ||
    dragEnding
  ) {
    return
  }

  if (dragPointerType === 'mouse' || dragPointerType === 'pen') {
    const dx = e.screenX - dragStartScreenX
    const dy = e.screenY - dragStartScreenY
    if (!hasMoved && Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return
    hasMoved = true
    e.preventDefault()
    const generation = dragGeneration
    const pointerId = dragPointerId
    isDragging = false
    dragEnding = true
    removeDragListeners()
    releaseDragPointerCapture()
    appWindow.startDragging()
      .then(() => finishNativeDrag(generation, pointerId))
      .catch(() => {
        finishDrag(generation, pointerId, true)
      })
    return
  }

  touchLatestClientX = e.clientX
  touchLatestClientY = e.clientY
  const deltaX = touchLatestClientX - touchStartClientX
  const deltaY = touchLatestClientY - touchStartClientY
  if (
    !hasMoved &&
    Math.abs(deltaX) < DRAG_THRESHOLD &&
    Math.abs(deltaY) < DRAG_THRESHOLD
  ) {
    return
  }

  hasMoved = true
  e.preventDefault()
  queueTouchPosition()
}

const onDragEnd = async (e) => {
  if (
    dragPointerId === null ||
    e.pointerId !== dragPointerId ||
    e.pointerType !== dragPointerType
  ) {
    return
  }

  const generation = dragGeneration
  const pointerId = dragPointerId
  const wasDragged = isDragging && hasMoved
  // A sample received during setPosition belongs to the previous viewport
  // coordinate frame and cannot be safely reapplied after that call completes.
  const canRefreshFinalTouchPosition =
    dragPointerType === 'touch' &&
    !touchIsSettingPos
  if (dragPointerType === 'touch') {
    touchLatestClientX = e.clientX
    touchLatestClientY = e.clientY
  }

  isDragging = false
  dragEnding = true
  removeDragListeners()
  releaseDragPointerCapture()
  if (touchRafId !== null) {
    cancelAnimationFrame(touchRafId)
    touchRafId = null
  }

  if (wasDragged && dragPointerType === 'touch') {
    if (touchPreparationPromise) {
      await touchPreparationPromise
      if (!isCurrentDrag(generation, pointerId)) return
    }
    if (touchScaleRebasePromise) {
      await touchScaleRebasePromise
      if (!isCurrentDrag(generation, pointerId)) return
    }
    while (touchIsSettingPos) {
      await new Promise((resolve) => setTimeout(resolve, 0))
      if (!isCurrentDrag(generation, pointerId)) return
    }

    if (
      !touchFailed &&
      canRefreshFinalTouchPosition &&
      isCurrentDrag(generation, pointerId)
    ) {
      updatePendingTouchPosition()
    }

    if (
      !touchFailed &&
      touchInitialized &&
      Number.isFinite(touchPendingPhysicalX) &&
      Number.isFinite(touchPendingPhysicalY)
    ) {
      try {
        await commitTouchPosition(touchPendingPhysicalX, touchPendingPhysicalY)
        if (!isCurrentDrag(generation, pointerId)) return

        const finalPos = await appWindow.outerPosition()
        if (!isCurrentDrag(generation, pointerId)) return

        queueToolbarPositionSave(finalPos)
      } catch {}
    }
  }

  finishDrag(generation, pointerId, wasDragged)
}

const onIconClick = () => {
  if (!hasMoved) toggleMenu()
}

// 窗口鼠标命中测试：透明区域穿透到下层窗口/桌面。
// Tauri 透明窗口默认整窗吃鼠标事件，CSS 的 pointer-events:none 只影响 DOM，
// 必须用 setIgnoreCursorEvents 在 OS 层切换。
//
// 难点：当 ignoreCursorEvents=true 时，DOM 收不到任何鼠标事件，
// 所以必须用全局光标轮询（cursorPosition + outerPosition）来判断是否进入桌宠区域。
//
// 命中规则：
//   - 鼠标在内圈 80×80 桌宠图标 → ignore=false（可点击/拖拽）
//   - 菜单展开时：鼠标在 240×240 整窗内 → ignore=false（可点气泡按钮 / 点空白关闭）
//   - 否则（外圈空白 / 完全在窗口外）→ ignore=true（穿透到下层）
let cursorIgnoreState = false
let hitTestTimer = null
let hitTestEnabled = false
const HIT_TEST_ACTIVE_INTERVAL_MS = 80
const HIT_TEST_IDLE_INTERVAL_MS = 250
const setCursorIgnore = (ignore) => {
  if (ignore === cursorIgnoreState) return
  cursorIgnoreState = ignore
  appWindow.setIgnoreCursorEvents(ignore).catch((e) => {
    console.warn('[桌宠HitTest] setIgnoreCursorEvents 失败', ignore, e)
  })
}

const hitTestTick = async () => {
  // 拖拽会话结束前维持非穿透，避免打断原生拖拽或触摸收尾。
  if (dragPointerId !== null) {
    setCursorIgnore(false)
    return
  }
  try {
    const [winPos, winSize, cur] = await Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      cursorPosition(),
    ])
    if (dragPointerId !== null) {
      setCursorIgnore(false)
      return
    }

    // PhysicalPosition: 物理像素
    const relX = cur.x - winPos.x
    const relY = cur.y - winPos.y
    const inWindow = relX >= 0 && relY >= 0 && relX < winSize.width && relY < winSize.height
    if (!inWindow) {
      setCursorIgnore(true)
      return
    }
    if (menuVisible.value) {
      setCursorIgnore(false)
      return
    }
    const dpr = window.devicePixelRatio || 1
    const cx = winSize.width / 2
    const cy = winSize.height / 2
    const halfIcon = 40 * dpr
    const inIcon = Math.abs(relX - cx) <= halfIcon && Math.abs(relY - cy) <= halfIcon
    setCursorIgnore(!inIcon)
  } catch (e) {
    console.warn('[桌宠HitTest] tick 失败', e)
  }
}

const initBubbleClickThrough = () => {
  // 初始进入穿透状态，由轮询决定何时取消
  hitTestEnabled = true
  setCursorIgnore(true)
  const scheduleNextHitTest = (delay) => {
    if (!hitTestEnabled) return
    hitTestTimer = setTimeout(async () => {
      if (!hitTestEnabled) return
      await hitTestTick()
      if (!hitTestEnabled) return
      const active = dragPointerId !== null || menuVisible.value || !cursorIgnoreState
      scheduleNextHitTest(active ? HIT_TEST_ACTIVE_INTERVAL_MS : HIT_TEST_IDLE_INTERVAL_MS)
    }, delay)
  }
  scheduleNextHitTest(0)
}

onMounted(async () => {
  try {
    await initPixiApp()
  } catch (error) {
    console.error('❌ [桌宠] PixiJS 初始化异常:', error?.message || String(error))
  }
  startSpeechLoop()
  loadSpeechPhrases()
  startResourceRefresh()
  initSpeechBroadcast()
  initBubbleClickThrough()
})

onUnmounted(() => {
  dragDisposed = true
  clearDragState()
  if (unlistenDragScaleChanged) {
    unlistenDragScaleChanged()
    unlistenDragScaleChanged = null
  }
  if (refreshTimer) {
    clearInterval(refreshTimer)
    refreshTimer = null
  }
  if (speechInterval) {
    clearInterval(speechInterval)
    speechInterval = null
  }
  if (speechTimer) {
    clearTimeout(speechTimer)
    speechTimer = null
  }
  cleanupPixiApp()
  // 清理拖拽
  hitTestEnabled = false
  if (hitTestTimer) { clearTimeout(hitTestTimer); hitTestTimer = null }
  if (speechBroadcast) {
    try { speechBroadcast.close() } catch (_) {}
    speechBroadcast = null
  }
})
</script>

<style scoped lang="less">
// 变量定义
@pink-light: rgba(255, 182, 193, 0.95);
@pink-dark: rgba(255, 192, 203, 0.95);
@blue-light: rgba(173, 216, 230, 0.95);
@blue-dark: rgba(135, 206, 250, 0.95);
@purple-light: rgba(221, 160, 221, 0.95);
@purple-dark: rgba(218, 112, 214, 0.95);
@orange-light: rgba(255, 193, 7, 0.95);
@orange-dark: rgba(255, 152, 0, 0.95);
@danger-light: rgba(255, 182, 193, 0.95);
@danger-dark: rgba(255, 150, 150, 0.95);

@halo-default: drop-shadow(0 0 2px rgba(255, 182, 193, 0.1)) drop-shadow(0 0 4px rgba(221, 160, 221, 0.05));
@halo-hover: drop-shadow(0 0 3px rgba(255, 182, 193, 0.16)) drop-shadow(0 0 5px rgba(221, 160, 221, 0.07));
@halo-active: drop-shadow(0 0 3px rgba(123, 80, 87, 0.2)) drop-shadow(0 0 6px rgba(221, 160, 221, 0.08));

@transition-bounce: cubic-bezier(0.34, 1.56, 0.64, 1);

// Mixins
.gpu-accelerate() {
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
}

.bubble-shadow(@color) {
  box-shadow: 0 2px 10px fade(@color, 35%), 0 0 0 2px rgba(255, 255, 255, 0.3), inset 0 1px 4px rgba(255, 255, 255, 0.2);
}

.bubble-shadow-hover(@color) {
  box-shadow: 0 4px 18px fade(@color, 55%), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 6px rgba(255, 255, 255, 0.35);
}

#toolbar-container {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  box-sizing: border-box;
  // 默认状态（菜单收起）：容器不响应鼠标/触控，只有图标可交互
  pointer-events: none;
  .gpu-accelerate();
  -webkit-font-smoothing: antialiased;

  // 菜单展开状态：容器响应点击（用于点击空白关闭菜单）
  &.menu-open {
    pointer-events: auto;
    touch-action: none;
  }
}

.bubble-menu {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 50;
  .gpu-accelerate();
}

.bubble-wrapper {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 48px;
  height: 48px;
  margin: -24px 0 0 -24px;
  pointer-events: all;
  will-change: transform, margin-top;
  .gpu-accelerate();
}

.bubble-item {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: linear-gradient(135deg, @pink-light 0%, rgba(255, 160, 220, 0.95) 50%, rgba(186, 148, 255, 0.95) 100%);
  backdrop-filter: blur(15px);
  .bubble-shadow(rgb(255, 182, 193));
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s @transition-bounce;
  animation: bubbleIn 0.6s @transition-bounce both;
  position: relative;
  will-change: transform, opacity, box-shadow;
  .gpu-accelerate();
  -webkit-font-smoothing: antialiased;

  &:hover {
    .bubble-shadow-hover(rgb(255, 182, 193));
    z-index: 10;
    transform: scale(1.1) translateZ(0);

    .bubble-icon {
      transform: scale(1.2) rotate(15deg);
    }
  }

  &.danger {
    background: linear-gradient(135deg, @danger-light 0%, @danger-dark 100%);
    .bubble-shadow(rgb(255, 150, 150));

    &:hover {
      .bubble-shadow-hover(rgb(255, 150, 150));
    }
  }
}

// 气泡颜色变体
.bubble-wrapper:nth-child(1) .bubble-item {
  background: linear-gradient(135deg, @pink-light 0%, @pink-dark 100%);
  .bubble-shadow(rgb(255, 182, 193));
}

.bubble-wrapper:nth-child(2) .bubble-item {
  background: linear-gradient(135deg, @blue-light 0%, @blue-dark 100%);
  .bubble-shadow(rgb(173, 216, 230));
}

.bubble-wrapper:nth-child(3) .bubble-item {
  background: linear-gradient(135deg, @purple-light 0%, @purple-dark 100%);
  .bubble-shadow(rgb(221, 160, 221));
}

.bubble-wrapper:nth-child(4) .bubble-item {
  background: linear-gradient(135deg, @orange-light 0%, @orange-dark 100%);
  .bubble-shadow(rgb(255, 193, 7));
}

.bubble-icon {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.3s @transition-bounce;
  will-change: transform;
  backface-visibility: hidden;
  animation: iconScale 0.5s @transition-bounce both;
  animation-delay: inherit;

  svg {
    width: 100%;
    height: 100%;
  }
}

.toolbar-icon {
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  background: transparent;
  border: none;
  padding: 0;
  margin: 0;
  border-radius: 50%;
  animation: float 4s ease-in-out infinite;
  filter: @halo-default;
  transition: all 0.4s @transition-bounce;
  position: relative;
  z-index: 100;
  pointer-events: auto;  // 始终可交互（覆盖容器的 pointer-events: none）
  touch-action: none;    // 阻止浏览器默认触摸手势
  .gpu-accelerate();
  -webkit-font-smoothing: antialiased;

  &:hover {
    animation: pulse 2.4s ease-in-out infinite;
    filter: @halo-hover;
  }

  &.active {
    transform: scale(1.12) translateZ(0);
    filter: @halo-active;
  }
}

.icon-image,
.icon-canvas {
  width: 100%;
  height: 100%;
  pointer-events: none;
  display: block;
}

.icon-image {
  object-fit: contain;
  .gpu-accelerate();
}

.speech-bubble {
  position: absolute;
  bottom: calc(50% + 60px);
  left: 50%;
  transform: translateX(-50%);
  max-width: 220px;
  width: max-content;
  padding: 8px 12px;
  color: #4b2b34;
  font-size: 12px;
  line-height: 1.4;
  background: rgba(255, 248, 252, 0.95);
  border-radius: 12px;
  pointer-events: none;
  white-space: normal;
  word-break: break-all;
  z-index: 150;

  &::after {
    content: '';
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 6px solid transparent;
    border-top-color: rgba(255, 248, 252, 0.95);
  }
}

.bubble-enter-active {
  transition: opacity 0.3s;
}

.bubble-leave-active {
  transition: opacity 0.2s;
}

.bubble-enter-from,
.bubble-leave-to {
  opacity: 0;
}

.bubble-enter-from .bubble-item {
  transform: scale(0);
}

// 关键帧动画
@keyframes bubbleIn {
  0% {
    opacity: 0;
    transform: scale(0) rotate(-180deg) translate3d(0, 0, 0);
  }
  70% {
    transform: scale(1.1) rotate(10deg) translate3d(0, 0, 0);
  }
  100% {
    opacity: 1;
    transform: scale(1) rotate(0deg) translate3d(0, 0, 0);
  }
}

@keyframes iconScale {
  0% {
    transform: scale(0) rotate(-90deg) translateZ(0);
  }
  60% {
    transform: scale(1.2) rotate(10deg) translateZ(0);
  }
  100% {
    transform: scale(1) rotate(0deg) translateZ(0);
  }
}

@keyframes float {
  0%,
  100% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  50% {
    transform: translate3d(0, -7px, 0) scale(1);
  }
}

@keyframes pulse {
  0%,
  100% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  25% {
    transform: translate3d(0, -5px, 0) scale(1.05);
  }
  50% {
    transform: translate3d(0, -10px, 0) scale(1.1);
  }
  75% {
    transform: translate3d(0, -5px, 0) scale(1.05);
  }
}
</style>
