/**
 * Material You 风格的动态取色（路线 A：@material/material-color-utilities）。
 *
 * 与旧的 k-means 方案的本质区别：壁纸不再直接提供颜色，只提供「种子」。
 * 种子经 HCT 色彩空间生成完整的色调阶梯（tonal palette），角色从阶梯上取固定
 * 档位 —— 所以任意壁纸下「文字/背景」对比度由结构保证，而不是碰运气。
 *
 * 本模块是纯函数集合，不依赖 Vue 和 DOM，可以直接在 node --test 里测。
 * （0.4.0 的 ESM 内部引用没有扩展名，node 原生 ESM 加载不了，所以用 0.2.7。）
 */

import {
  QuantizerCelebi,
  Score,
  TonalPalette,
  argbFromHex,
  hexFromArgb,
  themeFromSourceColor,
} from '@material/material-color-utilities'

/** 量化桶数。128 是 Material You 的默认值，兼顾准确度和速度。 */
const MAX_COLORS = 128
/** 提供给用户挑选的候选种子数（Android 同款逻辑是 4，多给两个）。 */
const DESIRED_CANDIDATES = 6

/**
 * Material You 状态色相。M3 没有语义化的 success/warning，这里按惯例取：
 * 绿 142（success）、黄 80（warning）、红直接用 scheme 的 error 色板。
 * 色度固定 48：在 tone 80 上足够醒目，又不至于在深背景上发飘。
 */
const SUCCESS_HUE = 142
const WARNING_HUE = 80
const STATUS_CHROMA = 48

// ===== 基础转换 =====

function rgbTripleFromArgb(argb) {
  return [(argb >> 16) & 0xff, (argb >> 8) & 0xff, argb & 0xff]
}

export function argbFromRgbTriple([r, g, b]) {
  return (((255 << 24) | (r << 16) | (g << 8) | b) >>> 0) | 0
}

function hexToRgbTriple(hex) {
  const argb = argbFromHex(hex)
  return rgbTripleFromArgb(argb)
}

/** 'r, g, b' 形式，供 rgba(var(--fd-xxx-rgb), a) 消费。 */
function rgbVar(triple) {
  return triple.map(Math.round).join(', ')
}

/**
 * 相对亮度（WCAG 定义）。用于对比度自检——M3 的档位选择已经保证对比度，
 * 这里是给测试断言用的第二道防线。
 */
export function relativeLuminance([r, g, b]) {
  const channel = (value) => {
    const s = value / 255
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

export function contrastRatio(a, b) {
  const la = relativeLuminance(a)
  const lb = relativeLuminance(b)
  const [lighter, darker] = la >= lb ? [la, lb] : [lb, la]
  return (lighter + 0.05) / (darker + 0.05)
}

// ===== 种子提取 =====

/** ImageData 的 RGBA 字节流转 ARGB int 数组，跳过透明像素。 */
export function imageDataToArgb(imageData) {
  const { data } = imageData
  const pixels = []
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < 128) continue
    pixels.push((((data[i + 3] << 24) | (data[i] << 16) | (data[i + 1] << 8) | data[i + 2]) >>> 0) | 0)
  }
  return pixels
}

/**
 * 从像素集合提取种子候选。
 *
 * QuantizerCelebi + Score 都是确定性的——同一张壁纸永远得到同一个种子，
 * 修掉了旧 k-means 用 Math.random() 初始化导致「重选同一张壁纸主题会变」的问题。
 *
 * @returns {{ seed: number, candidates: number[] }} candidates 按推荐度排序
 */
export function extractSeedCandidates(pixels) {
  if (!pixels.length) {
    return { seed: argbFromHex('#00fff5'), candidates: [] }
  }
  const quantized = QuantizerCelebi.quantize(pixels, MAX_COLORS)
  const candidates = Score.score(quantized, DESIRED_CANDIDATES)
  if (!candidates.length) {
    return { seed: argbFromHex('#00fff5'), candidates: [] }
  }
  return { seed: candidates[0], candidates }
}

