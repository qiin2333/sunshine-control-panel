import test from 'node:test'
import assert from 'node:assert/strict'
import { createFixedShortcutTracker } from './controllerButtons.js'

const bindings = ['LB+RB+X', 'LB+RB+Y']
const pad = (pressed = []) => ({
  mapping: 'standard',
  buttons: Array.from({ length: 16 }, (_, index) => ({ pressed: pressed.includes(index) }))
})

test('only a configured three-button hold of 500 ms consumes navigation', () => {
  const tracker = createFixedShortcutTracker()
  assert.equal(tracker.update(pad(), bindings, 0), false)
  assert.equal(tracker.update(pad([4, 5, 2]), bindings, 100), false)
  assert.equal(tracker.update(pad([4, 5, 2]), bindings, 599), false)
  assert.equal(tracker.update(pad([4, 5, 2]), bindings, 600), true)
  assert.equal(tracker.update(pad(), bindings, 610), true)
  assert.equal(tracker.update(pad(), bindings, 620), false)
  assert.equal(tracker.update(pad([4, 5, 2, 3]), bindings, 700), false)
})

test('short presses are not consumed; edits and pad switches discard held shortcuts', () => {
  const tracker = createFixedShortcutTracker()
  tracker.update(pad(), bindings, 0)
  tracker.update(pad([4, 5, 2]), bindings, 100)
  assert.equal(tracker.update(pad(), bindings, 300), false)
  tracker.update(pad([4]), bindings, 400)
  tracker.discard()
  assert.equal(tracker.update(pad(), bindings, 500), true)
  tracker.update(pad([4]), bindings, 600)
  assert.equal(tracker.update(pad([4]), [], 650), true)
  assert.equal(tracker.update(pad(), [], 700), true)
})

test('a controller button is never consumed when no shortcut is configured', () => {
  const tracker = createFixedShortcutTracker()
  assert.equal(tracker.update(pad([2]), [], 0), false)
  tracker.discard()
  assert.equal(tracker.update(pad(), [], 100), false)
})
