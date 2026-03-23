<template>
  <Teleport to="body">
    <Transition name="splash" @after-leave="$emit('done')">
      <div v-if="visible" class="splash-screen">
        <!-- 粒子背景 -->
        <canvas ref="canvasRef" class="splash-particles"></canvas>

        <!-- 渐变叠加层 -->
        <div class="splash-overlay"></div>

        <!-- 中心内容 -->
        <div class="splash-content" :class="{ 'reveal': phase >= 1, 'expand': phase >= 2 }">
          <!-- Logo -->
          <div class="splash-logo">
            <div class="logo-glow"></div>
            <span class="logo-icon">☀️</span>
          </div>

          <!-- 标题 -->
          <div class="splash-title">
            <span class="title-main">Foundation</span>
            <span class="title-sub">SUNSHINE</span>
          </div>

          <!-- 底部扫光线 -->
          <div class="splash-line"></div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { ref, watch, onMounted, onUnmounted, nextTick } from 'vue'

const props = defineProps({
  visible: { type: Boolean, default: true },
})

const emit = defineEmits(['done'])

const canvasRef = ref(null)
const phase = ref(0)
let animFrame = null
let timers = []

// ===== 手柄振动 =====
// 缓存通过 gamepadconnected 事件发现的手柄索引
const connectedGamepadIndices = new Set()

function onGamepadConnected(e) {
  connectedGamepadIndices.add(e.gamepad.index)
  console.log('[SplashScreen] gamepad connected:', e.gamepad.index, e.gamepad.id)
}

function onGamepadDisconnected(e) {
  connectedGamepadIndices.delete(e.gamepad.index)
}

function vibrateGamepad(strong = 0, weak = 0, duration = 100) {
  try {
    const gamepads = navigator.getGamepads()
    let vibrated = false

    for (const gp of gamepads) {
      if (!gp) continue

      // 尝试标准 vibrationActuator
      if (gp.vibrationActuator) {
        gp.vibrationActuator.playEffect('dual-rumble', {
          startDelay: 0,
          duration,
          weakMagnitude: weak,
          strongMagnitude: strong,
        }).then(() => {
          console.log('[SplashScreen] vibration OK, gamepad:', gp.index)
        }).catch((err) => {
          console.warn('[SplashScreen] vibration failed:', err)
        })
        vibrated = true
      }
      // 尝试非标准 hapticActuators (Chrome fallback)
      else if (gp.hapticActuators?.length > 0) {
        gp.hapticActuators[0].pulse(strong, duration)
        vibrated = true
      }
    }

    if (!vibrated) {
      console.warn('[SplashScreen] no vibration-capable gamepad found.',
        'Connected indices:', [...connectedGamepadIndices],
        'getGamepads result:', gamepads.length,
        'non-null count:', [...gamepads].filter(Boolean).length)
    }
  } catch (err) {
    console.warn('[SplashScreen] vibrateGamepad error:', err)
  }
}

// ===== 音效系统 (Web Audio API 合成) =====
let audioCtx = null

function getAudioCtx() {
  if (!audioCtx) {
    audioCtx = new (window.AudioContext || window.webkitAudioContext)()
  }
  // 确保 AudioContext 处于运行状态
  if (audioCtx.state === 'suspended') {
    audioCtx.resume()
  }
  return audioCtx
}

/**
 * Phase 1: Logo 出现 — 可爱的弹跳 "叮~"
 */
