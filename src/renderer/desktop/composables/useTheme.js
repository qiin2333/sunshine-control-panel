import { ref, watch, onMounted } from 'vue'

const STORAGE_KEY = 'foundation-desktop-theme'
const WALLPAPER_KEY = 'foundation-desktop-wallpaper'
const IDB_NAME = 'foundation-desktop'
const IDB_STORE = 'assets'

// IndexedDB 辅助 —— 用于存储大体积壁纸 data URL
function openIDB() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(IDB_NAME, 1)
    req.onupgradeneeded = () => req.result.createObjectStore(IDB_STORE)
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

async function idbSet(key, value) {
  const db = await openIDB()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(IDB_STORE, 'readwrite')
    tx.objectStore(IDB_STORE).put(value, key)
    tx.oncomplete = () => { db.close(); resolve() }
    tx.onerror = () => { db.close(); reject(tx.error) }
  })
}

async function idbGet(key) {
  const db = await openIDB()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(IDB_STORE, 'readonly')
    const req = tx.objectStore(IDB_STORE).get(key)
    req.onsuccess = () => { db.close(); resolve(req.result) }
    req.onerror = () => { db.close(); reject(req.error) }
  })
}

async function idbDelete(key) {
  const db = await openIDB()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(IDB_STORE, 'readwrite')
    tx.objectStore(IDB_STORE).delete(key)
    tx.oncomplete = () => { db.close(); resolve() }
    tx.onerror = () => { db.close(); reject(tx.error) }
  })
}

