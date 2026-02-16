<template>
  <div id="toolbar-container" @click.self="handleOutsideClick"
       @pointerdown.self="onDragStart">
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
import { ref, onUnmounted, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { PhysicalPosition } from '@tauri-apps/api/dpi'
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
let animationTimer = null

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
  // 后续固定间隔（启动时随机选择 15s~35s）
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

  try {
    await invoke('handle_toolbar_menu_action', { action })
  } catch (error) {
    console.error('菜单操作失败:', error)
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
let hasMoved = false            // 是否实际产生了移动（区分点击和拖拽）
let dragPointerType = ''        // 发起拖拽的指针类型（mouse/pen/touch）
let dragStartScreenX = 0        // 按下时的屏幕坐标（用于鼠标阈值检测）
let dragStartScreenY = 0
// 触摸拖拽专用变量
// 核心思路：WebView2 触摸的 screenX ≈ clientX（视口相对坐标，非屏幕绝对坐标）
// 移动窗口会导致 screenX 偏移→反馈震荡，因此用 clientX + 已知窗口位置 + 串行 await setPosition
let touchStartClientX = 0       // 触摸开始时的 clientX（抓取偏移量）
let touchStartClientY = 0
let touchBaseLogX = NaN         // 当前已确认的窗口逻辑位置（setPosition 完成后更新）
let touchBaseLogY = NaN
let touchPendingLogX = 0        // 待应用的逻辑坐标
let touchPendingLogY = 0
let touchIsSettingPos = false   // setPosition 正在执行中，跳过中间事件
let touchRafId = null           // requestAnimationFrame ID
const DRAG_THRESHOLD = 3        // 移动超过 3px 才算拖拽

// 移除所有拖拽相关事件监听（复用于多处清理）
const removeDragListeners = () => {
  document.removeEventListener('pointermove', onDragMove)
  document.removeEventListener('pointerup', onDragEnd)
  document.removeEventListener('pointercancel', onDragEnd)
}

const onDragStart = (e) => {
  if (e.button !== 0) return
  e.preventDefault()
  isDragging = true
  hasMoved = false
  dragPointerType = e.pointerType
  dragStartScreenX = e.screenX
  dragStartScreenY = e.screenY
  
  document.addEventListener('pointermove', onDragMove)
  document.addEventListener('pointerup', onDragEnd)
  document.addEventListener('pointercancel', onDragEnd)
  
  if (e.pointerType === 'touch') {
    touchStartClientX = e.clientX
    touchStartClientY = e.clientY
    touchBaseLogX = NaN
    touchBaseLogY = NaN
    touchIsSettingPos = false
    const dpr = window.devicePixelRatio || 1
    appWindow.outerPosition().then((pos) => {
      touchBaseLogX = pos.x / dpr
      touchBaseLogY = pos.y / dpr
    }).catch(() => { isDragging = false })
  }
}

// 触摸拖拽：rAF 回调，串行 await setPosition 避免竞态
const touchApplyPosition = async () => {
  touchRafId = null
  if (!isDragging || touchIsSettingPos) return
  touchIsSettingPos = true
  const dpr = window.devicePixelRatio || 1
  const physX = Math.round(touchPendingLogX * dpr)
  const physY = Math.round(touchPendingLogY * dpr)
  try {
    await appWindow.setPosition(new PhysicalPosition(physX, physY))
    // 更新 baseline（用物理→逻辑避免累积舍入误差）
    touchBaseLogX = physX / dpr
    touchBaseLogY = physY / dpr
  } catch {} finally {
    touchIsSettingPos = false
  }
}

const onDragMove = (e) => {
  if (!isDragging) return
  // 过滤非同类型指针（避免 Windows 触摸模拟鼠标事件）
  if (e.pointerType !== dragPointerType) return
  
  if (dragPointerType === 'mouse' || dragPointerType === 'pen') {
    const dx = e.screenX - dragStartScreenX
    const dy = e.screenY - dragStartScreenY
    if (!hasMoved && Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return
    hasMoved = true
    e.preventDefault()
    // 超过阈值 → 切换为 OS 原生拖拽（完美处理跨 DPI 显示器）
    isDragging = false
    removeDragListeners()
    appWindow.startDragging().then(async () => {
      try {
        const finalPos = await appWindow.outerPosition()
        await invoke('save_toolbar_position', { x: finalPos.x, y: finalPos.y })
      } catch {}
    }).catch(() => {})
    return
  }
  
  // === 触摸拖拽 ===
  if (Number.isNaN(touchBaseLogX) || touchIsSettingPos) return
  
  const dcx = e.clientX - touchStartClientX
  const dcy = e.clientY - touchStartClientY
  if (!hasMoved && Math.abs(dcx) < DRAG_THRESHOLD && Math.abs(dcy) < DRAG_THRESHOLD) return
  hasMoved = true
  e.preventDefault()
  
  // 目标 = 已确认位置 + clientX delta（详见 docs/toolbar_interaction.md）
  touchPendingLogX = touchBaseLogX + dcx
  touchPendingLogY = touchBaseLogY + dcy
  if (!touchRafId) {
    touchRafId = requestAnimationFrame(touchApplyPosition)
  }
}

const onDragEnd = async (e) => {
  const wasDragged = isDragging && hasMoved
  isDragging = false
  if (touchRafId) {
    cancelAnimationFrame(touchRafId)
    touchRafId = null
  }
  removeDragListeners()
  
  if (wasDragged && dragPointerType === 'touch') {
    // 等待 flight 中的 setPosition 完成
    while (touchIsSettingPos) {
      await new Promise(r => setTimeout(r, 10))
    }
    try {
      const dpr = window.devicePixelRatio || 1
      await appWindow.setPosition(new PhysicalPosition(
        Math.round(touchPendingLogX * dpr),
        Math.round(touchPendingLogY * dpr)
      ))
      const finalPos = await appWindow.outerPosition()
      await invoke('save_toolbar_position', { x: finalPos.x, y: finalPos.y })
    } catch {}
  }
  requestAnimationFrame(() => { hasMoved = false })
}

// 点击切换菜单（鼠标 click + 触摸合成 click 都走这里）
const onIconClick = () => {
  if (!hasMoved) toggleMenu()
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
})

onUnmounted(() => {
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
  removeDragListeners()
  if (touchRafId) { cancelAnimationFrame(touchRafId); touchRafId = null }
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
  touch-action: none;  // 阻止浏览器默认触摸手势（滚动/缩放），确保 touchmove 可用
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
