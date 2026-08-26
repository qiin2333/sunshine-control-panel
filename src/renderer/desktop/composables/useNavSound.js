import { bigScreenSettings } from './useBigScreenSettings.js'

/**
 * 手柄导航音效。三米沙发距离上，声音是主机 shell 最重要的反馈通道。
 *
 * 全部用 WebAudio 现场合成（正弦/三角波短音），不带任何资源文件。
 * AudioContext 延迟创建；WebView2 侧通过启动参数关闭了自动播放限制
 * （UI 音效属于应用自身反馈，不是网页自动播放媒体）。
 */

let ctx = null

function ensureCtx() {
  if (!ctx) {
    const AC = window.AudioContext || window.webkitAudioContext
    if (!AC) return null
    ctx = new AC()
  }
  if (ctx.state === 'suspended') ctx.resume().catch(() => {})
  return ctx
}

/**
 * 单个短音：频率从 from 滑到 to，快速淡入淡出防爆音。
 * 音量刻意压低（0.05~0.07）——是反馈，不是干扰。
 */
function blip({ from, to = from, duration = 60, volume = 0.06, type = 'sine', delay = 0 }) {
  const audio = ensureCtx()
  if (!audio) return
  try {
    const osc = audio.createOscillator()
    const gain = audio.createGain()
    const startTime = audio.currentTime + delay
    osc.type = type
    osc.frequency.setValueAtTime(from, startTime)
    osc.frequency.exponentialRampToValueAtTime(Math.max(40, to), startTime + duration / 1000)
    gain.gain.setValueAtTime(0, startTime)
    gain.gain.linearRampToValueAtTime(volume, startTime + 0.008)
    gain.gain.exponentialRampToValueAtTime(0.0001, startTime + duration / 1000)
    osc.connect(gain).connect(audio.destination)
    osc.start(startTime)
    osc.stop(startTime + duration / 1000 + 0.02)
  } catch {
    // 音频不可用不影响导航
  }
}

const SOUNDS = {
  /** 焦点移动的 tick——Xbox 手柄导航的那种轻微咔哒 */
  tick: () => blip({ from: 1250, to: 1100, duration: 35, volume: 0.045, type: 'triangle' }),
  /** 确认：上行双音 */
  confirm: () => {
    blip({ from: 620, duration: 55, volume: 0.06 })
    blip({ from: 930, duration: 70, volume: 0.06, delay: 0.05 })
  },
  /** 返回：下行双音 */
  back: () => {
    blip({ from: 560, duration: 55, volume: 0.05 })
    blip({ from: 380, duration: 75, volume: 0.05, delay: 0.05 })
  },
  /** 长按回首页达成：一个明确的三连上行 */
  home: () => {
    blip({ from: 520, duration: 50, volume: 0.055 })
    blip({ from: 700, duration: 50, volume: 0.055, delay: 0.06 })
    blip({ from: 940, duration: 90, volume: 0.055, delay: 0.12 })
  },
}

/** 播放导航音。type 未知时静默忽略；设置关闭时直接返回。 */
export function playNavSound(type) {
  if (!bigScreenSettings.value.navSounds) return
  SOUNDS[type]?.()
}
