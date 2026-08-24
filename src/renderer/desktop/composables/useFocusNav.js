import { nextTick, onUnmounted, ref, watch } from 'vue'

/**
 * 大屏模式的焦点系统。
 *
 * 旧实现直接在 `document` 上查询可聚焦元素，于是弹层打开后焦点会跑到被遮住的
 * 底层元素上。这里引入「作用域栈」：模态框压栈后，导航和 focus trap 都只在栈顶
 * 元素内进行，弹出时把焦点还给打开它的元素。
 */

const FOCUSABLE_SELECTOR = [
  '[data-focusable]:not([disabled]):not([data-focusable="false"])',
  '.app-tile',
  '.recent-tile',
  '.app-list-item',
  '.nav-item:not(.disabled)',
  '.desktop-card.action-card',
  'button:not([disabled])',
  'a[href]',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ')

/** 方向判定容差，避免同一行内的亚像素差异被当成上下关系。 */
const DIRECTION_TOLERANCE = 4

const scopeStack = ref([])
/** 作用域 → 压栈前的焦点元素，用于弹出后归还焦点。 */
const scopeReturnFocus = new WeakMap()
/** 视图 key → 上次焦点标识，用于切回视图时恢复位置。 */
const focusMemory = new Map()

let trapInstalled = false

export const focusScopeStack = scopeStack

function topScope() {
  return scopeStack.value[scopeStack.value.length - 1] || null
}

/** 当前应当参与导航的根节点。 */
export function activeScopeRoot() {
  return topScope() || document.querySelector('.desktop-window') || document.body
}

function isVisible(element) {
  if (element.hasAttribute('disabled') || element.getAttribute('aria-hidden') === 'true') {
    return false
  }
  const rect = element.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) return false
  // 视口外的元素（例如横向条里滚走的部分）仍然可聚焦，只排除被隐藏的
  const style = window.getComputedStyle(element)
  return style.visibility !== 'hidden' && style.display !== 'none'
}

export function collectFocusables(root = activeScopeRoot()) {
  if (!root) return []
  return Array.from(root.querySelectorAll(FOCUSABLE_SELECTOR)).filter(isVisible)
}

function edgeGap(aStart, aEnd, bStart, bEnd) {
  const overlap = Math.min(aEnd, bEnd) - Math.max(aStart, bStart)
  return overlap > 0 ? 0 : -overlap
}

/**
 * 按方向从一组矩形里挑最合适的目标，返回它在数组里的下标（找不到返回 -1）。
 *
 * 用边距而不是中心距：网格里同一行的相邻卡片才不会被上下方向抢走。交叉方向的
 * 偏移权重更高，保证移动尽量留在同一行/同一列上。
 *
 * 纯函数，与 DOM 无关，方便单独验证。
 */
export function chooseByDirection(direction, from, rects) {
  let bestIndex = -1
  let bestScore = Infinity

  for (let index = 0; index < rects.length; index++) {
    const rect = rects[index]
    if (!rect) continue

    let primary
    let cross
    switch (direction) {
      case 'up':
        if (rect.bottom > from.top - DIRECTION_TOLERANCE) continue
        primary = from.top - rect.bottom
        cross = edgeGap(from.left, from.right, rect.left, rect.right)
        break
      case 'down':
        if (rect.top < from.bottom - DIRECTION_TOLERANCE) continue
        primary = rect.top - from.bottom
        cross = edgeGap(from.left, from.right, rect.left, rect.right)
        break
      case 'left':
        if (rect.right > from.left - DIRECTION_TOLERANCE) continue
        primary = from.left - rect.right
        cross = edgeGap(from.top, from.bottom, rect.top, rect.bottom)
        break
      case 'right':
        if (rect.left < from.right - DIRECTION_TOLERANCE) continue
        primary = rect.left - from.right
        cross = edgeGap(from.top, from.bottom, rect.top, rect.bottom)
        break
      default:
        continue
    }

    const score = Math.max(primary, 0) + cross * 3
    if (score < bestScore) {
      bestScore = score
      bestIndex = index
    }
  }

  return bestIndex
}

/**
 * 阅读顺序（先上下再左右）的下标序列，用于行末左右移动时自然换行。
 *
 * 纯函数，与 DOM 无关。
 */
export function readingOrderIndexes(rects) {
  return rects
    .map((rect, index) => ({ rect, index }))
    .sort((a, b) => {
      if (Math.abs(a.rect.top - b.rect.top) > DIRECTION_TOLERANCE) return a.rect.top - b.rect.top
      return a.rect.left - b.rect.left
    })
    .map((entry) => entry.index)
}

function bestCandidate(direction, current, candidates) {
  const from = current.getBoundingClientRect()
  const rects = candidates.map((element) =>
    element === current ? null : element.getBoundingClientRect()
  )
  const index = chooseByDirection(direction, from, rects)
  return index === -1 ? null : candidates[index]
}

function readingOrder(candidates) {
  const rects = candidates.map((element) => element.getBoundingClientRect())
  return readingOrderIndexes(rects).map((index) => candidates[index])
}

export function focusElement(element) {
  if (!element) return false
  element.focus({ preventScroll: true })
  element.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' })
  return true
}

export function focusFirst(root = activeScopeRoot()) {
  const focusables = collectFocusables(root)
  return focusElement(readingOrder(focusables)[0] || null)
}

export function navigateFocus(direction, root = activeScopeRoot()) {
  const focusables = collectFocusables(root)
  if (focusables.length === 0) return false

  const current = document.activeElement
  if (!current || current === document.body || !focusables.includes(current)) {
    return focusFirst(root)
  }

  const target = bestCandidate(direction, current, focusables)
  if (target) return focusElement(target)

  // 行末/列尾按左右时按阅读顺序换行，避免「卡住不动」
  if (direction === 'left' || direction === 'right') {
    const ordered = readingOrder(focusables)
    const index = ordered.indexOf(current)
    const next = ordered[direction === 'right' ? index + 1 : index - 1]
    if (next) return focusElement(next)
  }

  return false
}

