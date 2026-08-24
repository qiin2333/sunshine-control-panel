import test from 'node:test'
import assert from 'node:assert/strict'

import {
  argbFromRgbTriple,
  candidatesToSwatches,
  contrastRatio,
  extractSeedCandidates,
  imageDataToArgb,
  m3ThemeFromSeed,
  relativeLuminance,
} from './useDynamicColor.js'

/** 构造合成 ImageData（RGBA Uint8ClampedArray）。 */
function makeImageData(pixels) {
  const data = new Uint8ClampedArray(pixels.flat())
  return { data, width: pixels.length, height: 1 }
}

/** 多色分布：模拟一张有几个色块的壁纸。 */
function mixedPixels() {
  const colors = [
    [220, 30, 30], // 红
    [30, 120, 220], // 蓝
    [240, 200, 60], // 黄
    [40, 160, 80], // 绿
  ]
  const pixels = []
  for (const color of colors) {
    for (let i = 0; i < 200; i++) pixels.push(argbFromRgbTriple(color))
  }
  return pixels
}

function hexToTriple(hex) {
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ]
}

/** 组件实际消费的变量清单——映射完整性以此为准。 */
const REQUIRED_VARS = [
  '--fd-accent',
  '--fd-accent-rgb',
  '--fd-accent-secondary',
  '--fd-accent-secondary-rgb',
  '--fd-bg-primary',
  '--fd-bg-secondary',
  '--fd-bg-tertiary',
  '--fd-text-primary',
  '--fd-text-primary-rgb',
  '--fd-text-secondary',
  '--fd-text-muted',
  '--fd-status-success',
  '--fd-status-success-rgb',
  '--fd-status-warning',
  '--fd-status-warning-rgb',
  '--fd-status-danger',
  '--fd-status-danger-rgb',
]

const RGB_VAR = /^\d+, \d+, \d+$/
const HEX = /^#[0-9a-f]{6}$/

test('同一像素集合永远得到同一个种子（旧的 k-means 做不到这一点）', () => {
  const pixels = mixedPixels()
  const first = extractSeedCandidates(pixels)
  for (let i = 0; i < 5; i++) {
    const again = extractSeedCandidates(pixels)
    assert.equal(again.seed, first.seed)
    assert.deepEqual(again.candidates, first.candidates)
  }
})

test('多色壁纸产出多个候选，且全部是合法颜色', () => {
  const { candidates } = extractSeedCandidates(mixedPixels())
  assert.ok(candidates.length >= 2, `期望至少 2 个候选，实际 ${candidates.length}`)
  for (const swatch of candidatesToSwatches(candidates)) {
    assert.equal(swatch.length, 3)
    for (const channel of swatch) {
      assert.ok(channel >= 0 && channel <= 255)
    }
  }
})

test('色相丰富的壁纸能拿到满额 6 个候选（desired 必须走 options 对象传入）', () => {
  // 0.2.7 的 Score.score(colors, options)：如果第二个参数误传数值，
  // 展开后 desired 为 undefined，会静默落回默认 4 —— 这个测试钉住该回归
  const colors = [
    [225, 40, 40], [240, 140, 30], [240, 210, 60], [50, 180, 90],
    [40, 160, 220], [90, 90, 235], [180, 70, 220], [230, 70, 150],
  ]
  const pixels = []
  for (const color of colors) {
    for (let i = 0; i < 150; i++) pixels.push(argbFromRgbTriple(color))
  }
  const { candidates } = extractSeedCandidates(pixels)
  assert.equal(candidates.length, 6, `期望 6 个候选，实际 ${candidates.length}`)
})

test('imageDataToArgb 跳过透明像素并按 ARGB 打包', () => {
  // ImageData 是 RGBA 字节序
  const imageData = makeImageData([
    [10, 20, 30, 255],
    [99, 99, 99, 0], // alpha < 128，跳过
    [40, 50, 60, 255],
  ])
  const argbs = imageDataToArgb(imageData)
  assert.equal(argbs.length, 2)
  assert.equal(argbs[0], argbFromRgbTriple([10, 20, 30]))
  assert.equal(argbs[1], argbFromRgbTriple([40, 50, 60]))
})

test('种子映射出组件消费的全部变量，格式正确', () => {
  const HEX_KEYS = REQUIRED_VARS.filter(
    (k) =>
      !k.endsWith('-rgb') &&
      k !== '--fd-text-secondary' && // rgba() 形式
      k !== '--fd-text-muted' // rgba() 形式
  )
  for (const seed of [0xffdc1e1e, 0xff1e7adc, 0xffe8d5a3]) {
    const vars = m3ThemeFromSeed(seed)
    for (const key of REQUIRED_VARS) {
      assert.ok(vars[key] !== undefined, `种子 ${seed.toString(16)} 缺少 ${key}`)
    }
    for (const key of REQUIRED_VARS.filter((k) => k.endsWith('-rgb'))) {
      assert.match(vars[key], RGB_VAR, `${key} 应为 'r, g, b' 形式`)
    }
    for (const key of HEX_KEYS) {
      assert.match(vars[key], HEX, `${key} 应为 #rrggbb 形式`)
    }
    // 壁纸主题关掉网格和扫描线
    assert.equal(vars['--fd-grid-visible'], '0')
    assert.equal(vars['--fd-scanline-visible'], '0')
  }
})

test('M3 档位选择保证正文对比度：任意种子下 onSurface vs 背景都 >= 4.5:1', () => {
  const seeds = [0xffdc1e1e, 0xff1e7adc, 0xffe8d5a3, 0xff222222, 0xff98fb98, 0xffff00ff]
  for (const seed of seeds) {
    const vars = m3ThemeFromSeed(seed)
    const text = hexToTriple(vars['--fd-text-primary'])
    const bg = hexToTriple(vars['--fd-bg-primary'])
    const ratio = contrastRatio(text, bg)
    assert.ok(
      ratio >= 4.5,
      `种子 ${seed.toString(16)} 的正文对比度仅 ${ratio.toFixed(2)}:1`
    )
  }
})

test('强调色在深背景上足够醒目（>= 3:1，图形元素标准）', () => {
  for (const seed of [0xffdc1e1e, 0xff1e7adc, 0xffe8d5a3]) {
    const vars = m3ThemeFromSeed(seed)
    const accent = hexToTriple(vars['--fd-accent'])
    const bg = hexToTriple(vars['--fd-bg-primary'])
    assert.ok(
      contrastRatio(accent, bg) >= 3,
      `种子 ${seed.toString(16)} 的强调色对比度不足`
    )
  }
})

test('背景三层是单调抬升的（bg-primary 最暗，bg-tertiary 最亮）', () => {
  for (const seed of [0xffdc1e1e, 0xff1e7adc]) {
    const vars = m3ThemeFromSeed(seed)
    const levels = ['--fd-bg-primary', '--fd-bg-secondary', '--fd-bg-tertiary'].map((key) =>
      relativeLuminance(hexToTriple(vars[key]))
    )
    assert.ok(levels[0] < levels[1] && levels[1] < levels[2], `背景层级混乱: ${levels}`)
  }
})

test('状态色三态互不相同且都偏亮（深背景上可读）', () => {
  const vars = m3ThemeFromSeed(0xff1e7adc)
  const statuses = ['--fd-status-success', '--fd-status-warning', '--fd-status-danger'].map((key) =>
    vars[key]
  )
  assert.equal(new Set(statuses).size, 3)
  for (const status of statuses) {
    const [r, g, b] = hexToTriple(status)
    assert.ok(relativeLuminance([r, g, b]) > 0.25, `${status} 在深背景上太暗`)
  }
})
