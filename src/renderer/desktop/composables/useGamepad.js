import { ref, onMounted, onUnmounted } from 'vue'

// 手柄按键映射 (Xbox 标准布局)
export const BUTTON = {
  A: 0,
  B: 1,
  X: 2,
  Y: 3,
  LB: 4,
  RB: 5,
  LT: 6,
  RT: 7,
  BACK: 8,
  START: 9,
  L3: 10,
  R3: 11,
  DPAD_UP: 12,
  DPAD_DOWN: 13,
  DPAD_LEFT: 14,
  DPAD_RIGHT: 15,
}

const AXIS = {
  LEFT_X: 0,
  LEFT_Y: 1,
  RIGHT_X: 2,
  RIGHT_Y: 3,
}

/** 按键 → 语义动作。视图层只关心动作名，不关心按键号。 */
/**
 * 按键 → 语义动作，对齐主机惯例：
 *   A 确认 / B 返回（长按回首页）/ LB RB 换页 —— 全局
 *   Y 搜索、X 收藏、START 菜单 —— 视图级（见 useGamepadActions）
 *   LT/RT 不再是离散按键：扳机有模拟行程，半按即触发，当翻页键误触率高，
 *   改为模拟量驱动的快速滚动（正统用法），在 poll 循环里直接读值
 *   BACK 不绑定（保留给未来）
 */
const BUTTON_ACTIONS = {
  [BUTTON.A]: 'confirm',
  [BUTTON.B]: 'back',
  [BUTTON.X]: 'favorite',
  [BUTTON.Y]: 'search',
  [BUTTON.LB]: 'tabPrev',
  [BUTTON.RB]: 'tabNext',
  [BUTTON.START]: 'menu',
  [BUTTON.L3]: 'cursorToggle',
  [BUTTON.R3]: 'home',
}

const DPAD_DIRECTIONS = {
  [BUTTON.DPAD_UP]: 'up',
  [BUTTON.DPAD_DOWN]: 'down',
  [BUTTON.DPAD_LEFT]: 'left',
  [BUTTON.DPAD_RIGHT]: 'right',
}

const DEADZONE = 0.4
/** 摇杆/光标的独立死区，比导航死区小，保证微调可用。 */
const ANALOG_DEADZONE = 0.18
const REPEAT_DELAY = 400
const REPEAT_INTERVAL = 120
/** 长按 B 回到顶层的阈值。 */
const BACK_HOLD_MS = 650
/** 轮询间隔。用 setInterval 而不是 rAF：窗口被游戏遮挡时 rAF 会被节流到 1fps。 */
const POLL_INTERVAL_MS = 16
/**
 * 没有手柄时的轮询间隔。
 *
 * 大屏 shell 本身就是被编码进串流画面的那一帧，所以空转要尽量便宜；
 * 只有真的插了手柄才值得 60Hz。
 */
const IDLE_POLL_INTERVAL_MS = 500
/** 右摇杆满偏时每次轮询滚动的像素数。 */
const SCROLL_SPEED = 22
/** 扳机满偏时的快速滚动速度（px/帧），比右摇杆快得多——这是「快速翻到底」用的。 */
const TRIGGER_SCROLL_SPEED = 60
/** 左摇杆满偏时光标每次轮询移动的像素数。 */
const CURSOR_SPEED = 16

/** 视图之间用 window 事件传递手柄动作，避免为此引入全局状态容器。 */
export const GAMEPAD_ACTION_EVENT = 'fd-gamepad-action'

/**
 * 手柄状态是模块级单例：大屏 shell 只会有一个 `useGamepad` 实例，而侧边栏、
 * 应用库这些组件都需要知道「现在是不是在用手柄」来决定要不要抢焦点。
 */
export const gamepadActive = ref(false)
export const gamepadConnected = ref(false)
/** 当前活跃手柄的 id 字符串（含厂商标识），布局识别据此推导。 */
export const gamepadName = ref('')
/** 长按 B 的进度（0..1），供环形进度指示消费。 */
export const backHoldProgress = ref(0)

export function emitGamepadAction(action, detail = {}) {
  window.dispatchEvent(new CustomEvent(GAMEPAD_ACTION_EVENT, { detail: { action, ...detail } }))
}

