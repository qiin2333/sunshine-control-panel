import { onUnmounted } from 'vue'
import { PhysicalPosition } from '@tauri-apps/api/dpi'
import { getCurrentWindow } from '@tauri-apps/api/window'

const DRAG_THRESHOLD = 4

/**
 * Adds touch dragging to a custom Tauri title bar.
 *
 * Mouse dragging remains handled by `data-tauri-drag-region`. WebView2 touch
 * coordinates are viewport-relative while the window is moving, so touch
 * dragging uses the current window position and applies incremental updates.
 */
export function useTouchWindowDrag() {
  const appWindow = getCurrentWindow()

  let pointerId = null
  let pointerTarget = null
  let hasMoved = false
  let dragEnding = false

  let startClientX = 0
  let startClientY = 0
  let latestClientX = 0
  let latestClientY = 0

  let baseLogicalX = Number.NaN
  let baseLogicalY = Number.NaN
  let pendingLogicalX = Number.NaN
  let pendingLogicalY = Number.NaN

  let initialPosition = null
  let initialMaximized = false
  let initialized = false
  let preparing = false
  let preparationPromise = null
  let settingPosition = false
  let positionRaf = null

  const devicePixelRatio = () => window.devicePixelRatio || 1

  const releasePointerCapture = () => {
    if (pointerTarget && pointerId !== null) {
      try {
        if (pointerTarget.hasPointerCapture(pointerId)) {
          pointerTarget.releasePointerCapture(pointerId)
        }
      } catch {
        // The WebView may already have released capture after pointercancel.
      }
    }
    pointerTarget = null
  }

  const removeListeners = () => {
    document.removeEventListener('pointermove', onPointerMove)
    document.removeEventListener('pointerup', onPointerEnd)
    document.removeEventListener('pointercancel', onPointerEnd)
  }

  const clearDragState = () => {
    if (positionRaf !== null) {
      cancelAnimationFrame(positionRaf)
      positionRaf = null
    }
    removeListeners()
    releasePointerCapture()

    pointerId = null
    hasMoved = false
    dragEnding = false
    initialPosition = null
    initialMaximized = false
    initialized = false
    preparing = false
    preparationPromise = null
    settingPosition = false
    baseLogicalX = Number.NaN
    baseLogicalY = Number.NaN
    pendingLogicalX = Number.NaN
    pendingLogicalY = Number.NaN
  }

  const applyPendingPosition = async () => {
    positionRaf = null
    if (
      pointerId === null ||
      dragEnding ||
      preparing ||
      settingPosition ||
      !Number.isFinite(pendingLogicalX) ||
      !Number.isFinite(pendingLogicalY)
    ) {
      return
    }

    settingPosition = true
    const nextLogicalX = pendingLogicalX
    const nextLogicalY = pendingLogicalY
    const dpr = devicePixelRatio()

    try {
      await appWindow.setPosition(new PhysicalPosition(
        Math.round(nextLogicalX * dpr),
        Math.round(nextLogicalY * dpr),
      ))
      baseLogicalX = nextLogicalX
      baseLogicalY = nextLogicalY
    } catch {
      clearDragState()
    } finally {
      settingPosition = false
    }
  }

  const queueLatestPosition = () => {
    if (
      pointerId === null ||
      dragEnding ||
      preparing ||
      settingPosition ||
      !initialized ||
      !Number.isFinite(baseLogicalX) ||
      !Number.isFinite(baseLogicalY)
    ) {
      return
    }

    pendingLogicalX = baseLogicalX + (latestClientX - startClientX)
    pendingLogicalY = baseLogicalY + (latestClientY - startClientY)

    if (positionRaf === null) {
      positionRaf = requestAnimationFrame(applyPendingPosition)
    }
  }

  const restoreForTouchDrag = () => {
    if (preparationPromise) return preparationPromise

    preparing = true
    preparationPromise = (async () => {
      const dpr = devicePixelRatio()
      const pointerPhysicalX = initialPosition.x + latestClientX * dpr
      const pointerPhysicalY = initialPosition.y + latestClientY * dpr
      const horizontalAnchorRatio = Math.min(
        1,
        Math.max(0, startClientX / Math.max(window.innerWidth, 1)),
      )

      await appWindow.unmaximize()
      await new Promise((resolve) => requestAnimationFrame(resolve))

      const restoredSize = await appWindow.outerSize()
      const anchorPhysicalX = restoredSize.width * horizontalAnchorRatio
      const anchorPhysicalY = Math.min(
        restoredSize.height,
        Math.max(0, startClientY * dpr),
      )
      const restoredPhysicalX = Math.round(pointerPhysicalX - anchorPhysicalX)
      const restoredPhysicalY = Math.round(pointerPhysicalY - anchorPhysicalY)

      await appWindow.setPosition(new PhysicalPosition(
        restoredPhysicalX,
        restoredPhysicalY,
      ))

      baseLogicalX = restoredPhysicalX / dpr
      baseLogicalY = restoredPhysicalY / dpr
      pendingLogicalX = baseLogicalX
      pendingLogicalY = baseLogicalY
      startClientX = anchorPhysicalX / dpr
      startClientY = anchorPhysicalY / dpr
      initialized = true
      initialMaximized = false
    })()
      .catch(() => {
        clearDragState()
      })
      .finally(() => {
        preparing = false
      })

    return preparationPromise
  }

  const prepareWindowPosition = async (activePointerId) => {
    try {
      const [position, maximized] = await Promise.all([
        appWindow.outerPosition(),
        appWindow.isMaximized(),
      ])

      if (pointerId !== activePointerId || dragEnding) return

      initialPosition = position
      initialMaximized = maximized

      if (maximized) {
        if (hasMoved) {
          await restoreForTouchDrag()
        }
        return
      }

      const dpr = devicePixelRatio()
      baseLogicalX = position.x / dpr
      baseLogicalY = position.y / dpr
      pendingLogicalX = baseLogicalX
      pendingLogicalY = baseLogicalY
      initialized = true

      if (hasMoved) {
        queueLatestPosition()
      }
    } catch {
      clearDragState()
    }
  }

  function onPointerMove(event) {
    if (
      pointerId === null ||
      event.pointerId !== pointerId ||
      event.pointerType !== 'touch' ||
      dragEnding
    ) {
      return
    }

    latestClientX = event.clientX
    latestClientY = event.clientY

    const deltaX = latestClientX - startClientX
    const deltaY = latestClientY - startClientY
    if (
      !hasMoved &&
      Math.abs(deltaX) < DRAG_THRESHOLD &&
      Math.abs(deltaY) < DRAG_THRESHOLD
    ) {
      return
    }

    hasMoved = true
    event.preventDefault()

    if (!initialPosition) return
    if (initialMaximized && !initialized) {
      void restoreForTouchDrag()
      return
    }

    queueLatestPosition()
  }

  async function onPointerEnd(event) {
    if (pointerId === null || event.pointerId !== pointerId) return

    dragEnding = true
    removeListeners()
    releasePointerCapture()

    if (positionRaf !== null) {
      cancelAnimationFrame(positionRaf)
      positionRaf = null
    }

    if (preparationPromise) {
      await preparationPromise
    }

    while (settingPosition) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }

    if (
      hasMoved &&
      initialized &&
      Number.isFinite(pendingLogicalX) &&
      Number.isFinite(pendingLogicalY)
    ) {
      const dpr = devicePixelRatio()
      try {
        await appWindow.setPosition(new PhysicalPosition(
          Math.round(pendingLogicalX * dpr),
          Math.round(pendingLogicalY * dpr),
        ))
      } catch {
        // Keep the last position accepted by the operating system.
      }
    }

    clearDragState()
  }

  const onTouchWindowDragStart = (event) => {
    if (
      event.pointerType !== 'touch' ||
      !event.isPrimary ||
      event.button !== 0 ||
      pointerId !== null
    ) {
      return
    }

    event.preventDefault()
    event.stopPropagation()

    pointerId = event.pointerId
    pointerTarget = event.currentTarget
    hasMoved = false
    dragEnding = false
    startClientX = event.clientX
    startClientY = event.clientY
    latestClientX = event.clientX
    latestClientY = event.clientY

    try {
      pointerTarget.setPointerCapture(pointerId)
    } catch {
      // Document-level listeners still keep the drag active without capture.
    }

    document.addEventListener('pointermove', onPointerMove, { passive: false })
    document.addEventListener('pointerup', onPointerEnd)
    document.addEventListener('pointercancel', onPointerEnd)

    void prepareWindowPosition(pointerId)
  }

  onUnmounted(clearDragState)

  return {
    onTouchWindowDragStart,
  }
}
