import { ref } from 'vue'
import { scrollAt } from './useFocusNav.js'

/**
 * 手柄光标模式。
 *
 * 空间导航覆盖不了每一个页面 —— VDD 设置、DualSense 面板、日志控制台都是密集
 * 表单。这里用左摇杆驱动一个虚拟指针，把「手柄到不了的地方」兜住：合成真实的
 * 鼠标事件，所以未适配的组件不需要任何改动就能操作。
 */

const CURSOR_MARGIN = 4

export const cursorEnabled = ref(false)
export const cursorX = ref(0)
export const cursorY = ref(0)

let initialized = false

function clamp(value, min, max) {
  return Math.min(Math.max(value, min), max)
}

function centerCursor() {
  cursorX.value = window.innerWidth / 2
  cursorY.value = window.innerHeight / 2
}

function elementUnderCursor() {
  return document.elementFromPoint(cursorX.value, cursorY.value)
}

/** 供上层判断光标下的目标是不是文本框（决定 A 键是点击还是弹屏幕键盘）。 */
export function elementAtCursor() {
  if (!cursorEnabled.value) return null
  return elementUnderCursor()
}

function mouseInit(extra = {}) {
  return {
    bubbles: true,
    cancelable: true,
    composed: true,
    view: window,
    clientX: cursorX.value,
    clientY: cursorY.value,
    screenX: cursorX.value,
    screenY: cursorY.value,
    ...extra,
  }
}

let lastHovered = null

function syncHover() {
  const element = elementUnderCursor()
  if (element === lastHovered) {
    element?.dispatchEvent(new MouseEvent('mousemove', mouseInit()))
    return
  }

  if (lastHovered) {
    lastHovered.dispatchEvent(new MouseEvent('mouseout', mouseInit({ relatedTarget: element })))
    lastHovered.dispatchEvent(
      new MouseEvent('mouseleave', mouseInit({ bubbles: false, relatedTarget: element }))
    )
  }
  if (element) {
    element.dispatchEvent(new MouseEvent('mouseover', mouseInit({ relatedTarget: lastHovered })))
    element.dispatchEvent(
      new MouseEvent('mouseenter', mouseInit({ bubbles: false, relatedTarget: lastHovered }))
    )
    element.dispatchEvent(new MouseEvent('mousemove', mouseInit()))
  }
  lastHovered = element
}

export function moveCursor(dx, dy) {
  if (!cursorEnabled.value) return
  cursorX.value = clamp(cursorX.value + dx, CURSOR_MARGIN, window.innerWidth - CURSOR_MARGIN)
  cursorY.value = clamp(cursorY.value + dy, CURSOR_MARGIN, window.innerHeight - CURSOR_MARGIN)
  syncHover()
}

export function clickAtCursor() {
  if (!cursorEnabled.value) return false
  const element = elementUnderCursor()
  if (!element) return false

  // 先把焦点交给目标，某些组件依赖 focus 才会响应后续交互
  const focusTarget = element.closest('[tabindex], button, a[href], input, select, textarea')
  if (focusTarget instanceof HTMLElement) focusTarget.focus({ preventScroll: true })

  element.dispatchEvent(new PointerEvent('pointerdown', mouseInit({ pointerId: 1, button: 0 })))
  element.dispatchEvent(new MouseEvent('mousedown', mouseInit({ button: 0, buttons: 1 })))
  element.dispatchEvent(new PointerEvent('pointerup', mouseInit({ pointerId: 1, button: 0 })))
  element.dispatchEvent(new MouseEvent('mouseup', mouseInit({ button: 0, buttons: 0 })))
  element.dispatchEvent(new MouseEvent('click', mouseInit({ button: 0, detail: 1 })))
  return true
}

/** 复用已有的鼠标右键菜单路径，不需要为手柄再写一套。 */
export function contextMenuAtCursor() {
  if (!cursorEnabled.value) return false
  const element = elementUnderCursor()
  if (!element) return false
  element.dispatchEvent(new MouseEvent('contextmenu', mouseInit({ button: 2 })))
  return true
}

export function scrollAtCursor(deltaY, deltaX = 0) {
  if (!cursorEnabled.value) return false
  return scrollAt(cursorX.value, cursorY.value, deltaY, deltaX)
}

export function setCursorEnabled(enabled) {
  if (cursorEnabled.value === enabled) return
  cursorEnabled.value = enabled
  if (enabled) {
    if (!initialized) {
      centerCursor()
      initialized = true
    }
    // 视口可能已经变化（分辨率自适应），重新夹一次
    cursorX.value = clamp(cursorX.value, CURSOR_MARGIN, window.innerWidth - CURSOR_MARGIN)
    cursorY.value = clamp(cursorY.value, CURSOR_MARGIN, window.innerHeight - CURSOR_MARGIN)
    syncHover()
  } else if (lastHovered) {
    lastHovered.dispatchEvent(new MouseEvent('mouseout', mouseInit()))
    lastHovered.dispatchEvent(new MouseEvent('mouseleave', mouseInit({ bubbles: false })))
    lastHovered = null
  }
}