export function onGamepadAction(handler) {
  const listener = (event) => handler(event.detail?.action, event.detail)
  window.addEventListener(GAMEPAD_ACTION_EVENT, listener)
  return () => window.removeEventListener(GAMEPAD_ACTION_EVENT, listener)
}

function applyDeadzone(value, deadzone) {
  if (Math.abs(value) <= deadzone) return 0
  // 重新映射到 0..1，避免越过死区时的跳变
  const sign = value < 0 ? -1 : 1
  return sign * ((Math.abs(value) - deadzone) / (1 - deadzone))
}

export function useGamepad(options = {}) {
  const {
    onNavigate,
    onScroll,
    onCursorMove,
    onAction,
    isCursorMode = () => false,
    enabled = () => true,
  } = options

  let timerId = null
  let currentInterval = IDLE_POLL_INTERVAL_MS
  /** 每个手柄独立记录上一帧状态，这样切换手柄时边沿检测依然正确。 */
  const padStates = new Map()
  let activeIndex = null
  const repeatTimers = {}
  let backHoldTimer = null
  let backHoldRaf = null
  let backHoldConsumed = false

  function markActive() {
    if (!gamepadActive.value) gamepadActive.value = true
  }

  function stopRepeat(key) {
    const timer = repeatTimers[key]
    if (!timer) return
    clearTimeout(timer.timeout)
    clearInterval(timer.interval)
    delete repeatTimers[key]
  }

  function stopAllRepeats() {
    Object.keys(repeatTimers).forEach(stopRepeat)
  }

  function startRepeat(key, action) {
    action()
    stopRepeat(key)
    const timer = {}
    timer.timeout = setTimeout(() => {
      timer.interval = setInterval(action, REPEAT_INTERVAL)
    }, REPEAT_DELAY)
    repeatTimers[key] = timer
  }

  function clearBackHold() {
    if (backHoldTimer) {
      clearTimeout(backHoldTimer)
      backHoldTimer = null
    }
    if (backHoldRaf) {
      cancelAnimationFrame(backHoldRaf)
      backHoldRaf = null
    }
    if (backHoldProgress.value > 0) backHoldProgress.value = 0
  }

  /**
   * 长按 B 的进度指示（0..1）。主机惯例是按住时出现环形进度——没有它，
   * 「长按回首页」等于不存在：没人会发现，或纯误触。
   */
  function trackBackHold(startedAt) {
    const tick = () => {
      const elapsed = performance.now() - startedAt
      const progress = Math.min(1, elapsed / BACK_HOLD_MS)
      backHoldProgress.value = progress
      if (progress < 1) backHoldRaf = requestAnimationFrame(tick)
    }
    backHoldRaf = requestAnimationFrame(tick)
  }

  function dispatchAction(action) {
    markActive()
    onAction?.(action)
  }

  function handleButtonDown(index) {
    markActive()

    const direction = DPAD_DIRECTIONS[index]
    if (direction) {
      startRepeat(`dpad_${direction}`, () => onNavigate?.(direction))
      return
    }

    if (index === BUTTON.B) {
      // 短按返回上一层，长按直接回到顶层；长按过程有环形进度反馈
      backHoldConsumed = false
      clearBackHold()
      trackBackHold(performance.now())
      backHoldTimer = setTimeout(() => {
        backHoldTimer = null
        clearBackHold()
        backHoldConsumed = true
        dispatchAction('backRoot')
      }, BACK_HOLD_MS)
      return
    }

    const action = BUTTON_ACTIONS[index]
    if (action) dispatchAction(action)
  }

  function handleButtonUp(index) {
    const direction = DPAD_DIRECTIONS[index]
    if (direction) {
      stopRepeat(`dpad_${direction}`)
      return
    }

    if (index === BUTTON.B) {
      clearBackHold()
      if (!backHoldConsumed) dispatchAction('back')
      backHoldConsumed = false
    }
  }

  function handleNavAxis(axisIndex, value) {
    const positiveKey = `axis_${axisIndex}_pos`
    const negativeKey = `axis_${axisIndex}_neg`
    const activeKey = value > 0 ? positiveKey : negativeKey
    const idleKey = value > 0 ? negativeKey : positiveKey

    stopRepeat(idleKey)

    if (Math.abs(value) <= DEADZONE) {
      stopRepeat(activeKey)
      return
    }

    markActive()
    if (repeatTimers[activeKey]) return

    const direction =
      axisIndex === AXIS.LEFT_Y ? (value > 0 ? 'down' : 'up') : value > 0 ? 'right' : 'left'
    startRepeat(activeKey, () => onNavigate?.(direction))
  }

  function releaseNavAxes() {
    stopRepeat(`axis_${AXIS.LEFT_X}_pos`)
    stopRepeat(`axis_${AXIS.LEFT_X}_neg`)
    stopRepeat(`axis_${AXIS.LEFT_Y}_pos`)
    stopRepeat(`axis_${AXIS.LEFT_Y}_neg`)
  }

  function padStateFor(index) {
    let state = padStates.get(index)
    if (!state) {
      state = { buttons: [], axes: [] }
      padStates.set(index, state)
    }
    return state
  }

  /**
   * 收集一个手柄相对上一帧的变化，并把当前帧写回。
   * 返回 { edges, moved } —— edges 是按键边沿，moved 表示摇杆有明显位移。
   */
  function diffPad(pad) {
    const state = padStateFor(pad.index)
    const edges = []
    let moved = false

    for (let i = 0; i < pad.buttons.length; i++) {
      const pressed = !!pad.buttons[i]?.pressed
      const wasPressed = state.buttons[i] || false
      if (pressed !== wasPressed) edges.push({ index: i, pressed })
      state.buttons[i] = pressed
    }

    for (let i = 0; i < pad.axes.length; i++) {
      const value = pad.axes[i] || 0
      const previous = state.axes[i] || 0
      if (Math.abs(value) > DEADZONE && Math.abs(value - previous) > 0.1) moved = true
      state.axes[i] = value
    }

    return { edges, moved }
  }

  function connectedPads() {
    const pads = navigator.getGamepads?.() || []
    const result = []
    for (let i = 0; i < pads.length; i++) {
      if (pads[i] && pads[i].connected) result.push(pads[i])
    }
    return result
  }

  function switchActivePad(index) {
    if (activeIndex === index) return
    activeIndex = index
    // 活跃手柄变了，id（布局识别的数据源）跟着换
    gamepadName.value = connectedPads().find((pad) => pad.index === index)?.id || gamepadName.value
    // 换手柄时旧的按住状态不再有效
    stopAllRepeats()
    clearBackHold()
    backHoldConsumed = false
  }

  function poll() {
    const pads = connectedPads()
    gamepadConnected.value = pads.length > 0
    applyPollRate(pads.length > 0)

    if (pads.length === 0) {
      activeIndex = null
      padStates.clear()
      stopAllRepeats()
      clearBackHold()
      return
    }

    // 每一帧都为所有手柄做边沿检测：串流时主机上同时存在 Sunshine 虚拟手柄和
    // 物理手柄，只信任「最近真的动过」的那一个，而不是列表里的第一个。
    const diffs = pads.map((pad) => ({ pad, ...diffPad(pad) }))

    if (activeIndex === null || !pads.some((pad) => pad.index === activeIndex)) {
      const firstBusy = diffs.find((entry) => entry.edges.length > 0 || entry.moved)
      switchActivePad((firstBusy?.pad || pads[0]).index)
    } else {
      const challenger = diffs.find(
        (entry) => entry.pad.index !== activeIndex && (entry.edges.length > 0 || entry.moved)
      )
      if (challenger) switchActivePad(challenger.pad.index)
    }

    const active = diffs.find((entry) => entry.pad.index === activeIndex)
    if (!active) return

    if (!enabled()) {
      // 游戏运行中 / 窗口不可见：保持状态同步但不派发，避免恢复时补发一串输入
      stopAllRepeats()
      clearBackHold()
      return
    }

    for (const edge of active.edges) {
      if (edge.pressed) handleButtonDown(edge.index)
      else handleButtonUp(edge.index)
    }

    const pad = active.pad
    const leftX = pad.axes[AXIS.LEFT_X] || 0
    const leftY = pad.axes[AXIS.LEFT_Y] || 0

    if (isCursorMode()) {
      // 光标模式下左摇杆驱动虚拟指针，不再做焦点导航
      releaseNavAxes()
      const dx = applyDeadzone(leftX, ANALOG_DEADZONE)
      const dy = applyDeadzone(leftY, ANALOG_DEADZONE)
      if (dx || dy) {
        markActive()
        onCursorMove?.(dx * CURSOR_SPEED, dy * CURSOR_SPEED)
      }
    } else if (pad.axes.length >= 2) {
      handleNavAxis(AXIS.LEFT_X, leftX)
      handleNavAxis(AXIS.LEFT_Y, leftY)
    }

    if (pad.axes.length >= 4) {
      const scrollY = applyDeadzone(pad.axes[AXIS.RIGHT_Y] || 0, ANALOG_DEADZONE)
      const scrollX = applyDeadzone(pad.axes[AXIS.RIGHT_X] || 0, ANALOG_DEADZONE)
      if (scrollY || scrollX) {
        markActive()
        onScroll?.(scrollY * SCROLL_SPEED, scrollX * SCROLL_SPEED)
      }
    }

    // 扳机快速滚动：模拟量直接驱动滚动速度，轻点是慢滚，按到底是全速。
    // 这是扳机的正统用法——之前把它们当离散翻页键，半按即触发，误触率高。
    const lt = pad.buttons[BUTTON.LT]?.value || 0
    const rt = pad.buttons[BUTTON.RT]?.value || 0
    const triggerScroll = Math.max(lt, rt)
    if (triggerScroll > 0.55) {
      markActive()
      const direction = rt >= lt ? 1 : -1
      onScroll?.(direction * (triggerScroll - 0.55) * TRIGGER_SCROLL_SPEED)
    }
  }

  function startPolling(intervalMs = currentInterval) {
    stopPolling()
    currentInterval = intervalMs
    timerId = setInterval(poll, intervalMs)
  }

  function stopPolling() {
    if (timerId === null) return
    clearInterval(timerId)
    timerId = null
  }

  /** 手柄接入/拔出时切换轮询频率，空转时不必每 16ms 唤醒一次。 */
  function applyPollRate(hasPad) {
    const wanted = hasPad ? POLL_INTERVAL_MS : IDLE_POLL_INTERVAL_MS
    if (wanted !== currentInterval) startPolling(wanted)
  }

  function onGamepadConnected(event) {
    gamepadConnected.value = true
    // 尚无活跃手柄时先用接入者的 id，提示条不用等第一次输入才识别布局
    if (!gamepadName.value && event?.gamepad?.id) gamepadName.value = event.gamepad.id
    startPolling(POLL_INTERVAL_MS)
  }

  function onGamepadDisconnected(event) {
    padStates.delete(event?.gamepad?.index)
    if (activeIndex === event?.gamepad?.index) activeIndex = null
    if (connectedPads().length === 0) {
      gamepadConnected.value = false
      gamepadActive.value = false
      stopAllRepeats()
      clearBackHold()
    }
  }

  function onMouseMove(event) {
    // 光标模式合成的鼠标事件 isTrusted 为 false，不能因此退出手柄模式
    if (event.isTrusted && gamepadActive.value) gamepadActive.value = false
  }

  function onVisibilityChange() {
    if (document.hidden) {
      stopPolling()
      stopAllRepeats()
      clearBackHold()
    } else {
      startPolling()
    }
  }

  onMounted(() => {
    window.addEventListener('gamepadconnected', onGamepadConnected)
    window.addEventListener('gamepaddisconnected', onGamepadDisconnected)
    window.addEventListener('mousemove', onMouseMove)
    document.addEventListener('visibilitychange', onVisibilityChange)
    startPolling()
  })

  onUnmounted(() => {
    window.removeEventListener('gamepadconnected', onGamepadConnected)
    window.removeEventListener('gamepaddisconnected', onGamepadDisconnected)
    window.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('visibilitychange', onVisibilityChange)
    stopPolling()
    stopAllRepeats()
    clearBackHold()
  })

}