function playSoundReveal() {
  const ctx = getAudioCtx()
  const now = ctx.currentTime

  // 清脆的高音 "叮" (类似音乐盒)
  const bell = ctx.createOscillator()
  bell.type = 'sine'
  bell.frequency.setValueAtTime(1318.5, now) // E6
  bell.frequency.exponentialRampToValueAtTime(1174.7, now + 0.3) // 轻微下滑，弹跳感

  const bellGain = ctx.createGain()
  bellGain.gain.setValueAtTime(0, now)
  bellGain.gain.linearRampToValueAtTime(0.25, now + 0.01)
  bellGain.gain.exponentialRampToValueAtTime(0.1, now + 0.15)
  bellGain.gain.exponentialRampToValueAtTime(0.01, now + 0.6)

  // 泛音层 (八度+五度 = 更亮更甜)
  const harm = ctx.createOscillator()
  harm.type = 'sine'
  harm.frequency.setValueAtTime(2637, now) // E7
  const harmGain = ctx.createGain()
  harmGain.gain.setValueAtTime(0, now)
  harmGain.gain.linearRampToValueAtTime(0.08, now + 0.01)
  harmGain.gain.exponentialRampToValueAtTime(0.01, now + 0.3)

  // 柔和的"噗"底音
  const pop = ctx.createOscillator()
  pop.type = 'sine'
  pop.frequency.setValueAtTime(300, now)
  pop.frequency.exponentialRampToValueAtTime(150, now + 0.08)

  const popGain = ctx.createGain()
  popGain.gain.setValueAtTime(0.2, now)
  popGain.gain.exponentialRampToValueAtTime(0.01, now + 0.12)

  bell.connect(bellGain).connect(ctx.destination)
  harm.connect(harmGain).connect(ctx.destination)
  pop.connect(popGain).connect(ctx.destination)

  bell.start(now); bell.stop(now + 0.7)
  harm.start(now); harm.stop(now + 0.4)
  pop.start(now); pop.stop(now + 0.15)
}

/**
 * Phase 2: 标题展开 — 暖甜的上升琶音 "叮咚叮~"
 */
function playSoundExpand() {
  const ctx = getAudioCtx()
  const now = ctx.currentTime

  // 快速上行琶音: C6 → E6 → G6 → C7
  const arpNotes = [1046.5, 1318.5, 1568, 2093]
  const spacing = 0.08

  arpNotes.forEach((freq, i) => {
    const t = now + i * spacing
    const osc = ctx.createOscillator()
    osc.type = 'sine'
    osc.frequency.setValueAtTime(freq, t)

    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0, t)
    gain.gain.linearRampToValueAtTime(0.2, t + 0.01)
    gain.gain.exponentialRampToValueAtTime(0.06, t + 0.15)
    gain.gain.exponentialRampToValueAtTime(0.01, t + 0.5)

    osc.connect(gain).connect(ctx.destination)
    osc.start(t)
    osc.stop(t + 0.55)
  })

  // 琶音结束后的柔和 pad 余韵 (C 大三和弦)
  const padStart = now + arpNotes.length * spacing
  const padNotes = [523.25, 659.25, 783.99] // C5, E5, G5
  padNotes.forEach(freq => {
    const osc = ctx.createOscillator()
    osc.type = 'triangle'
    osc.frequency.setValueAtTime(freq, padStart)

    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0, padStart)
    gain.gain.linearRampToValueAtTime(0.08, padStart + 0.1)
    gain.gain.exponentialRampToValueAtTime(0.01, padStart + 0.8)

    osc.connect(gain).connect(ctx.destination)
    osc.start(padStart)
    osc.stop(padStart + 0.9)
  })
}

/**
 * Phase 3: 完成 — 欢快的双音确认 "叮叮♪"
 */
function playSoundFinish() {
  const ctx = getAudioCtx()
  const now = ctx.currentTime

  // 两声活泼短音 (像收到奖励)
  const notes = [
    { freq: 1568, t: 0 },      // G6
    { freq: 2093, t: 0.1 },    // C7 (高八度，更欢快)
  ]

  notes.forEach(({ freq, t }) => {
    const osc = ctx.createOscillator()
    osc.type = 'sine'
    osc.frequency.setValueAtTime(freq, now + t)

    const gain = ctx.createGain()
    gain.gain.setValueAtTime(0, now + t)
    gain.gain.linearRampToValueAtTime(0.22, now + t + 0.01)
    gain.gain.exponentialRampToValueAtTime(0.01, now + t + 0.35)

    // 泛音
    const harm = ctx.createOscillator()
    harm.type = 'sine'
    harm.frequency.setValueAtTime(freq * 2, now + t)
    const harmGain = ctx.createGain()
    harmGain.gain.setValueAtTime(0, now + t)
    harmGain.gain.linearRampToValueAtTime(0.06, now + t + 0.01)
    harmGain.gain.exponentialRampToValueAtTime(0.01, now + t + 0.2)

    osc.connect(gain).connect(ctx.destination)
    harm.connect(harmGain).connect(ctx.destination)
    osc.start(now + t); osc.stop(now + t + 0.4)
    harm.start(now + t); harm.stop(now + t + 0.25)
  })

  // 轻柔尾韵 "嗡~"
  const hum = ctx.createOscillator()
  hum.type = 'triangle'
  hum.frequency.setValueAtTime(523.25, now + 0.15) // C5

  const humGain = ctx.createGain()
  humGain.gain.setValueAtTime(0, now + 0.15)
  humGain.gain.linearRampToValueAtTime(0.06, now + 0.25)
  humGain.gain.exponentialRampToValueAtTime(0.01, now + 0.7)

  hum.connect(humGain).connect(ctx.destination)
  hum.start(now + 0.15)
  hum.stop(now + 0.8)
}

