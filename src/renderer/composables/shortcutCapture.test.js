import assert from 'node:assert/strict'
import test from 'node:test'
import { shortcutCapture } from './shortcutCapture.js'
const flush = () => new Promise(resolve => setImmediate(resolve))
test('cancellation waits for a pending start and never restores recording UI', async () => {
  const calls = [], changes = []
  let release
  const capture = shortcutCapture(active => {
    calls.push(active)
    if (active) return new Promise(resolve => { release = resolve })
  }, key => changes.push(key))
  const start = capture.start('nrShortcut')
  await flush()
  const cancel = capture.cancel()
  assert.deepEqual(calls, [false, true])
  release()
  await Promise.all([start, cancel])
  assert.deepEqual(calls, [false, true, false])
  assert.ok(changes.every(key => key === ''))
})
test('a newer start survives an older delayed completion', async () => {
  let release, delayed = true, selected = ''
  const capture = shortcutCapture(active => {
    if (active && delayed) { delayed = false; return new Promise(resolve => { release = resolve }) }
  }, key => { selected = key })
  const first = capture.start('nrShortcut')
  await flush()
  const second = capture.start('overlayShortcut')
  release()
  await Promise.all([first, second])
  assert.equal(selected, 'overlayShortcut')
  await capture.cancel()
})
test('cancellation before backend activation prevents the activation entirely', async () => {
  const calls = []
  const capture = shortcutCapture(active => { calls.push(active) }, () => {})
  const start = capture.start('nrShortcut')
  await capture.cancel()
  await start
  assert.ok(calls.every(active => !active))
})
