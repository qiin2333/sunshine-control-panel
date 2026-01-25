<template>
  <div id="toolbar-container" @click.self="handleOutsideClick" data-tauri-drag-region>
    <!-- 气泡菜单 -->
    <transition name="bubble">
      <div v-if="menuVisible" class="bubble-menu" @click.stop data-tauri-drag-region="false">
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
      data-tauri-drag-region="false"
      @click.stop="toggleMenu"
      @contextmenu.prevent="toggleMenu"
    >
      <!-- PixiJS Canvas 容器 -->
      <canvas ref="pixiCanvas" class="icon-canvas"></canvas>
    </div>
  </div>
</template>

<script setup>
import { ref, onUnmounted, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import * as PIXI from 'pixi.js'

const menuVisible = ref(false)
const speechVisible = ref(false)
const speechText = ref('')
let speechTimer = null
let speechInterval = null

// PixiJS 相关
const pixiCanvas = ref(null)
let pixiApp = null
let spriteFrames = []
let currentSprite = null
let currentFrameIndex = 0
let animationTimer = null

// 精灵图集 URL
const SPRITESHEET_URL =
  'https://hub.gitmirror.com/raw.githubusercontent.com/qiin2333/qiin.github.io/assets/img/toolbar-spritesheet.png'

// 精灵图集别名，便于使用 PixiJS 资源缓存
const SPRITESHEET_ALIAS = 'toolbar-spritesheet'

// IndexedDB 缓存配置
const DB_NAME = 'toolbar-cache'
const DB_STORE = 'images'
const CACHE_KEY = 'spritesheet-blob'

// 打开 IndexedDB
const openDB = () => {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1)
    request.onerror = () => reject(request.error)
    request.onsuccess = () => resolve(request.result)
    request.onupgradeneeded = (event) => {
      const db = event.target.result
      if (!db.objectStoreNames.contains(DB_STORE)) {
        db.createObjectStore(DB_STORE)
      }
    }
  })
}

// 从 IndexedDB 获取缓存的 Blob
const getCachedBlob = async () => {
  try {
    const db = await openDB()
    const transaction = db.transaction([DB_STORE], 'readonly')
    const store = transaction.objectStore(DB_STORE)
    return new Promise((resolve, reject) => {
      const request = store.get(CACHE_KEY)
      request.onsuccess = () => resolve(request.result)
      request.onerror = () => reject(request.error)
    })
  } catch (error) {
    console.warn('⚠️  IndexedDB 读取失败:', error)
    return null
  }
}

// 保存 Blob 到 IndexedDB
const saveBlobToCache = async (blob) => {
  try {
    const db = await openDB()
    const transaction = db.transaction([DB_STORE], 'readwrite')
    const store = transaction.objectStore(DB_STORE)
    await new Promise((resolve, reject) => {
      const request = store.put(blob, CACHE_KEY)
      request.onsuccess = () => resolve()
      request.onerror = () => reject(request.error)
    })
    console.log('✅ 精灵图已缓存到 IndexedDB')
  } catch (error) {
    console.warn('⚠️  IndexedDB 保存失败:', error)
  }
}

// 默认话术（fallback）
const defaultPhrases = [
  '杂鱼～杂鱼～',
  '串流画质又调低了？杂鱼～',
  '码率不够高哦，杂鱼看得清吗♡',
  '延迟这么高，杂鱼在干什么呢～',
  '帧率掉了吧？杂鱼的网络不太行呢',
  '虚拟显示器开着呢，杂鱼想看什么？',
  '嘿嘿，杂鱼又在偷偷串流了～',
  'DPI调那么高，杂鱼眼睛受得了吗♡',
  '连接不稳定哦，杂鱼要检查网络啦～',
  '串流质量还不错嘛，杂鱼今天很乖♡',
  '又在调码率了？杂鱼真是麻烦呢～',
  '分辨率调这么低，杂鱼是想省流量吗',
  '串流开这么久，杂鱼不累吗？',
  '网络波动了哦，杂鱼要注意啦♡',
  '画面卡顿了吧？杂鱼就是杂鱼～',
  '音频延迟了呢，杂鱼听得清吗♡',
  '串流设置改来改去，杂鱼真挑剔～',
]

// 响应式话术列表
const speechPhrases = ref([...defaultPhrases])

