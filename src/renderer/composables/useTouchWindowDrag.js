import { onUnmounted } from 'vue'
import { PhysicalPosition } from '@tauri-apps/api/dpi'
import { getCurrentWindow } from '@tauri-apps/api/window'

const DRAG_THRESHOLD = 4
const DRAG_OPERATION_TIMEOUT_MS = 1000
const RESTORE_RESIZE_TIMEOUT_MS = 500

/**
 * Adds touch dragging to a custom Tauri title bar.
 *
 * Mouse dragging remains handled by `data-tauri-drag-region`. WebView2 touch
 * coordinates are viewport-relative while the window is moving, so touch
 * dragging uses the current window position and applies incremental updates.
 *
 * @param {{ value: boolean } | null} isMaximized reactive window state used by
 * the custom maximize button
 */
export function useTouchWindowDrag(isMaximized = null) {
  let appWindow = null
  let unlistenScaleChanged = null
  let scaleListenerPromise = null
  let disposed = false
  let dragGeneration = 0
  let pointerId = null
  let pointerTarget = null
  let hasMoved = false
  let dragEnding = false

  let startClientX = 0
  let startClientY = 0
  let latestClientX = 0
  let latestClientY = 0

  let basePhysicalX = Number.NaN
  let basePhysicalY = Number.NaN
  let pendingPhysicalX = Number.NaN
  let pendingPhysicalY = Number.NaN
  let activeScaleFactor = 1
  let scaleFactorVersion = 0
  let scaleRebasing = false
  let scaleRebasePromise = null

  let initialPosition = null
  let initialMaximized = false
  let initialized = false
  let initialPreparationPromise = null
  let preparing = false
  let preparationPromise = null
  let settingPosition = false
  let positionRaf = null

  const normalizeScaleFactor = (value) => (
    Number.isFinite(value) && value > 0 ? value : 1
  )

  const isCurrentDrag = (generation, activePointerId) => (
    !disposed &&
    generation === dragGeneration &&
    pointerId === activePointerId
  )

  const getAppWindow = () => {
    if (appWindow) return appWindow

    try {
      appWindow = getCurrentWindow()
      return appWindow
    } catch {
      return null
    }
  }

  const commitPosition = (physicalX, physicalY) => {
    return appWindow.setPosition(new PhysicalPosition(
      Math.round(physicalX),
      Math.round(physicalY),
    ))
  }

  const waitForRestoreResize = async () => {
    let resolveResize
    let unlistenResize = null
    let timeoutId = null
    const resizePromise = new Promise((resolve) => {
      resolveResize = resolve
    })

    try {
      try {
        unlistenResize = await appWindow.onResized(() => resolveResize())
      } catch {
        // Fall back to the bounded delay if resize events are unavailable.
      }

      await appWindow.unmaximize()
      if (!disposed && isMaximized && typeof isMaximized === 'object' && 'value' in isMaximized) {
        isMaximized.value = false
      }
      await Promise.race([
        resizePromise,
        new Promise((resolve) => {
          timeoutId = setTimeout(resolve, RESTORE_RESIZE_TIMEOUT_MS)
        }),
      ])
    } finally {
      if (timeoutId !== null) {
        clearTimeout(timeoutId)
      }
      if (unlistenResize) {
        unlistenResize()
      }
    }
  }

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
    dragGeneration += 1
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
    initialPreparationPromise = null
    preparing = false
    preparationPromise = null
    settingPosition = false
    scaleRebasing = false
    scaleRebasePromise = null
    basePhysicalX = Number.NaN
    basePhysicalY = Number.NaN
    pendingPhysicalX = Number.NaN
    pendingPhysicalY = Number.NaN
  }

  const rebaseForScaleChange = (scaleFactor) => {
    scaleFactorVersion += 1
    activeScaleFactor = normalizeScaleFactor(scaleFactor)
    if (
      pointerId === null ||
      dragEnding ||
      !initialized ||
      scaleRebasePromise
    ) {
      return
    }

    const activePointerId = pointerId
    const generation = dragGeneration
    scaleRebasing = true
    if (positionRaf !== null) {
      cancelAnimationFrame(positionRaf)
      positionRaf = null
    }

    let rebasePromise
    rebasePromise = (async () => {
      while (settingPosition) {
        await new Promise((resolve) => setTimeout(resolve, 0))
        if (!isCurrentDrag(generation, activePointerId)) return
      }

      // Let WebView2 update viewport-relative pointer coordinates for the new DPI.
      await new Promise((resolve) => requestAnimationFrame(resolve))
      if (!isCurrentDrag(generation, activePointerId) || dragEnding) return

      const position = await appWindow.outerPosition()
      if (!isCurrentDrag(generation, activePointerId) || dragEnding) return

      basePhysicalX = position.x
      basePhysicalY = position.y
      pendingPhysicalX = position.x
      pendingPhysicalY = position.y
      startClientX = latestClientX
      startClientY = latestClientY
    })()
      .catch(() => {
        if (isCurrentDrag(generation, activePointerId)) {
          clearDragState()
        }
      })
      .finally(() => {
        if (isCurrentDrag(generation, activePointerId)) {
          scaleRebasing = false
        }
        if (scaleRebasePromise === rebasePromise) {
          scaleRebasePromise = null
        }
      })
    scaleRebasePromise = rebasePromise
  }

  const ensureScaleChangeListener = () => {
    if (disposed || unlistenScaleChanged || scaleListenerPromise) return

    scaleListenerPromise = appWindow
      .onScaleChanged(({ payload }) => {
        rebaseForScaleChange(payload.scaleFactor)
      })
      .then((unlisten) => {
        scaleListenerPromise = null
        if (disposed) {
          unlisten()
          return
        }
        unlistenScaleChanged = unlisten
      })
      .catch(() => {
        scaleListenerPromise = null
      })
  }

  const applyPendingPosition = async () => {
    positionRaf = null
    if (
      pointerId === null ||
      dragEnding ||
      preparing ||
      scaleRebasing ||
      settingPosition ||
      !Number.isFinite(pendingPhysicalX) ||
      !Number.isFinite(pendingPhysicalY)
    ) {
      return
    }

    settingPosition = true
    const generation = dragGeneration
    const activePointerId = pointerId
    const nextPhysicalX = pendingPhysicalX
    const nextPhysicalY = pendingPhysicalY

    try {
      await commitPosition(nextPhysicalX, nextPhysicalY)
      if (!isCurrentDrag(generation, activePointerId)) return

      basePhysicalX = nextPhysicalX
      basePhysicalY = nextPhysicalY
    } catch {
      if (isCurrentDrag(generation, activePointerId)) {
        clearDragState()
      }
    } finally {
      if (isCurrentDrag(generation, activePointerId)) {
        settingPosition = false
      }
    }
  }

  const updatePendingPosition = () => {
    if (
      !initialized ||
      !Number.isFinite(basePhysicalX) ||
      !Number.isFinite(basePhysicalY)
    ) return false

    pendingPhysicalX = basePhysicalX + (latestClientX - startClientX) * activeScaleFactor
    pendingPhysicalY = basePhysicalY + (latestClientY - startClientY) * activeScaleFactor
    return Number.isFinite(pendingPhysicalX) && Number.isFinite(pendingPhysicalY)
  }

  const waitForDragOperation = async (promise) => {
    let timeoutId = null
    try {
      return await Promise.race([
        promise.then(() => true, () => false),
        new Promise((resolve) => {
          timeoutId = setTimeout(() => resolve(false), DRAG_OPERATION_TIMEOUT_MS)
        }),
      ])
    } finally {
      if (timeoutId !== null) {
        clearTimeout(timeoutId)
      }
    }
  }

  const waitForCurrentDragOperation = async (promise, generation, activePointerId) => {
    const completed = await waitForDragOperation(promise)
    if (!isCurrentDrag(generation, activePointerId)) return false
    if (!completed) clearDragState()
    return completed
  }

  const queueLatestPosition = () => {
    if (
      pointerId === null ||
      dragEnding ||
      preparing ||
      scaleRebasing ||
      settingPosition ||
      !updatePendingPosition()
    ) {
      return
    }

    if (positionRaf === null) {
      positionRaf = requestAnimationFrame(applyPendingPosition)
    }
  }

  const restoreForTouchDrag = () => {
    if (preparationPromise) return preparationPromise

    preparing = true
    const generation = dragGeneration
    const activePointerId = pointerId
    preparationPromise = (async () => {
      const scaleFactor = activeScaleFactor
      const pointerPhysicalX = initialPosition.x + latestClientX * scaleFactor
      const pointerPhysicalY = initialPosition.y + latestClientY * scaleFactor
      const horizontalAnchorRatio = Math.min(
        1,
        Math.max(0, startClientX / Math.max(window.innerWidth, 1)),
      )

      await waitForRestoreResize()
      if (!isCurrentDrag(generation, activePointerId)) return

      const restoredSize = await appWindow.outerSize()
      if (!isCurrentDrag(generation, activePointerId)) return

      const anchorPhysicalX = restoredSize.width * horizontalAnchorRatio
      const anchorPhysicalY = Math.min(
        restoredSize.height,
        Math.max(0, startClientY * scaleFactor),
      )
      const restoredPhysicalX = Math.round(pointerPhysicalX - anchorPhysicalX)
      const restoredPhysicalY = Math.round(pointerPhysicalY - anchorPhysicalY)

      await commitPosition(restoredPhysicalX, restoredPhysicalY)
      if (!isCurrentDrag(generation, activePointerId)) return

      basePhysicalX = restoredPhysicalX
      basePhysicalY = restoredPhysicalY
      pendingPhysicalX = basePhysicalX
      pendingPhysicalY = basePhysicalY
      startClientX = anchorPhysicalX / scaleFactor
      startClientY = anchorPhysicalY / scaleFactor
      initialized = true
      initialMaximized = false
    })()
      .catch(() => {
        if (isCurrentDrag(generation, activePointerId)) {
          clearDragState()
        }
      })
      .finally(() => {
        if (isCurrentDrag(generation, activePointerId)) {
          preparing = false
        }
      })

    return preparationPromise
  }

  const prepareWindowPosition = async (activePointerId, generation) => {
    try {
      const initialScaleFactorVersion = scaleFactorVersion
      const [position, maximized, scaleFactor] = await Promise.all([
        appWindow.outerPosition(),
        appWindow.isMaximized(),
        appWindow.scaleFactor(),
      ])

      if (!isCurrentDrag(generation, activePointerId)) return

      initialPosition = position
      initialMaximized = maximized
      if (scaleFactorVersion === initialScaleFactorVersion) {
        activeScaleFactor = normalizeScaleFactor(scaleFactor)
      }

      if (maximized) {
        if (hasMoved) {
          await restoreForTouchDrag()
        }
        return
      }

      basePhysicalX = position.x
      basePhysicalY = position.y
      pendingPhysicalX = basePhysicalX
      pendingPhysicalY = basePhysicalY
      initialized = true

      if (hasMoved) {
        if (dragEnding) {
          updatePendingPosition()
        } else {
          queueLatestPosition()
        }
      }
    } catch {
      if (isCurrentDrag(generation, activePointerId)) {
        clearDragState()
      }
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

    const generation = dragGeneration
    const activePointerId = pointerId
    dragEnding = true
    removeListeners()
    releasePointerCapture()

    if (positionRaf !== null) {
      cancelAnimationFrame(positionRaf)
      positionRaf = null
    }

    if (initialPreparationPromise && hasMoved) {
      if (!(await waitForCurrentDragOperation(
        initialPreparationPromise,
        generation,
        activePointerId,
      ))) return
    }

    if (preparationPromise) {
      if (!(await waitForCurrentDragOperation(
        preparationPromise,
        generation,
        activePointerId,
      ))) return
    }

    if (scaleRebasePromise) {
      if (!(await waitForCurrentDragOperation(
        scaleRebasePromise,
        generation,
        activePointerId,
      ))) return
    }

    if (settingPosition) {
      if (!(await waitForCurrentDragOperation(
        (async () => {
          while (settingPosition) {
            await new Promise((resolve) => setTimeout(resolve, 0))
          }
        })(),
        generation,
        activePointerId,
      ))) return
    }

    if (
      hasMoved &&
      initialized &&
      Number.isFinite(pendingPhysicalX) &&
      Number.isFinite(pendingPhysicalY)
    ) {
      if (!(await waitForCurrentDragOperation(
        commitPosition(pendingPhysicalX, pendingPhysicalY),
        generation,
        activePointerId,
      ))) return
    }

    if (isCurrentDrag(generation, activePointerId)) {
      clearDragState()
    }
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

    if (!getAppWindow()) return

    ensureScaleChangeListener()
    const generation = ++dragGeneration
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

    let initialPromise
    initialPromise = prepareWindowPosition(pointerId, generation)
      .finally(() => {
        if (initialPreparationPromise === initialPromise) {
          initialPreparationPromise = null
        }
      })
    initialPreparationPromise = initialPromise
  }

  onUnmounted(() => {
    disposed = true
    clearDragState()
    if (unlistenScaleChanged) {
      unlistenScaleChanged()
      unlistenScaleChanged = null
    }
  })

  return {
    onTouchWindowDragStart,
  }
}
