<template>
  <div id="toolbar-container" :class="{ dragging: isDragActive }" @click.self="handleOutsideClick">
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

    <!-- 中心工具栏图标 -->
    <div
      class="toolbar-icon"
      :class="{ active: menuVisible }"
      @mousedown="handleMouseDown"
      @mouseup="handleMouseUp"
      @touchstart.prevent="handleMouseDown"
      @touchend="handleMouseUp"
      @click.stop="toggleMenu"
      @contextmenu.prevent="toggleMenu"
    >
      <!-- PixiJS Canvas 容器 -->
      <canvas ref="pixiCanvas" class="icon-canvas"></canvas>
      <transition name="speech">
        <div v-if="speechVisible" class="speech-bubble" role="status" aria-live="polite">
          {{ speechText }}
        </div>
      </transition>
    </div>
  </div>
</template>

<script setup>
import { ref, onUnmounted, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import * as PIXI from 'pixi.js'

const menuVisible = ref(false)
const isDragActive = ref(false)
const DRAG_THRESHOLD_SQ = 9 // 3px 的平方
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
  'https://hub.gitmirror.com/raw.githubusercontent.com/qiin2333/qiin.github.io/assets/img/toolbar-spritesheet.png?t=' +
  Date.now()

// 精灵图集别名，便于使用 PixiJS 资源缓存
const SPRITESHEET_ALIAS = 'toolbar-spritesheet'

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

// 通过后端代理加载话术
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

// 立即加载话术
loadSpeechPhrases()

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
    // 避免拖动或菜单展开时打断交互
    if (!isDragActive.value && !menuVisible.value) {
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
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M3 17v2h6v-2H3zM3 5v2h10V5H3zm10 16v-2h8v-2h-8v-2h-2v6h2zM7 9v2H3v2h4v2h2V9H7zm14 4v-2H11v2h10zm-6-4h2V7h4V5h-4V3h-2v6z"/></svg>',
  },
  {
    id: 'bitrate',
    label: '码率调整',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14zm2.5-4h-2v2H9v-2H7V9h2V7h1v2h2v1z"/></svg>',
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

let isDragging = false
let startX = 0
let startY = 0
let mouseMoveHandler = null
let mouseUpHandler = null

const bindTempDragListeners = () => {
  window.addEventListener('mousemove', mouseMoveHandler)
  window.addEventListener('mouseup', mouseUpHandler)
  window.addEventListener('touchmove', mouseMoveHandler, { passive: false })
  window.addEventListener('touchend', mouseUpHandler)
}

const unbindTempDragListeners = () => {
  if (mouseMoveHandler) {
    window.removeEventListener('mousemove', mouseMoveHandler)
    window.removeEventListener('touchmove', mouseMoveHandler)
  }
  if (mouseUpHandler) {
    window.removeEventListener('mouseup', mouseUpHandler)
    window.removeEventListener('touchend', mouseUpHandler)
  }
}

// 统一获取坐标（支持鼠标和触摸）
const getEventCoords = (e) => {
  if (e.touches && e.touches.length > 0) {
    return { x: e.touches[0].clientX, y: e.touches[0].clientY }
  }
  return { x: e.clientX, y: e.clientY }
}

const handleMouseDown = (e) => {
  // 右键不处理
  if (e.button === 2) {
    return
  }

  // 左键或触摸
  if (e.button === 0 || e.type === 'touchstart') {
    // 使用位移阈值触发拖动，避免动画导致的偏移
    const coords = getEventCoords(e)
    startX = coords.x
    startY = coords.y
    isDragging = false

    mouseMoveHandler = (ev) => {
      if (isDragging) return

      // 阻止触摸滚动
      if (ev.type === 'touchmove') {
        ev.preventDefault()
      }

      const coords = getEventCoords(ev)
      const dx = coords.x - startX
      const dy = coords.y - startY
      if (dx * dx + dy * dy > DRAG_THRESHOLD_SQ) {
        // 超过阈值判定为拖动
        isDragging = true
        isDragActive.value = true
        const window = getCurrentWindow()
        window.startDragging().finally(() => {
          isDragActive.value = false
        })
      }
    }

    mouseUpHandler = () => {
      unbindTempDragListeners()
      mouseMoveHandler = null
      mouseUpHandler = null
      // 延后一帧复位，避免与点击冲突
      setTimeout(() => {
        isDragging = false
      }, 0)
    }

    bindTempDragListeners()
  }
}

const handleMouseUp = () => {
  unbindTempDragListeners()
  mouseMoveHandler = null
  mouseUpHandler = null
}

const toggleMenu = (e) => {
  if (e) {
    e.preventDefault()
    e.stopPropagation()
  }

  // 如果正在拖动，不切换菜单
  if (isDragging) {
    isDragging = false
    return
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

  if (!PIXI.Assets.resolver.hasKey(SPRITESHEET_ALIAS)) {
    PIXI.Assets.add({ alias: SPRITESHEET_ALIAS, src: SPRITESHEET_URL })
  }

  const spritesheet = PIXI.Assets.cache.has(SPRITESHEET_ALIAS)
    ? PIXI.Assets.cache.get(SPRITESHEET_ALIAS)
    : await PIXI.Assets.load(SPRITESHEET_ALIAS)

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
  await initPixiApp()
  startSpeechLoop()
})

onUnmounted(() => {
  // 位置保存已在 Rust 后端窗口关闭事件中处理
  
  unbindTempDragListeners()
  mouseMoveHandler = null
  mouseUpHandler = null
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
@halo-default: drop-shadow(0 0 8px rgba(255, 182, 193, 0.4)) drop-shadow(0 0 16px rgba(221, 160, 221, 0.2));
@halo-hover: drop-shadow(0 0 12px rgba(255, 182, 193, 0.6)) drop-shadow(0 0 24px rgba(221, 160, 221, 0.3));
@halo-active: drop-shadow(0 0 16px rgba(123, 80, 87, 0.8)) drop-shadow(0 0 32px rgba(221, 160, 221, 0.4));
#toolbar-container {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  box-sizing: border-box;
  transform: translateZ(0);
  -webkit-font-smoothing: antialiased;
}

/* 拖动进行时，关闭图标的浮动/缩放动画，避免视觉与鼠标偏移 */
#toolbar-container.dragging .toolbar-icon {
  animation: none !important;
  transform: none !important;
}

#toolbar-container.dragging .toolbar-icon:hover {
  animation: none !important;
}

/* 气泡菜单容器 */
.bubble-menu {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 50;
  will-change: transform;
  transform: translateZ(0);
}

/* 气泡包裹层（负责定位） */
.bubble-wrapper {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 48px;
  height: 48px;
  margin-left: -24px;
  margin-top: -24px;
  pointer-events: all;
  will-change: transform, margin-top;
  transform: translateZ(0);
  backface-visibility: hidden;
}

/* 气泡项（负责入场动画和样式） */
.bubble-item {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: linear-gradient(
    135deg,
    rgba(255, 182, 193, 0.95) 0%,
    /* 粉色 */ rgba(255, 160, 220, 0.95) 50%,
    /* 淡紫粉 */ rgba(186, 148, 255, 0.95) 100% /* 淡紫色 */
  );
  backdrop-filter: blur(15px);
  box-shadow: 0 4px 20px rgba(255, 182, 193, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  animation: bubbleIn 0.6s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  position: relative;
  will-change: transform, opacity, box-shadow;
  transform: translateZ(0);
  backface-visibility: hidden;
  -webkit-font-smoothing: antialiased;
}

/* 为每个气泡添加不同的可爱颜色 */
.bubble-wrapper:nth-child(1) .bubble-item {
  background: linear-gradient(135deg, rgba(255, 182, 193, 0.95) 0%, rgba(255, 192, 203, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(255, 182, 193, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-wrapper:nth-child(2) .bubble-item {
  background: linear-gradient(135deg, rgba(173, 216, 230, 0.95) 0%, rgba(135, 206, 250, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(173, 216, 230, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-wrapper:nth-child(3) .bubble-item {
  background: linear-gradient(135deg, rgba(221, 160, 221, 0.95) 0%, rgba(218, 112, 214, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(221, 160, 221, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-wrapper:nth-child(4) .bubble-item {
  background: linear-gradient(135deg, rgba(255, 193, 7, 0.95) 0%, rgba(255, 152, 0, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(255, 193, 7, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-item:hover {
  box-shadow: 0 8px 35px rgba(255, 182, 193, 0.9), 0 0 0 4px rgba(255, 255, 255, 0.6),
    inset 0 3px 10px rgba(255, 255, 255, 0.5);
  z-index: 10;
  transform: scale(1.1) translateZ(0);
}

.bubble-item:hover .bubble-icon {
  transform: scale(1.2) rotate(15deg);
}

.bubble-item.danger {
  background: linear-gradient(135deg, rgba(255, 182, 193, 0.95) 0%, rgba(255, 150, 150, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(255, 150, 150, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-item.danger:hover {
  box-shadow: 0 8px 35px rgba(255, 150, 150, 0.9), 0 0 0 4px rgba(255, 255, 255, 0.6),
    inset 0 3px 10px rgba(255, 255, 255, 0.5);
}

.bubble-icon {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  will-change: transform;
  backface-visibility: hidden;
}

.bubble-icon svg {
  width: 100%;
  height: 100%;
}

/* 中心工具栏图标 */
.toolbar-icon {
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  -webkit-app-region: no-drag;
  background: transparent;
  border: none;
  padding: 0;
  margin: 0;
  border-radius: 50%;
  animation: float 3s ease-in-out infinite;
  filter: @halo-default;
  transition: all 0.4s cubic-bezier(0.34, 1.56, 0.64, 1);
  position: relative;
  z-index: 100;
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
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

.icon-image {
  width: 100%;
  height: 100%;
  object-fit: contain;
  pointer-events: none;
  display: block;
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
}

.icon-canvas {
  width: 100%;
  height: 100%;
  pointer-events: none;
  display: block;
}

/* 说话气泡样式 */
.speech-bubble {
  position: absolute;
  bottom: 100px;
  left: 50%;
  transform: translateX(-50%);
  max-width: 220px;
  padding: 8px 12px;
  color: #4b2b34;
  font-size: 12px;
  line-height: 1.4;
  background: rgba(255, 248, 252, 0.95);
  border-radius: 12px;
  box-shadow: 0 6px 18px rgba(255, 182, 193, 0.45), 0 0 0 2px rgba(255, 255, 255, 0.7) inset;
  pointer-events: none;
  white-space: normal;
  overflow-wrap: break-word;
  word-break: keep-all;
  hyphens: auto;
  overflow: hidden;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  line-clamp: 2;
  -webkit-line-clamp: 2;
  text-overflow: ellipsis;
}

.speech-bubble::after {
  content: '';
  position: absolute;
  top: 100%;
  left: 50%;
  transform: translateX(-50%);
  border-width: 6px;
  border-style: solid;
  border-color: rgba(255, 248, 252, 0.95) transparent transparent transparent;
}

/* 说话出现/消失动画 */
.speech-enter-active,
.speech-leave-active {
  transition: opacity 0.22s ease, transform 0.22s ease;
}

.speech-enter-from,
.speech-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(6px);
}

/* 气泡入场动画（带旋转，使用 3D 加速） */
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

.bubble-item .bubble-icon {
  animation: iconScale 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  animation-delay: inherit; /* 继承父元素的动画延迟 */
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
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

/* 气泡过渡动画 */
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

/* 浮动跳动动画（使用 translate3d 硬件加速） */
@keyframes float {
  0%,
  100% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  50% {
    transform: translate3d(0, -10px, 0) scale(1);
  }
}

/* 脉冲光晕动画（使用 translate3d 硬件加速） */
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