// 预设主题
const presets = {
  cyberpunk: {
    label: '赛博朋克',
    vars: {
      '--fd-accent': '#00fff5',
      '--fd-accent-rgb': '0, 255, 245',
      '--fd-accent-secondary': '#ff00ff',
      '--fd-accent-secondary-rgb': '255, 0, 255',
      '--fd-bg-primary': '#0f0f23',
      '--fd-bg-secondary': '#1a1a2e',
      '--fd-bg-tertiary': '#16213e',
      '--fd-text-primary': '#ffffff',
      '--fd-text-primary-rgb': '255, 255, 255',
      '--fd-text-secondary': 'rgba(255,255,255,0.7)',
      '--fd-text-muted': 'rgba(255,255,255,0.4)',
      '--fd-status-success': '#00ff88',
      '--fd-status-success-rgb': '0, 255, 136',
      '--fd-status-warning': '#ffd700',
      '--fd-status-warning-rgb': '255, 215, 0',
      '--fd-status-danger': '#ff6b35',
      '--fd-status-danger-rgb': '255, 107, 53',
      '--fd-card-radius': '20px',
      '--fd-font-size': '14px',
      '--fd-grid-visible': '1',
      '--fd-scanline-visible': '1',
    },
  },
  midnight: {
    label: '午夜蓝',
    vars: {
      '--fd-accent': '#6c9eff',
      '--fd-accent-rgb': '108, 158, 255',
      '--fd-accent-secondary': '#a78bfa',
      '--fd-accent-secondary-rgb': '167, 139, 250',
      '--fd-bg-primary': '#0b1120',
      '--fd-bg-secondary': '#111827',
      '--fd-bg-tertiary': '#1e293b',
      '--fd-text-primary': '#f1f5f9',
      '--fd-text-primary-rgb': '241, 245, 249',
      '--fd-text-secondary': 'rgba(241,245,249,0.65)',
      '--fd-text-muted': 'rgba(241,245,249,0.35)',
      '--fd-status-success': '#34d399',
      '--fd-status-success-rgb': '52, 211, 153',
      '--fd-status-warning': '#fbbf24',
      '--fd-status-warning-rgb': '251, 191, 36',
      '--fd-status-danger': '#f87171',
      '--fd-status-danger-rgb': '248, 113, 113',
      '--fd-card-radius': '16px',
      '--fd-font-size': '14px',
      '--fd-grid-visible': '0',
      '--fd-scanline-visible': '0',
    },
  },
  emerald: {
    label: '翡翠绿',
    vars: {
      '--fd-accent': '#10b981',
      '--fd-accent-rgb': '16, 185, 129',
      '--fd-accent-secondary': '#06b6d4',
      '--fd-accent-secondary-rgb': '6, 182, 212',
      '--fd-bg-primary': '#0c1a14',
      '--fd-bg-secondary': '#132a1f',
      '--fd-bg-tertiary': '#1a3a2a',
      '--fd-text-primary': '#ecfdf5',
      '--fd-text-primary-rgb': '236, 253, 245',
      '--fd-text-secondary': 'rgba(236,253,245,0.7)',
      '--fd-text-muted': 'rgba(236,253,245,0.4)',
      '--fd-status-success': '#34d399',
      '--fd-status-success-rgb': '52, 211, 153',
      '--fd-status-warning': '#fbbf24',
      '--fd-status-warning-rgb': '251, 191, 36',
      '--fd-status-danger': '#f87171',
      '--fd-status-danger-rgb': '248, 113, 113',
      '--fd-card-radius': '16px',
      '--fd-font-size': '14px',
      '--fd-grid-visible': '1',
      '--fd-scanline-visible': '0',
    },
  },
  rose: {
    label: '玫瑰金',
    vars: {
      '--fd-accent': '#f472b6',
      '--fd-accent-rgb': '244, 114, 182',
      '--fd-accent-secondary': '#fb923c',
      '--fd-accent-secondary-rgb': '251, 146, 60',
      '--fd-bg-primary': '#1a0a12',
      '--fd-bg-secondary': '#2a1520',
      '--fd-bg-tertiary': '#3a2030',
      '--fd-text-primary': '#fdf2f8',
      '--fd-text-primary-rgb': '253, 242, 248',
      '--fd-text-secondary': 'rgba(253,242,248,0.7)',
      '--fd-text-muted': 'rgba(253,242,248,0.4)',
      '--fd-status-success': '#86efac',
      '--fd-status-success-rgb': '134, 239, 172',
      '--fd-status-warning': '#fde68a',
      '--fd-status-warning-rgb': '253, 230, 138',
      '--fd-status-danger': '#fca5a5',
      '--fd-status-danger-rgb': '252, 165, 165',
      '--fd-card-radius': '24px',
      '--fd-font-size': '14px',
      '--fd-grid-visible': '0',
      '--fd-scanline-visible': '0',
    },
  },
  steam: {
    label: 'Steam 经典',
    vars: {
      '--fd-accent': '#66c0f4',
      '--fd-accent-rgb': '102, 192, 244',
      '--fd-accent-secondary': '#4fc3f7',
      '--fd-accent-secondary-rgb': '79, 195, 247',
      '--fd-bg-primary': '#1b2838',
      '--fd-bg-secondary': '#2a475e',
      '--fd-bg-tertiary': '#171a21',
      '--fd-text-primary': '#c7d5e0',
      '--fd-text-primary-rgb': '199, 213, 224',
      '--fd-text-secondary': 'rgba(199,213,224,0.7)',
      '--fd-text-muted': 'rgba(199,213,224,0.4)',
      '--fd-status-success': '#a3e635',
      '--fd-status-success-rgb': '163, 230, 53',
      '--fd-status-warning': '#facc15',
      '--fd-status-warning-rgb': '250, 204, 21',
      '--fd-status-danger': '#ef4444',
      '--fd-status-danger-rgb': '239, 68, 68',
      '--fd-card-radius': '4px',
      '--fd-font-size': '14px',
      '--fd-grid-visible': '0',
      '--fd-scanline-visible': '0',
    },
  },
}

const defaultPreset = 'cyberpunk'

// 获取默认变量
function getDefaultVars() {
  return { ...presets[defaultPreset].vars }
}

// 从 hex 解析 RGB
function hexToRgb(hex) {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `${r}, ${g}, ${b}`
}

// RGB → hex
function rgbToHex(r, g, b) {
  return '#' + [r, g, b].map(c => Math.round(c).toString(16).padStart(2, '0')).join('')
}

