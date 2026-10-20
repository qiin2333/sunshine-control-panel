import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const source = readFileSync(new URL('./lifecycle.js', import.meta.url), 'utf8')

function page(generation) {
  const calls = [], timers = [], events = [], styles = []
  const head = {
    appendChild(style) { styles.push(style); style.parentNode = this },
    removeChild(style) { styles.splice(styles.indexOf(style), 1); style.parentNode = null },
  }
  const document = { head, documentElement: head,
    createElement: () => ({ parentNode: null }), dispatchEvent: event => events.push(event.type) }
  const window = { __TAURI_INTERNALS__: { invoke: (name, args) => calls.push({ name, args }) } }
  vm.runInNewContext(source.replaceAll('__LIFECYCLE_GENERATION__', String(generation)), {
    window, document, Event: class { constructor(type) { this.type = type } },
    console: { log() {} }, setInterval: (callback, delay) => timers.push({ callback, delay }),
  })
  return { window, document, calls, timers, events, styles }
}

test('initial and periodic heartbeats identify the window lifetime', () => {
  const old = page(7), current = page(8)
  old.timers[0].callback()
  current.timers[0].callback()
  assert.equal(old.timers[0].delay, 10000)
  for (const call of old.calls) {
    assert.equal(call.name, 'webview_heartbeat')
    assert.equal(call.args.generation, 7)
  }
  for (const call of current.calls) assert.equal(call.args.generation, 8)
})

test('visibility notifications pause and resume page content only on transitions', () => {
  const view = page(1)
  view.window.__setWebviewVisibility(false)
  view.window.__setWebviewVisibility(false)
  assert.equal(view.document.hidden, true)
  assert.equal(view.document.visibilityState, 'hidden')
  assert.equal(view.styles.length, 1)
  assert.match(view.styles[0].textContent, /animation-play-state: paused/)
  assert.deepEqual(view.events, ['visibilitychange'])
  view.window.__setWebviewVisibility(true)
  assert.equal(view.document.hidden, false)
  assert.equal(view.styles.length, 0)
  assert.deepEqual(view.events, ['visibilitychange', 'visibilitychange'])
})
