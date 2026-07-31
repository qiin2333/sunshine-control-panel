import test from 'node:test'
import assert from 'node:assert/strict'

const deferred = () => {
  let resolve
  const promise = new Promise((next) => {
    resolve = next
  })
  return { promise, resolve }
}

const installDragEnvironment = (invoke) => {
  const callbacks = new Map()
  const listeners = new Map()
  let callbackId = 0
  const harness = { callbacks, listeners }

  globalThis.window = {
    innerWidth: 800,
    __TAURI_EVENT_PLUGIN_INTERNALS__: {
      unregisterListener() {},
    },
    __TAURI_INTERNALS__: {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: (callback) => {
        const id = ++callbackId
        callbacks.set(id, callback)
        return id
      },
      invoke: (command, args) => invoke(command, args, harness),
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

  return {
    callbacks,
    listeners,
    cleanup() {
      delete globalThis.window
      delete globalThis.document
      delete globalThis.requestAnimationFrame
      delete globalThis.cancelAnimationFrame
    },
  }
}

const pointerTarget = {
  setPointerCapture() {},
  hasPointerCapture() { return true },
  releasePointerCapture() {},
}

const startTouchDrag = (onTouchWindowDragStart, pointerId, clientX, clientY) => {
  onTouchWindowDragStart({
    pointerType: 'touch',
    isPrimary: true,
    button: 0,
    pointerId,
    clientX,
    clientY,
    currentTarget: pointerTarget,
    preventDefault() {},
    stopPropagation() {},
  })
}

const moveTouchDrag = (listeners, pointerId, clientX, clientY) => {
  listeners.get('pointermove')({
    pointerType: 'touch',
    pointerId,
    clientX,
    clientY,
    preventDefault() {},
  })
}

const nextTask = () => new Promise((resolve) => setTimeout(resolve, 0))

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

test('releases touch drag state when the initial window query never finishes', async () => {
  const never = new Promise(() => {})
  const committedPositions = []
  const listeners = new Map()
  let outerPositionCalls = 0
  let callbackId = 0

  globalThis.window = {
    innerWidth: 800,
    __TAURI_INTERNALS__: {
      metadata: { currentWindow: { label: 'main' } },
      transformCallback: () => ++callbackId,
      invoke: async (command, args) => {
        if (command === 'plugin:event|listen') return 1
        if (command === 'plugin:window|outer_position') {
          outerPositionCalls += 1
          return outerPositionCalls === 1 ? never : { x: 100, y: 200 }
        }
        if (command === 'plugin:window|is_maximized') return false
        if (command === 'plugin:window|scale_factor') return 2
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
    const startDrag = (pointerId, clientX, clientY) => {
      onTouchWindowDragStart({
        pointerType: 'touch',
        isPrimary: true,
        button: 0,
        pointerId,
        clientX,
        clientY,
        currentTarget: pointerTarget,
        preventDefault() {},
        stopPropagation() {},
      })
    }

    startDrag(7, 10, 20)
    listeners.get('pointermove')({
      pointerType: 'touch',
      pointerId: 7,
      clientX: 35,
      clientY: 50,
      preventDefault() {},
    })
    await listeners.get('pointerup')({ pointerId: 7 })
    assert.deepEqual(committedPositions, [])

    startDrag(8, 20, 30)
    listeners.get('pointermove')({
      pointerType: 'touch',
      pointerId: 8,
      clientX: 45,
      clientY: 60,
      preventDefault() {},
    })
    await listeners.get('pointerup')({ pointerId: 8 })

    assert.deepEqual(committedPositions, [{ x: 150, y: 260 }])
  } finally {
    console.warn = originalWarn
    delete globalThis.window
    delete globalThis.document
    delete globalThis.requestAnimationFrame
    delete globalThis.cancelAnimationFrame
  }
})

test('releases touch drag state when restoring a maximized window never finishes', async () => {
  const never = new Promise(() => {})
  const committedPositions = []
  let maximizedCalls = 0
  const environment = installDragEnvironment(async (command, args, { callbacks }) => {
    if (command === 'plugin:event|listen') {
      if (args.event === 'tauri://resize') {
        queueMicrotask(() => callbacks.get(args.handler)?.({ payload: {} }))
      }
      return 1
    }
    if (command === 'plugin:event|unlisten') return null
    if (command === 'plugin:window|outer_position') return { x: 100, y: 200 }
    if (command === 'plugin:window|is_maximized') {
      maximizedCalls += 1
      return maximizedCalls === 1
    }
    if (command === 'plugin:window|scale_factor') return 2
    if (command === 'plugin:window|unmaximize') return null
    if (command === 'plugin:window|outer_size') return never
    if (command === 'plugin:window|set_position') {
      committedPositions.push(args.value.toJSON().Physical)
      return null
    }
    throw new Error(`Unexpected Tauri command: ${command}`)
  })

  const originalWarn = console.warn
  console.warn = () => {}
  try {
    const { useTouchWindowDrag } = await import('./useTouchWindowDrag.js')
    const { onTouchWindowDragStart } = useTouchWindowDrag()

    startTouchDrag(onTouchWindowDragStart, 7, 10, 20)
    await nextTask()
    moveTouchDrag(environment.listeners, 7, 35, 50)
    await environment.listeners.get('pointerup')({ pointerId: 7 })
    assert.deepEqual(committedPositions, [])

    startTouchDrag(onTouchWindowDragStart, 8, 20, 30)
    moveTouchDrag(environment.listeners, 8, 45, 60)
    await environment.listeners.get('pointerup')({ pointerId: 8 })
    assert.deepEqual(committedPositions, [{ x: 150, y: 260 }])
  } finally {
    console.warn = originalWarn
    environment.cleanup()
  }
})

test('releases touch drag state when DPI rebasing never finishes', async () => {
  const never = new Promise(() => {})
  const committedPositions = []
  let outerPositionCalls = 0
  let scaleChangeHandler = null
  const environment = installDragEnvironment(async (command, args, { callbacks }) => {
    if (command === 'plugin:event|listen') {
      if (args.event === 'tauri://scale-change') {
        scaleChangeHandler = callbacks.get(args.handler)
      }
      return 1
    }
    if (command === 'plugin:event|unlisten') return null
    if (command === 'plugin:window|outer_position') {
      outerPositionCalls += 1
      return outerPositionCalls === 2 ? never : { x: 100, y: 200 }
    }
    if (command === 'plugin:window|is_maximized') return false
    if (command === 'plugin:window|scale_factor') return 2
    if (command === 'plugin:window|set_position') {
      committedPositions.push(args.value.toJSON().Physical)
      return null
    }
    throw new Error(`Unexpected Tauri command: ${command}`)
  })

  const originalWarn = console.warn
  console.warn = () => {}
  try {
    const { useTouchWindowDrag } = await import('./useTouchWindowDrag.js')
    const { onTouchWindowDragStart } = useTouchWindowDrag()

    startTouchDrag(onTouchWindowDragStart, 7, 10, 20)
    await nextTask()
    moveTouchDrag(environment.listeners, 7, 35, 50)
    await nextTask()
    assert.equal(typeof scaleChangeHandler, 'function')
    scaleChangeHandler({ payload: { scaleFactor: 1.5 } })
    await nextTask()
    await environment.listeners.get('pointerup')({ pointerId: 7 })

    const commitsBeforeRetry = committedPositions.length
    startTouchDrag(onTouchWindowDragStart, 8, 20, 30)
    moveTouchDrag(environment.listeners, 8, 45, 60)
    await environment.listeners.get('pointerup')({ pointerId: 8 })

    assert.equal(committedPositions.length, commitsBeforeRetry + 1)
    assert.deepEqual(committedPositions.at(-1), { x: 150, y: 260 })
  } finally {
    console.warn = originalWarn
    environment.cleanup()
  }
})

test('releases touch drag state when a position commit never finishes', async () => {
  const never = new Promise(() => {})
  const committedPositions = []
  let setPositionCalls = 0
  const environment = installDragEnvironment(async (command, args) => {
    if (command === 'plugin:event|listen') return 1
    if (command === 'plugin:event|unlisten') return null
    if (command === 'plugin:window|outer_position') return { x: 100, y: 200 }
    if (command === 'plugin:window|is_maximized') return false
    if (command === 'plugin:window|scale_factor') return 2
    if (command === 'plugin:window|set_position') {
      setPositionCalls += 1
      if (setPositionCalls === 1 || setPositionCalls === 3) return never
      committedPositions.push(args.value.toJSON().Physical)
      return null
    }
    throw new Error(`Unexpected Tauri command: ${command}`)
  })

  const originalWarn = console.warn
  console.warn = () => {}
  try {
    const { useTouchWindowDrag } = await import('./useTouchWindowDrag.js')
    const { onTouchWindowDragStart } = useTouchWindowDrag()

    startTouchDrag(onTouchWindowDragStart, 7, 10, 20)
    await nextTask()
    moveTouchDrag(environment.listeners, 7, 35, 50)
    await nextTask()
    await environment.listeners.get('pointerup')({ pointerId: 7 })

    startTouchDrag(onTouchWindowDragStart, 8, 20, 30)
    moveTouchDrag(environment.listeners, 8, 45, 60)
    await environment.listeners.get('pointerup')({ pointerId: 8 })
    assert.deepEqual(committedPositions, [{ x: 150, y: 260 }])

    startTouchDrag(onTouchWindowDragStart, 9, 10, 20)
    moveTouchDrag(environment.listeners, 9, 35, 50)
    await environment.listeners.get('pointerup')({ pointerId: 9 })

    startTouchDrag(onTouchWindowDragStart, 10, 20, 30)
    moveTouchDrag(environment.listeners, 10, 45, 60)
    await environment.listeners.get('pointerup')({ pointerId: 10 })
    assert.deepEqual(committedPositions, [
      { x: 150, y: 260 },
      { x: 150, y: 260 },
    ])
  } finally {
    console.warn = originalWarn
    environment.cleanup()
  }
})