export function confirmFocused() {
  const element = document.activeElement
  if (!element || element === document.body) return false
  element.click()
  return true
}

// ===== 滚动 =====

function isScrollable(element) {
  if (!(element instanceof HTMLElement)) return false
  if (element.scrollHeight - element.clientHeight <= 2) return false
  const overflowY = window.getComputedStyle(element).overflowY
  return overflowY === 'auto' || overflowY === 'scroll' || overflowY === 'overlay'
}

function findScrollContainer(start) {
  let node = start
  while (node && node !== document.body) {
    if (isScrollable(node)) return node
    node = node.parentElement
  }
  const scope = topScope()
  if (scope && isScrollable(scope)) return scope
  return document.querySelector('.desktop-window-main')
}

/** 右摇杆滚动：优先滚焦点所在的容器，没有焦点则滚主内容区。 */
export function scrollActiveScope(deltaY, deltaX = 0) {
  const anchor = document.activeElement instanceof HTMLElement ? document.activeElement : null
  const container = findScrollContainer(anchor || topScope())
  if (!container) return false
  container.scrollBy({ top: deltaY, left: deltaX, behavior: 'auto' })
  return true
}

/** 光标模式下按指针位置滚动，语义和真实滚轮一致。 */
export function scrollAt(x, y, deltaY, deltaX = 0) {
  const target = document.elementFromPoint(x, y)
  const container = findScrollContainer(target instanceof HTMLElement ? target : null)
  if (!container) return false
  container.scrollBy({ top: deltaY, left: deltaX, behavior: 'auto' })
  return true
}

// ===== 作用域栈 / focus trap =====

function handleFocusIn(event) {
  const scope = topScope()
  if (!scope) return
  if (scope.contains(event.target)) return
  // 焦点逃出栈顶作用域（Tab 或程序化聚焦），拉回来
  const focusables = collectFocusables(scope)
  focusElement(focusables[0] || scope)
}

function ensureTrap() {
  if (trapInstalled) return
  document.addEventListener('focusin', handleFocusIn, true)
  trapInstalled = true
}

function releaseTrap() {
  if (!trapInstalled || scopeStack.value.length > 0) return
  document.removeEventListener('focusin', handleFocusIn, true)
  trapInstalled = false
}

/**
 * 把一个弹层压入焦点栈。返回一个 dispose 函数，等价于 `popFocusScope(element)`。
 */
export function pushFocusScope(element, { autoFocus = true } = {}) {
  if (!element) return () => {}
  if (scopeStack.value.includes(element)) return () => popFocusScope(element)

  scopeReturnFocus.set(
    element,
    document.activeElement instanceof HTMLElement ? document.activeElement : null
  )
  scopeStack.value = [...scopeStack.value, element]
  ensureTrap()
  if (autoFocus) {
    // 让弹层内容先渲染出来再聚焦。这里用 setTimeout 而不是 rAF：窗口最小化或
    // 被完全遮挡时浏览器不合成帧，rAF 回调根本不会执行，焦点就永远进不了弹层。
    setTimeout(() => {
      if (topScope() === element) focusFirst(element)
    }, 0)
  }
  return () => popFocusScope(element)
}

export function popFocusScope(element) {
  if (!element || !scopeStack.value.includes(element)) return
  scopeStack.value = scopeStack.value.filter((entry) => entry !== element)
  const previous = scopeReturnFocus.get(element)
  scopeReturnFocus.delete(element)
  releaseTrap()
  if (previous && previous.isConnected) focusElement(previous)
}

// ===== 视图焦点记忆 =====

function focusIdentity(element) {
  return element?.dataset?.focusKey || null
}

/** 记住某个视图当前的焦点，供切回时恢复。 */
export function rememberFocus(viewKey) {
  const current = document.activeElement
  if (!(current instanceof HTMLElement)) return
  const focusables = collectFocusables()
  focusMemory.set(viewKey, {
    key: focusIdentity(current),
    index: focusables.indexOf(current),
  })
}

/**
 * 恢复某个视图上次的焦点。`data-focus-key` 优先，其次退回同位置，最后退回首个。
 */
export function restoreFocus(viewKey, root = activeScopeRoot()) {
  const focusables = collectFocusables(root)
  if (focusables.length === 0) return false

  const remembered = focusMemory.get(viewKey)
  if (remembered?.key) {
    const match = focusables.find((element) => focusIdentity(element) === remembered.key)
    if (match) return focusElement(match)
  }
  if (remembered && remembered.index >= 0 && remembered.index < focusables.length) {
    return focusElement(focusables[remembered.index])
  }
  return focusFirst(root)
}

export function clearFocusMemory(viewKey) {
  if (viewKey === undefined) focusMemory.clear()
  else focusMemory.delete(viewKey)
}

/**
 * 把一个模态框/抽屉接入焦点栈。
 *
 * `elementRef` 是面板根元素的 ref，`isOpen` 是返回开合状态的 getter。打开时压栈并
 * 把焦点移进面板，关闭时归还焦点。
 */
export function useModalFocusScope(elementRef, isOpen) {
  let dispose = null

  watch(
    [() => isOpen(), elementRef],
    async ([open, element]) => {
      if (open && element) {
        if (!dispose) {
          await nextTick()
          dispose = pushFocusScope(element)
        }
        return
      }
      dispose?.()
      dispose = null
    },
    { immediate: true, flush: 'post' }
  )

  onUnmounted(() => {
    dispose?.()
    dispose = null
  })
}