// RGB → HSL
function rgbToHsl(r, g, b) {
  r /= 255; g /= 255; b /= 255
  const max = Math.max(r, g, b), min = Math.min(r, g, b)
  let h, s, l = (max + min) / 2
  if (max === min) { h = s = 0 }
  else {
    const d = max - min
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
    switch (max) {
      case r: h = ((g - b) / d + (g < b ? 6 : 0)) / 6; break
      case g: h = ((b - r) / d + 2) / 6; break
      case b: h = ((r - g) / d + 4) / 6; break
    }
  }
  return [h * 360, s * 100, l * 100]
}

// 颜色距离 (简单欧几里得)
function colorDistance(a, b) {
  return Math.sqrt((a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2)
}

// K-means 聚类提取主色调
function extractColors(imageData, k = 5) {
  const pixels = []
  const data = imageData.data
  // 采样 (最多 5000 个像素)
  const step = Math.max(1, Math.floor(data.length / 4 / 5000))
  for (let i = 0; i < data.length; i += 4 * step) {
    const r = data[i], g = data[i + 1], b = data[i + 2], a = data[i + 3]
    if (a < 128) continue // 跳过透明像素
    pixels.push([r, g, b])
  }
  if (pixels.length < k) return pixels

  // 初始化中心点 (k-means++)
  const centers = [pixels[Math.floor(Math.random() * pixels.length)]]
  for (let c = 1; c < k; c++) {
    const dists = pixels.map(p => Math.min(...centers.map(ct => colorDistance(p, ct))))
    const total = dists.reduce((a, b) => a + b, 0)
    let r = Math.random() * total
    for (let i = 0; i < dists.length; i++) {
      r -= dists[i]
      if (r <= 0) { centers.push(pixels[i]); break }
    }
  }

  // 迭代 (最多 10 次)
  for (let iter = 0; iter < 10; iter++) {
    const clusters = Array.from({ length: k }, () => [])
    for (const p of pixels) {
      let minDist = Infinity, minIdx = 0
      for (let i = 0; i < k; i++) {
        const d = colorDistance(p, centers[i])
        if (d < minDist) { minDist = d; minIdx = i }
      }
      clusters[minIdx].push(p)
    }
    let changed = false
    for (let i = 0; i < k; i++) {
      if (clusters[i].length === 0) continue
      const avg = [0, 1, 2].map(ch => clusters[i].reduce((s, p) => s + p[ch], 0) / clusters[i].length)
      if (colorDistance(avg, centers[i]) > 1) changed = true
      centers[i] = avg
    }
    if (!changed) break
  }

  // 按聚类大小排序
  return centers.sort((a, b) => {
    const [, sa, la] = rgbToHsl(...a)
    const [, sb, lb] = rgbToHsl(...b)
    return (sb * lb) - (sa * la)
  })
}

// 从提取的颜色生成主题变量
function colorsToTheme(colors) {
  if (!colors.length) return {}

  // 按饱和度×亮度排序找到最鲜明的颜色
  const ranked = colors.map(c => {
    const [h, s, l] = rgbToHsl(...c)
    return { c, h, s, l, score: s * (l > 15 && l < 85 ? 1 : 0.3) }
  }).sort((a, b) => b.score - a.score)

  // 找最暗的颜色作背景
  const darkest = [...colors].sort((a, b) => {
    const la = rgbToHsl(...a)[2], lb = rgbToHsl(...b)[2]
    return la - lb
  })

  const accent = ranked[0]?.c || colors[0]
  const secondary = ranked[1]?.c || ranked[0]?.c || colors[0]
  const bgBase = darkest[0]

  // 生成暗色背景变体
  const bgPrimary = bgBase.map(c => Math.max(0, Math.round(c * 0.15)))
  const bgSecondary = bgBase.map(c => Math.max(0, Math.round(c * 0.25)))
  const bgTertiary = bgBase.map(c => Math.max(0, Math.round(c * 0.35)))

  // 根据背景亮度决定文字颜色
  const bgLightness = rgbToHsl(...bgPrimary)[2]
  const textPrimary = bgLightness > 50 ? [30, 30, 30] : [255, 255, 255]
  const textPrimaryHex = rgbToHex(...textPrimary)

  // 根据主色调生成 status 颜色
  const accentHsl = rgbToHsl(...accent)
  const successHue = (accentHsl[0] + 120) % 360
  const warningHue = 45
  const dangerHue = 0

  const hslToApproxRgb = (h, s, l) => {
    s /= 100; l /= 100
    const a = s * Math.min(l, 1 - l)
    const f = n => {
      const k = (n + h / 30) % 12
      return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))
    }
    return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)]
  }

  const success = hslToApproxRgb(successHue, 70, 55)
  const warning = hslToApproxRgb(warningHue, 80, 55)
  const danger = hslToApproxRgb(dangerHue, 75, 55)

  return {
    '--fd-accent': rgbToHex(...accent),
    '--fd-accent-rgb': accent.map(Math.round).join(', '),
    '--fd-accent-secondary': rgbToHex(...secondary),
    '--fd-accent-secondary-rgb': secondary.map(Math.round).join(', '),
    '--fd-bg-primary': rgbToHex(...bgPrimary),
    '--fd-bg-secondary': rgbToHex(...bgSecondary),
    '--fd-bg-tertiary': rgbToHex(...bgTertiary),
    '--fd-text-primary': textPrimaryHex,
    '--fd-text-primary-rgb': textPrimary.join(', '),
    '--fd-text-secondary': `rgba(${textPrimary.join(',')},0.7)`,
    '--fd-text-muted': `rgba(${textPrimary.join(',')},0.4)`,
    '--fd-status-success': rgbToHex(...success),
    '--fd-status-success-rgb': success.join(', '),
    '--fd-status-warning': rgbToHex(...warning),
    '--fd-status-warning-rgb': warning.join(', '),
    '--fd-status-danger': rgbToHex(...danger),
    '--fd-status-danger-rgb': danger.join(', '),
    '--fd-grid-visible': '0',
    '--fd-scanline-visible': '0',
  }
}