// 粒子系统
function initParticles(canvas) {
  const ctx = canvas.getContext('2d')
  const dpr = window.devicePixelRatio || 1
  let w, h
  const particles = []
  const PARTICLE_COUNT = 60

  // 从主题读取强调色 RGB
  const accentRgb = getComputedStyle(document.documentElement)
    .getPropertyValue('--fd-accent-rgb').trim() || '0, 255, 245'

  function resize() {
    w = window.innerWidth
    h = window.innerHeight
    canvas.width = w * dpr
    canvas.height = h * dpr
    ctx.scale(dpr, dpr)
  }

  resize()

  for (let i = 0; i < PARTICLE_COUNT; i++) {
    particles.push({
      x: Math.random() * w,
      y: Math.random() * h,
      radius: Math.random() * 2 + 0.5,
      vx: (Math.random() - 0.5) * 0.4,
      vy: -(Math.random() * 0.6 + 0.2),
      alpha: Math.random() * 0.5 + 0.1,
      fadeSpeed: Math.random() * 0.003 + 0.001,
    })
  }

  function draw() {
    ctx.clearRect(0, 0, w, h)

    for (const p of particles) {
      p.x += p.vx
      p.y += p.vy
      p.alpha += p.fadeSpeed

      if (p.alpha > 0.6) p.fadeSpeed = -Math.abs(p.fadeSpeed)
      if (p.alpha < 0.05) {
        p.alpha = 0.05
        p.fadeSpeed = Math.abs(p.fadeSpeed)
        p.x = Math.random() * w
        p.y = h + 10
      }

      if (p.y < -10) {
        p.y = h + 10
        p.x = Math.random() * w
      }

      ctx.beginPath()
      ctx.arc(p.x, p.y, p.radius, 0, Math.PI * 2)
      ctx.fillStyle = `rgba(${accentRgb}, ${p.alpha})`
      ctx.fill()
    }

    animFrame = requestAnimationFrame(draw)
  }

  draw()
}

function startAnimation() {
  phase.value = 0

  // 初始化粒子（延迟确保 canvas 已挂载）
  nextTick(() => {
    if (canvasRef.value) {
      initParticles(canvasRef.value)
    }
  })

  // Phase 1: Logo 出现 + 轻弹振动 + 音乐盒 "叮~"
  timers.push(setTimeout(() => {
    phase.value = 1
    vibrateGamepad(0.2, 0.5, 120)
    playSoundReveal()
  }, 300))

  // Phase 2: 标题展开 + 双脉冲振动 + 上行琶音 "叮咚叮~"
  timers.push(setTimeout(() => {
    phase.value = 2
    vibrateGamepad(0.3, 0.6, 150)
    playSoundExpand()
    // 第二下短脉冲
    setTimeout(() => vibrateGamepad(0.15, 0.4, 80), 160)
  }, 1000))

  // Phase 3: 完成 + 欢快双振 + 确认 "叮叮♪"
  timers.push(setTimeout(() => {
    vibrateGamepad(0.25, 0.5, 100)
    setTimeout(() => vibrateGamepad(0.35, 0.6, 120), 110)
    playSoundFinish()
  }, 2200))

  timers.push(setTimeout(() => {
    emit('done')
  }, 2800))
}

onMounted(() => {
  window.addEventListener('gamepadconnected', onGamepadConnected)
  window.addEventListener('gamepaddisconnected', onGamepadDisconnected)

  // 检查已经连接的手柄（页面加载前已插入的手柄）
  try {
    const gamepads = navigator.getGamepads()
    for (const gp of gamepads) {
      if (gp) connectedGamepadIndices.add(gp.index)
    }
  } catch { /* ignore */ }

  if (props.visible) startAnimation()
})

watch(() => props.visible, (val) => {
  if (val) {
    startAnimation()
  } else {
    cleanup()
  }
})

