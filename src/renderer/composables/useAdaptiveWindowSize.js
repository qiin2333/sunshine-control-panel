import { nextTick, onMounted, onUnmounted, unref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createAdaptiveSizeObserver, measureElementHeight } from '../shared/adaptive-window-size.js'

const readValue = (value, fallback) => {
  const resolved = unref(value)
  return resolved == null ? fallback : resolved
}

export function useAdaptiveWindowSize(targetRef, options = {}) {
  let disposed = false
  let generation = 0
  let sizeObserver = null
  let animationFrame = 0
  let animatedWidth = 0
  let animatedHeight = 0
  let lastAppliedWidth = 0
  let lastAppliedHeight = 0

  const invokeResize = async (width, height, requestGeneration) => {
    const nextWidth = Math.ceil(width)
    const nextHeight = Math.ceil(height)
    if (nextWidth === lastAppliedWidth && nextHeight === lastAppliedHeight) {
      return
    }

    while (!disposed && generation === requestGeneration) {
      const applied = options.isDragging?.() ? false
        : await invoke('resize_tool_window', { width: nextWidth, height: nextHeight })
      if (disposed || generation !== requestGeneration) return
      if (applied !== false) {
        lastAppliedWidth = nextWidth
        lastAppliedHeight = nextHeight
        return
      }
      await new Promise(resolve => setTimeout(resolve, 100))
    }
  }

  const stopAnimation = () => {
    ++generation
    if (animationFrame) {
      cancelAnimationFrame(animationFrame)
      animationFrame = 0
    }
  }

  const measureWindowSize = async (element) => {
    await nextTick()

    const width = readValue(options.width, 380)
    const compactHeight = readValue(options.height, null)
    const minHeight = readValue(options.minHeight, 180)
    const maxHeight = readValue(options.maxHeight, Number.POSITIVE_INFINITY)
    const measuredHeight = compactHeight ?? (measureElementHeight(element) + readValue(options.extraHeight, 0))
    const height = Math.max(minHeight, Math.min(Math.ceil(measuredHeight), maxHeight))

    return { width, height }
  }

  const applyWindowSize = async ({ width, height }) => {
    try {
      if (!readValue(options.animate, false) || !animatedWidth || !animatedHeight) {
        stopAnimation()
        animatedWidth = width
        animatedHeight = height
        await invokeResize(width, height, generation)
        return
      }

      const fromWidth = animatedWidth
      const fromHeight = animatedHeight
      const start = performance.now()
      const duration = readValue(options.animationDuration, 160)
      stopAnimation()

      const requestGeneration = generation
      const animate = async (now) => {
        if (disposed || generation !== requestGeneration) return
        try {
          const progress = Math.min(1, (now - start) / duration)
          const eased = 1 - Math.pow(1 - progress, 3)
          const nextWidth = fromWidth + (width - fromWidth) * eased
          const nextHeight = fromHeight + (height - fromHeight) * eased

          animatedWidth = nextWidth
          animatedHeight = nextHeight
          await invokeResize(nextWidth, nextHeight, requestGeneration)
          if (disposed || generation !== requestGeneration) return

          if (progress < 1) {
            animationFrame = requestAnimationFrame(animate)
          } else {
            animationFrame = 0
            animatedWidth = width
            animatedHeight = height
          }
        } catch (_) { animationFrame = 0 }
      }

      animationFrame = requestAnimationFrame(animate)
    } catch (_) {
      // Browser preview does not expose the Tauri window API.
    }
  }

  const syncWindowSize = async () => {
    await nextTick()
    await sizeObserver?.sync()
  }

  const scheduleSyncWindowSize = () => {
    sizeObserver?.schedule()
  }

  onMounted(() => {
    sizeObserver = createAdaptiveSizeObserver({
      enabled: () => readValue(options.enabled, true),
      getElement: () => unref(targetRef),
      measureSize: measureWindowSize,
      applySize: applyWindowSize,
    })
    sizeObserver.start()
  })

  onUnmounted(() => {
    disposed = true
    stopAnimation()
    sizeObserver?.stop()
    sizeObserver = null
  })

  return {
    syncWindowSize,
    scheduleSyncWindowSize,
  }
}