// 将图片文件加载为 Image 元素
function loadImage(file) {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file)
    const img = new Image()
    img.onload = () => { URL.revokeObjectURL(url); resolve(img) }
    img.onerror = () => { URL.revokeObjectURL(url); reject(new Error('Failed to load image')) }
    img.src = url
  })
}

// 从图片文件提取颜色并生成主题
async function analyzeWallpaper(file) {
  const img = await loadImage(file)

  // 小画布取色
  const thumbCanvas = document.createElement('canvas')
  const thumbScale = Math.min(1, 200 / Math.max(img.width, img.height))
  thumbCanvas.width = Math.round(img.width * thumbScale)
  thumbCanvas.height = Math.round(img.height * thumbScale)
  const thumbCtx = thumbCanvas.getContext('2d')
  thumbCtx.drawImage(img, 0, 0, thumbCanvas.width, thumbCanvas.height)
  const imageData = thumbCtx.getImageData(0, 0, thumbCanvas.width, thumbCanvas.height)
  const colors = extractColors(imageData, 6)
  const theme = colorsToTheme(colors)

  // 大画布生成显示用壁纸（限制 1920px，JPEG 质量 0.75）
  const displayCanvas = document.createElement('canvas')
  const maxDim = 1920
  const displayScale = Math.min(1, maxDim / Math.max(img.width, img.height))
  displayCanvas.width = Math.round(img.width * displayScale)
  displayCanvas.height = Math.round(img.height * displayScale)
  const displayCtx = displayCanvas.getContext('2d')
  displayCtx.drawImage(img, 0, 0, displayCanvas.width, displayCanvas.height)
  const dataUrl = displayCanvas.toDataURL('image/jpeg', 0.75)

  return { dataUrl, theme, colors }
}

