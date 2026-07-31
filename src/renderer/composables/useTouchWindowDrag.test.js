import test from 'node:test'
import assert from 'node:assert/strict'

const deferred = () => {
  let resolve
  const promise = new Promise((next) => {
    resolve = next
  })
  return { promise, resolve }
}

test('commits a quick touch drag after the initial window queries finish', async () => {
  const outerPosition = deferred()
  const maximized = deferred()
  const scaleFactor = deferred()
  const committedPositions = []
  const listeners = new Map()
  let callbackId = 0

  globalThis.window = {
    innerWidth: 800,
    __TAURI_INTERNALS__: {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => ++callbackId,
      invoke: async (command, args) => {
        if (command === 'plugin:event|listen') return 1
        if (command === 'plugin:window|outer_position') return outerPosition.promise
        if (command === 'plugin:window|is_maximized') return maximized.promise
        if (command === 'plugin:window|scale_factor') return scaleFactor.promise
        if (command === 'plugin:window|set_position') {
          committedPositions.push(args.value.toJSON().Physical)
          return null
        }
        throw new Error(`Unexpected Tauri command: ${command}`)
      },
    },
  }
  globalThis.document = {
    createElement() { return {} },
    addEventListener(type, listener) {
      listeners.set(type, listener)
    },
    removeEventListener(type, listener) {
      if (listeners.get(type) === listener) listeners.delete(type)
    },
  }
  globalThis.requestAnimationFrame = (callback) => {
    queueMicrotask(() => callback(0))
    return 1
  }
  globalThis.cancelAnimationFrame = () => {}

  const originalWarn = console.warn
  console.warn = () => {}
  try {
    const { useTouchWindowDrag } = await import('./useTouchWindowDrag.js')
    const { onTouchWindowDragStart } = useTouchWindowDrag()
    const pointerTarget = {
      setPointerCapture() {},
      hasPointerCapture() { return true },
      releasePointerCapture() {},
    }

    onTouchWindowDragStart({
      pointerType: 'touch',
      isPrimary: true,
      button: 0,
      pointerId: 7,
      clientX: 10,
      clientY: 20,
      currentTarget: pointerTarget,
      preventDefault() {},
      stopPropagation() {},
    })
    listeners.get('pointermove')({
      pointerType: 'touch',
      pointerId: 7,
      clientX: 35,
      clientY: 50,
      preventDefault() {},
    })

    const endPromise = listeners.get('pointerup')({ pointerId: 7 })
    assert.deepEqual(committedPositions, [])

    outerPosition.resolve({ x: 100, y: 200 })
    maximized.resolve(false)
    scaleFactor.resolve(2)
    await endPromise

    assert.deepEqual(committedPositions, [{ x: 150, y: 260 }])
  } finally {
    console.warn = originalWarn
    delete globalThis.window
    delete globalThis.document
    delete globalThis.requestAnimationFrame
    delete globalThis.cancelAnimationFrame
  }
})
