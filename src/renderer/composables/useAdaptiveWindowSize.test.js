import test from 'node:test'
import assert from 'node:assert/strict'
import { createRenderer, nextTick, ref } from 'vue'
import { useAdaptiveWindowSize } from './useAdaptiveWindowSize.js'
const tick = () => new Promise(resolve => setImmediate(resolve))
const renderer = createRenderer({
  createComment: () => ({}), createElement: () => ({}), createText: () => ({}),
  insert() {}, remove() {}, parentNode() {}, nextSibling() {}, setText() {}, setElementText() {}, patchProp() {}
})
function mountResize(invoke, options = {}) {
  let scheduled, controller
  globalThis.window = {
    __TAURI_INTERNALS__: { invoke },
    requestAnimationFrame: callback => { scheduled = callback; return 1 }, cancelAnimationFrame() {},
    addEventListener() {}, removeEventListener() {}
  }
  const app = renderer.createApp({
    setup() {
      controller = useAdaptiveWindowSize(ref({ scrollHeight: 240, getBoundingClientRect: () => ({ height: 240 }) }), { width: 340, ...options })
      return () => null
    }
  })
  app.mount({})
  return { app, start: () => scheduled(), controller }
}
test('native move defers automatic resizing until the backend accepts it', async () => {
  let calls = 0
  const { app, start } = mountResize(async command => {
    assert.equal(command, 'resize_tool_window')
    return ++calls > 1
  })
  try {
    start()
    await nextTick(); await tick()
    assert.equal(calls, 1)
    await new Promise(resolve => setTimeout(resolve, 130))
    assert.equal(calls, 2)
  } finally { app.unmount(); delete globalThis.window }
})
test('unmount stops native-move resize retries', async () => {
  let calls = 0
  const { app, start } = mountResize(async () => { ++calls; return false })
  start()
  await nextTick(); await tick()
  app.unmount()
  await new Promise(resolve => setTimeout(resolve, 130))
  assert.equal(calls, 1)
  delete globalThis.window
})

test('fallback touch dragging pauses size requests without treating them as applied', async () => {
  let dragging = true, calls = 0
  const { app, start } = mountResize(async () => { ++calls; return true }, { isDragging: () => dragging })
  try {
    start()
    await nextTick(); await tick()
    assert.equal(calls, 0)
    dragging = false
    await new Promise(resolve => setTimeout(resolve, 130))
    assert.equal(calls, 1)
  } finally { app.unmount(); delete globalThis.window }
})
