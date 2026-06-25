/**
 * 桌宠视觉观察模块
 * 定时截取桌面截图，发送给多模态 LLM，让米塔根据用户桌面内容生成吐槽/调侃文本
 */

import { ref } from 'vue'
import { callVisionLLM, isApiKeyRequired } from './aiClient.js'
import { STORAGE_KEY, DEFAULT_CONFIG } from './aiProviders.js'

// 桌宠视觉观察的系统提示词
const PET_VISION_PROMPT = `你是一个可爱但毒舌的桌面宠物"米塔"。你正在偷看用户的电脑屏幕。

## 你的任务
根据截图内容，假定用户正在做某件事，然后调戏用户。不要用"你是不是在..."这种猜测句式，而是直接断言"你又在..."来调侃。

## 你的性格
- 雌小鬼风格：嘲笑但不恶意，傲娇但关心用户
- 常用口癖：杂鱼♡、哼、切、笨蛋
- 偶尔表现出关心：比如看到用户加班会说"都这么晚了还不睡觉吗...笨蛋"

## 示例风格
- 看到游戏→"又在打游戏偷懒了♡ 杂鱼的操作真是一言难尽呢～"
- 看到代码→"写了半天bug又多了吧，杂鱼程序员～"
- 看到摸鱼→"上班时间逛这个，被老板看到可就惨了呢♡"
- 看到聊天→"跟谁聊得这么开心？哼，才不在意呢"

## 规则
- 只输出一句话（15-40字），不要解释
- 直接断言用户在做什么，不要猜测
- 不要重复说同样的话
- 用中文回复`

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

async function observe() {
  const config = getAiConfig()
  if (!config.enabled || (!config.apiKey && isApiKeyRequired(config))) {
    console.warn('[桌宠] AI 未启用或未配置 API Key，跳过观察')
    petMessage.value = '（米塔还没有连接到 AI 服务，请先在设置中配置 AI 助手~）'
    return
  }

  isObserving.value = true
  try {
    console.log('[桌宠] 开始截屏...')
    const screenshot = await captureScreen()
    console.log('[桌宠] 截屏完成，调用 Vision LLM...')
    const response = await callVisionLLM(
      config,
      PET_VISION_PROMPT,
      '看看我的桌面，说点什么吧',
      screenshot,
      150
    )

    if (response && response.trim()) {
      console.log('[桌宠] LLM 回复:', response.trim())
      petMessage.value = response.trim()
      lastObserveTime.value = Date.now()
    }
  } catch (err) {
    const errMsg = typeof err === 'string' ? err : (err?.message || JSON.stringify(err))
    console.warn('[桌宠] 观察失败:', errMsg, err)
    petMessage.value = `（观察失败: ${errMsg}）`
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