// 通过后端代理加载话术（延迟加载，不阻塞图标显示）
const loadSpeechPhrases = async () => {
  try {
    console.log('💬 开始加载话术配置...')
    const phrases = await invoke('fetch_speech_phrases')
    if (Array.isArray(phrases) && phrases.length > 0) {
      speechPhrases.value = phrases
      console.log('✅ 话术加载成功，共', phrases.length, '条')
    } else {
      console.warn('⚠️  话术格式错误，使用默认话术')
    }
  } catch (error) {
    console.warn('⚠️  话术加载失败，使用默认话术:', error)
  }
}

const showSpeech = () => {
  if (speechVisible.value) return
  const phrases = speechPhrases.value
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

const startSpeechLoop = () => {
  // 首次延迟随机出现
  const firstDelay = 4000 + Math.random() * 6000
  setTimeout(() => showSpeech(), firstDelay)
  // 后续随机间隔 15s ~ 35s
  speechInterval = setInterval(() => {
    // 避免菜单展开时打断交互
    if (!menuVisible.value) {
      showSpeech()
    }
  }, 15000 + Math.random() * 20000)
}

const menuItems = [
  {
    id: 'main',
    label: '控制面板',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/></svg>',
  },
  {
    id: 'vdd',
    label: '虚拟显示器',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z"/></svg>',
  },
  {
    id: 'dpi',
    label: '调整 DPI',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M21 3H3c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H3V5h18v14zM5 7h5v5H5zm6 0h8v2h-8zm0 3h8v2h-8zM5 13h5v5H5zm6 0h8v2h-8z"/></svg>',
  },
  {
    id: 'bitrate',
    label: '码率调整',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M9 3L5 6.99h3V14h2V6.99h3L9 3zm7 14.01V10h-2v7.01h-3L15 21l4-3.99h-3z"/></svg>',
  },
  {
    id: 'shortcuts',
    label: '快捷键手册',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M20 5H4c-1.1 0-1.99.9-1.99 2L2 17c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm-9 3h2v2h-2V8zm0 3h2v2h-2v-2zM8 8h2v2H8V8zm0 3h2v2H8v-2zm-1 2H5v-2h2v2zm0-3H5V8h2v2zm9 7H8v-2h8v2zm0-4h-2v-2h2v2zm0-3h-2V8h2v2zm3 3h-2v-2h2v2zm0-3h-2V8h2v2z"/></svg>',
  },
  {
    id: 'close',
    label: '关闭',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>',
    danger: true,
  },
]

const toggleMenu = (e) => {
  if (e) {
    e.preventDefault()
    e.stopPropagation()
  }

  console.log('切换菜单，当前状态:', menuVisible.value)
  menuVisible.value = !menuVisible.value
  console.log('新状态:', menuVisible.value)
}

const handleOutsideClick = () => {
  // 点击容器空白区域时关闭菜单
  if (menuVisible.value) {
    menuVisible.value = false
  }
}

const handleMenuItem = async (action) => {
  menuVisible.value = false

  try {
    await invoke('handle_toolbar_menu_action', { action })
  } catch (error) {
    console.error('菜单操作失败:', error)
  }
}

// 计算气泡位置（六角星布局：固定六个顶点分布）
const getBubbleStyle = (index) => {
  const outerRadius = 80 // 外圈半径
  const jitter = 0 // 轻微抖动保留为 0，便于后续微调

  // 六角星（大卫星）可视为正六边形的六个顶点
  // 顶点从顶部开始，顺时针每 60° 一个
  const baseAngles = [-90, -30, 30, 90, 150, -150]
  const k = index % 6
  const angle = baseAngles[k]
  const rad = (angle * Math.PI) / 180

  const x = Math.cos(rad) * outerRadius + (jitter ? (Math.random() - 0.5) * jitter : 0)
  const y = Math.sin(rad) * outerRadius + (jitter ? (Math.random() - 0.5) * jitter : 0)

  return {
    transform: `translate(${x}px, ${y}px)`,
    transitionDelay: `${index * 200}ms`,
  }
}

// 后台更新精灵图缓存（静默失败，不影响用户体验）
const updateSpritesheetCacheInBackground = () => {
  const updateUrl = `${SPRITESHEET_URL}?t=${Date.now()}`
  fetch(updateUrl)
    .then((res) => {
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`)
      }
      return res.blob()
    })
    .then((blob) => saveBlobToCache(blob))
    .catch((err) => {
      console.debug('📦 后台更新精灵图跳过（网络不可用或资源暂时无法访问）')
    })
}

// 初始化 PixiJS 精灵动画
const initPixiApp = async () => {
  if (!pixiCanvas.value) return

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
  let usedCache = false

  const cachedBlob = await getCachedBlob()
  if (cachedBlob) {
    try {
      console.log('⚡ 使用 IndexedDB 缓存的精灵图')

      // 将 Blob 转换为 ImageBitmap（PixiJS 支持）
      const imageBitmap = await createImageBitmap(cachedBlob)

      // 从 ImageBitmap 创建纹理（PixiJS 会自动处理）
      const texture = PIXI.Texture.from(imageBitmap)

      // 创建一个兼容的 spritesheet 对象
      spritesheet = {
        width: imageBitmap.width,
        height: imageBitmap.height,
        source: texture.source,
      }

      usedCache = true
      console.log('✅ 缓存的精灵图加载成功', spritesheet.width, 'x', spritesheet.height)
    } catch (error) {
      console.warn('⚠️  缓存的精灵图加载失败，将重新下载:', error)
      spritesheet = null
    }
  }

  if (!spritesheet) {
    console.log('📥 首次加载精灵图')
    if (!PIXI.Assets.resolver.hasKey(SPRITESHEET_ALIAS)) {
      PIXI.Assets.add({ alias: SPRITESHEET_ALIAS, src: SPRITESHEET_URL })
    }
    spritesheet = await PIXI.Assets.load(SPRITESHEET_ALIAS)
  }

  // 后台静默更新缓存（无论是否使用了缓存，都尝试更新以保持最新）
  if (usedCache) {
    // 延迟执行后台更新，避免阻塞主流程
    setTimeout(() => {
      updateSpritesheetCacheInBackground()
    }, 3000)
  }

  // 4列x4行 (16帧)
  const frameWidth = spritesheet.width / 4
  const frameHeight = spritesheet.height / 4

  // 创建所有帧的纹理
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

  // 创建精灵并添加到舞台
  currentSprite = new PIXI.Sprite(spriteFrames[0])

  // 缩放精灵以适应画布（保持宽高比）
  const scale = Math.min(80 / frameWidth, 80 / frameHeight) * 0.9
  currentSprite.scale.set(scale)
  currentSprite.anchor.set(0.5)
  currentSprite.x = 40
  currentSprite.y = 40

  pixiApp.stage.addChild(currentSprite)

  // 启动动画循环：idle动作（帧0-3），偶尔切换到其他表情
  startIdleAnimation()
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

onMounted(async () => {
  // 优先显示图标，话术后台加载不阻塞
  await initPixiApp()
  startSpeechLoop()

  loadSpeechPhrases()
})

onUnmounted(() => {
  if (speechInterval) {
    clearInterval(speechInterval)
    speechInterval = null
  }
  if (speechTimer) {
    clearTimeout(speechTimer)
    speechTimer = null
  }
  cleanupPixiApp()
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

@halo-default: drop-shadow(0 0 8px rgba(255, 182, 193, 0.4)) drop-shadow(0 0 16px rgba(221, 160, 221, 0.2));
@halo-hover: drop-shadow(0 0 12px rgba(255, 182, 193, 0.6)) drop-shadow(0 0 24px rgba(221, 160, 221, 0.3));
@halo-active: drop-shadow(0 0 16px rgba(123, 80, 87, 0.8)) drop-shadow(0 0 32px rgba(221, 160, 221, 0.4));

@transition-bounce: cubic-bezier(0.34, 1.56, 0.64, 1);

// Mixins
.gpu-accelerate() {
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
}

.bubble-shadow(@color) {
  box-shadow: 0 4px 20px fade(@color, 60%), 0 0 0 3px rgba(255, 255, 255, 0.4), inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-shadow-hover(@color) {
  box-shadow: 0 8px 35px fade(@color, 90%), 0 0 0 4px rgba(255, 255, 255, 0.6),
    inset 0 3px 10px rgba(255, 255, 255, 0.5);
}

#toolbar-container {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  box-sizing: border-box;
  .gpu-accelerate();
  -webkit-font-smoothing: antialiased;
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
  animation: float 3s ease-in-out infinite;
  filter: @halo-default;
  transition: all 0.4s @transition-bounce;
  position: relative;
  z-index: 100;
  .gpu-accelerate();
  -webkit-font-smoothing: antialiased;

  &:hover {
    animation: pulse 1.5s ease-in-out infinite;
    filter: @halo-hover;
  }

  &.active {
    transform: scale(1.15) translateZ(0);
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
  max-width: 280px;
  padding: 8px 12px;
  color: #4b2b34;
  font-size: 12px;
  line-height: 1.4;
  background: rgba(255, 248, 252, 0.95);
  border-radius: 12px;
  pointer-events: none;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
    transform: translate3d(0, -10px, 0) scale(1);
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