function cleanup() {
  if (animFrame) cancelAnimationFrame(animFrame)
  animFrame = null
  timers.forEach(clearTimeout)
  timers = []
}

onUnmounted(() => {
  cleanup()
  window.removeEventListener('gamepadconnected', onGamepadConnected)
  window.removeEventListener('gamepaddisconnected', onGamepadDisconnected)
  // 关闭 AudioContext 释放资源
  if (audioCtx) {
    audioCtx.close().catch(() => {})
    audioCtx = null
  }
})
</script>

<style lang="less" scoped>
.splash-screen {
  position: fixed;
  inset: 0;
  z-index: 99999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--fd-bg-primary, #0f0f23);
  overflow: hidden;
}

.splash-particles {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

.splash-overlay {
  position: absolute;
  inset: 0;
  background: radial-gradient(
    circle at 50% 50%,
    transparent 0%,
    color-mix(in srgb, var(--fd-bg-primary, #0f0f23) 40%, transparent) 50%,
    color-mix(in srgb, var(--fd-bg-primary, #0f0f23) 90%, transparent) 100%
  );
}

// ===== 中心内容 =====
.splash-content {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 24px;
  opacity: 0;
  transform: scale(0.8);
  transition: all 0.8s cubic-bezier(0.16, 1, 0.3, 1);

  &.reveal {
    opacity: 1;
    transform: scale(1);
  }

  &.expand {
    .splash-title {
      opacity: 1;
      transform: translateY(0);
    }
    .splash-line {
      transform: scaleX(1);
      opacity: 1;
    }
  }
}

// Logo
.splash-logo {
  position: relative;
  width: 120px;
  height: 120px;
  display: flex;
  align-items: center;
  justify-content: center;

  .logo-glow {
    position: absolute;
    width: 160px;
    height: 160px;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      rgba(var(--fd-accent-rgb, 0, 255, 245), 0.25) 0%,
      rgba(var(--fd-accent-rgb, 0, 255, 245), 0.05) 50%,
      transparent 70%
    );
    animation: glow-pulse 2s ease-in-out infinite;
  }

  .logo-icon {
    font-size: 72px;
    filter: drop-shadow(0 0 30px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.6));
    animation: logo-spin 1.5s ease-out;
  }
}

// 标题
.splash-title {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  opacity: 0;
  transform: translateY(20px);
  transition: all 0.6s 0.15s cubic-bezier(0.16, 1, 0.3, 1);

  .title-main {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: 12px;
    text-indent: 12px;
    text-transform: uppercase;
    color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.7);
  }

  .title-sub {
    font-size: 52px;
    font-weight: 800;
    letter-spacing: 8px;
    text-indent: 8px;
    background: linear-gradient(
      135deg,
      var(--fd-text-primary, #fff) 0%,
      var(--fd-accent, #00fff5) 50%,
      rgba(var(--fd-accent-secondary-rgb, 255, 0, 255), 0.8) 100%
    );
    background-clip: text;
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    text-shadow: none;
    filter: drop-shadow(0 0 40px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3));
  }
}

// 扫光线
.splash-line {
  width: 400px;
  height: 2px;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(var(--fd-accent-rgb, 0, 255, 245), 0.8),
    rgba(var(--fd-accent-secondary-rgb, 255, 0, 255), 0.4),
    transparent
  );
  transform: scaleX(0);
  opacity: 0;
  transition: all 0.8s 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

// ===== 动画 =====
@keyframes glow-pulse {
  0%, 100% { transform: scale(1); opacity: 0.6; }
  50% { transform: scale(1.15); opacity: 1; }
}

@keyframes logo-spin {
  0% {
    transform: rotate(-180deg) scale(0);
    opacity: 0;
  }
  60% {
    transform: rotate(15deg) scale(1.1);
    opacity: 1;
  }
  100% {
    transform: rotate(0deg) scale(1);
    opacity: 1;
  }
}

// ===== 过渡 =====
.splash-enter-active {
  transition: opacity 0.3s ease;
}

.splash-leave-active {
  transition: opacity 0.6s ease;

  .splash-content {
    transition: all 0.5s cubic-bezier(0.7, 0, 0.84, 0);
    transform: scale(1.1);
    opacity: 0;
  }
}

.splash-enter-from {
  opacity: 0;
}

.splash-leave-to {
  opacity: 0;
}
</style>