// ===== 种子 → 主题变量 =====

/**
 * 把 M3 dark scheme 映射到 shell 的 --fd-* 变量。
 *
 * 背景三层不直接用 scheme 角色（0.2.7 没有 surfaceContainer 系），而是从
 * palettes.neutral 的 tone 6/10/14 取——这正是 M3 surface 递进的官方档位，
 * 保证三层是同一色相的等差抬升，而不是各自独立的颜色。
 */
export function m3ThemeFromSeed(seedArgb) {
  const theme = themeFromSourceColor(seedArgb)
  const dark = theme.schemes.dark
  const neutral = theme.palettes.neutral

  const success = TonalPalette.fromHueAndChroma(SUCCESS_HUE, STATUS_CHROMA).tone(80)
  const warning = TonalPalette.fromHueAndChroma(WARNING_HUE, STATUS_CHROMA).tone(80)
  const danger = theme.palettes.error.tone(80)

  const accent = hexToRgbTriple(hexFromArgb(dark.primary))
  const accentSecondary = hexToRgbTriple(hexFromArgb(dark.tertiary))
  const bgPrimary = hexToRgbTriple(hexFromArgb(neutral.tone(6)))
  const bgSecondary = hexToRgbTriple(hexFromArgb(neutral.tone(10)))
  const bgTertiary = hexToRgbTriple(hexFromArgb(neutral.tone(14)))
  const textPrimary = hexToRgbTriple(hexFromArgb(dark.onSurface))
  const textSecondary = hexToRgbTriple(hexFromArgb(dark.onSurfaceVariant))
  const textMuted = hexToRgbTriple(hexFromArgb(dark.outline))
  const statusSuccess = rgbTripleFromArgb(success)
  const statusWarning = rgbTripleFromArgb(warning)
  const statusDanger = rgbTripleFromArgb(danger)

  return {
    '--fd-accent': `#${hexFromArgb(dark.primary).slice(1)}`,
    '--fd-accent-rgb': rgbVar(accent),
    '--fd-accent-secondary': `#${hexFromArgb(dark.tertiary).slice(1)}`,
    '--fd-accent-secondary-rgb': rgbVar(accentSecondary),
    '--fd-bg-primary': `#${hexFromArgb(neutral.tone(6)).slice(1)}`,
    '--fd-bg-primary-rgb': rgbVar(bgPrimary),
    '--fd-bg-secondary': `#${hexFromArgb(neutral.tone(10)).slice(1)}`,
    '--fd-bg-secondary-rgb': rgbVar(bgSecondary),
    '--fd-bg-tertiary': `#${hexFromArgb(neutral.tone(14)).slice(1)}`,
    '--fd-text-primary': `#${hexFromArgb(dark.onSurface).slice(1)}`,
    '--fd-text-primary-rgb': rgbVar(textPrimary),
    '--fd-text-secondary': `rgba(${rgbVar(textSecondary)},0.7)`,
    '--fd-text-muted': `rgba(${rgbVar(textMuted)},0.4)`,
    '--fd-status-success': `#${hexFromArgb(success).slice(1)}`,
    '--fd-status-success-rgb': rgbVar(statusSuccess),
    '--fd-status-warning': `#${hexFromArgb(warning).slice(1)}`,
    '--fd-status-warning-rgb': rgbVar(statusWarning),
    '--fd-status-danger': `#${hexFromArgb(danger).slice(1)}`,
    '--fd-status-danger-rgb': rgbVar(statusDanger),
    '--fd-grid-visible': '0',
    '--fd-scanline-visible': '0',
  }
}

/** 候选 ARGB 列表 → ThemeEditor 色板需要的 [[r,g,b], ...]。 */
export function candidatesToSwatches(candidates) {
  return candidates.map(rgbTripleFromArgb)
}