export function useTheme() {
  const themeVars = ref(getDefaultVars())
  const activePreset = ref(defaultPreset)
  const editorOpen = ref(false)
  const wallpaper = ref(null) // data URL
  const wallpaperColors = ref([]) // 提取的主色调 [[r,g,b], ...]

  // 应用 CSS 变量到 document
  function applyTheme(vars) {
    const el = document.documentElement
    for (const [key, value] of Object.entries(vars)) {
      el.style.setProperty(key, value)
      // 自动为 hex 颜色生成 -rgb 变体（跳过已有的 -rgb 键）
      if (typeof value === 'string' && value.startsWith('#') && !key.endsWith('-rgb')) {
        el.style.setProperty(key + '-rgb', hexToRgb(value))
      }
    }
  }

  // 设置单个变量
  function setVar(key, value) {
    themeVars.value[key] = value
    // 自动为 hex 颜色同步设置 -rgb 变体
    if (typeof value === 'string' && value.startsWith('#') && !key.endsWith('-rgb')) {
      themeVars.value[key + '-rgb'] = hexToRgb(value)
    }
    activePreset.value = 'custom'
  }

  // 应用预设
  function applyPreset(name) {
    if (presets[name]) {
      themeVars.value = { ...presets[name].vars }
      activePreset.value = name
    }
  }

  // 导出主题 JSON
  function exportTheme() {
    return JSON.stringify({ preset: activePreset.value, vars: themeVars.value }, null, 2)
  }

  // 导入主题 JSON
  function importTheme(json) {
    try {
      const data = JSON.parse(json)
      if (data.vars) {
        themeVars.value = { ...getDefaultVars(), ...data.vars }
        activePreset.value = data.preset || 'custom'
      }
    } catch (e) {
      console.error('Invalid theme JSON:', e)
    }
  }

  // 设置壁纸并从中提取主题
  async function setWallpaper(file) {
    try {
      const result = await analyzeWallpaper(file)
      wallpaper.value = result.dataUrl
      wallpaperColors.value = result.colors
      // 应用提取的主题 + 保留当前的圆角/字体等外观设置
      const currentAppearance = {
        '--fd-card-radius': themeVars.value['--fd-card-radius'],
        '--fd-font-size': themeVars.value['--fd-font-size'],
      }
      themeVars.value = { ...getDefaultVars(), ...result.theme, ...currentAppearance }
      activePreset.value = 'wallpaper'
      // 持久化壁纸到 IndexedDB（支持大体积图片）
      idbSet(WALLPAPER_KEY, result.dataUrl).catch(e =>
        console.warn('Failed to persist wallpaper:', e)
      )
      applyWallpaperBg(result.dataUrl)
    } catch (e) {
      console.error('Failed to analyze wallpaper:', e)
    }
  }

  // 移除壁纸
  function removeWallpaper() {
    wallpaper.value = null
    wallpaperColors.value = []
    idbDelete(WALLPAPER_KEY).catch(() => {})
    localStorage.removeItem(WALLPAPER_KEY) // 清理旧数据
    applyWallpaperBg(null)
  }

  // 应用壁纸背景到 DOM
  function applyWallpaperBg(dataUrl) {
    const el = document.querySelector('.desktop-window')
    if (!el) return
    if (dataUrl) {
      el.style.setProperty('--fd-wallpaper', `url("${dataUrl}")`)
    } else {
      el.style.removeProperty('--fd-wallpaper')
    }
  }

  // 持久化 + 应用（CSS 变量立即更新，localStorage 写入防抖）
  let saveTimer = null
  watch(themeVars, (vars) => {
    applyTheme(vars)
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ preset: activePreset.value, vars }))
    }, 300)
  }, { deep: true })

  // 初始化：读取持久化数据
  onMounted(() => {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved) {
      try {
        const data = JSON.parse(saved)
        if (data.vars) {
          themeVars.value = { ...getDefaultVars(), ...data.vars }
          activePreset.value = data.preset || 'custom'
        }
      } catch (e) {
        // fallback to default
      }
    }
    applyTheme(themeVars.value)
    // 恢复壁纸（优先 IndexedDB，兼容旧 localStorage）
    idbGet(WALLPAPER_KEY).then(savedWp => {
      if (!savedWp) savedWp = localStorage.getItem(WALLPAPER_KEY)
      if (savedWp) {
        wallpaper.value = savedWp
        applyWallpaperBg(savedWp)
      }
    }).catch(() => {})
  })

  return {
    themeVars,
    activePreset,
    editorOpen,
    wallpaper,
    wallpaperColors,
    presets,
    setVar,
    applyPreset,
    exportTheme,
    importTheme,
    setWallpaper,
    removeWallpaper,
  }
}
