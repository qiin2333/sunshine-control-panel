import { nextTick, onMounted, onUnmounted, unref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createAdaptiveSizeObserver, measureElementHeight } from '../shared/adaptive-window-size.js'

const readValue = (value, fallback) => {
  const resolved = unref(value)
  return resolved == null ? fallback : resolved
}

export function useAdaptiveWindowSize(targetRef, options = {}) {
  let sizeObserver = null
  let animationFrame = 0
  let animatedWidth = 0
  let animatedHeight = 0
  let lastAppliedWidth = 0
  let lastAppliedHeight = 0

  const invokeResize = async (width, height) => {
    const nextWidth = Math.ceil(width)
    const nextHeight = Math.ceil(height)
    if (nextWidth === lastAppliedWidth && nextHeight === lastAppliedHeight) {
      return
    }

    lastAppliedWidth = nextWidth
    lastAppliedHeight = nextHeight
    await invoke('resize_tool_window', { width: nextWidth, height: nextHeight })
  }

  const stopAnimation = () => {
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
        await invokeResize(width, height)
        return
      }

      const fromWidth = animatedWidth
      const fromHeight = animatedHeight
      const start = performance.now()
      const duration = readValue(options.animationDuration, 160)
      stopAnimation()

      const animate = async (now) => {
        const progress = Math.min(1, (now - start) / duration)
        const eased = 1 - Math.pow(1 - progress, 3)
        const nextWidth = fromWidth + (width - fromWidth) * eased
        const nextHeight = fromHeight + (height - fromHeight) * eased

        animatedWidth = nextWidth
        animatedHeight = nextHeight
        await invokeResize(nextWidth, nextHeight)

        if (progress < 1) {
          animationFrame = requestAnimationFrame(animate)
        } else {
          animationFrame = 0
          animatedWidth = width
          animatedHeight = height
        }
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
    stopAnimation()
    sizeObserver?.stop()
    sizeObserver = null
  })

  return {
    syncWindowSize,
    scheduleSyncWindowSize,
  }
}
