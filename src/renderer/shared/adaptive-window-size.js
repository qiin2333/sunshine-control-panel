const readRuntimeValue = (value, fallback) => {
  if (typeof value === 'function') {
    const resolved = value()
    return resolved == null ? fallback : resolved
  }

  return value == null ? fallback : value
}

const normalizeSize = (size) => {
  if (!size) {
    return null
  }

  const width = Number.isFinite(size.width) ? Math.ceil(size.width) : null
  const height = Number.isFinite(size.height) ? Math.ceil(size.height) : null

  if (width == null && height == null) {
    return null
  }

  return { width, height }
}

export const measureElementHeight = (element) => {
  if (!element) {
    return 0
  }

  return Math.ceil(Math.max(element.scrollHeight, element.getBoundingClientRect().height))
}

export function createAdaptiveSizeObserver(options = {}) {
  let resizeObserver = null
  let pendingFrame = 0
  let lastWidth = null
  let lastHeight = null
  let running = false
  let rerunRequested = false
  let started = false

  const getElement = () => options.getElement?.() ?? null

  const applyIfChanged = async (size, element) => {
    const normalized = normalizeSize(size)
    if (!normalized) {
      return
    }

    const sameWidth = normalized.width == null || Math.abs(normalized.width - lastWidth) < 1
    const sameHeight = normalized.height == null || Math.abs(normalized.height - lastHeight) < 1
    if (sameWidth && sameHeight) {
      return
    }

    if (normalized.width != null) {
      lastWidth = normalized.width
    }
    if (normalized.height != null) {
      lastHeight = normalized.height
    }

    await options.applySize?.(normalized, element)
  }

  const sync = async () => {
    pendingFrame = 0

    if (readRuntimeValue(options.enabled, true) === false) {
      return
    }

    if (running) {
      rerunRequested = true
      return
    }

    running = true
    try {
      const element = getElement()
      if (!element) {
        return
      }

      const measured = await options.measureSize?.(element)
      await applyIfChanged(measured, element)
    } catch (error) {
      console.warn('Adaptive size sync failed:', error)
    } finally {
      running = false
      if (rerunRequested) {
        rerunRequested = false
        schedule()
      }
    }
  }

  const schedule = () => {
    if (typeof window === 'undefined') {
      return
    }

    if (pendingFrame) {
      window.cancelAnimationFrame(pendingFrame)
    }

    pendingFrame = window.requestAnimationFrame(sync)
  }

  const start = () => {
    if (started || typeof window === 'undefined') {
      return
    }

    started = true
    const element = getElement()

    if (element && 'ResizeObserver' in window) {
      resizeObserver = new ResizeObserver(schedule)
      resizeObserver.observe(element)
    }

    if (readRuntimeValue(options.observeWindowResize, true)) {
      window.addEventListener('resize', schedule)
    }

    schedule()
  }

  const stop = () => {
    if (pendingFrame && typeof window !== 'undefined') {
      window.cancelAnimationFrame(pendingFrame)
      pendingFrame = 0
    }

    if (resizeObserver) {
      resizeObserver.disconnect()
      resizeObserver = null
    }

    if (started && typeof window !== 'undefined' && readRuntimeValue(options.observeWindowResize, true)) {
      window.removeEventListener('resize', schedule)
    }

    started = false
  }

  return {
    start,
    stop,
    sync,
    schedule,
  }
}
