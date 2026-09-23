import test from 'node:test'
import assert from 'node:assert/strict'
import { createGamepadInputGate } from './gamepadInputGate.js'

const pad = (buttons = [], index = 0) => ({
  index, mapping: 'standard',
  buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: buttons.includes(i) }))
})
const edge = (index, pressed) => ({ index, pressed })
const options = { capturing: false, bindings: ['LB+RB+X', 'LB+RB+Y'] }
function setup() {
  const gate = createGamepadInputGate()
  gate.update([pad()], 0, [], options, 0)
  return gate
}

test('a full 500 ms shortcut consumes its buttons without freezing other navigation', () => {
  const gate = setup()
  assert.equal(gate.update([pad([4])], 0, [edge(4, true)], options, 0).blocked, false)
  gate.update([pad([4, 5])], 0, [edge(5, true)], options, 100)
  gate.update([pad([4, 5, 2])], 0, [edge(2, true)], options, 200)
  gate.update([pad([4, 5, 2])], 0, [], options, 700)
  assert.deepEqual(gate.update([pad()], 0, [edge(4, false), edge(5, false), edge(2, false)], options, 710).edges, [])
})

test('a short three-button press replays normal navigation', () => {
  const gate = setup()
  gate.update([pad([4, 5, 2])], 0, [edge(4, true), edge(5, true), edge(2, true)], options, 100)
  assert.deepEqual(
    gate.update([pad()], 0, [edge(4, false), edge(5, false), edge(2, false)], options, 400).edges,
    [edge(4, true), edge(5, true), edge(2, true), edge(4, false), edge(5, false), edge(2, false)]
  )
})

test('a shoulder button used alone navigates on release and B is unaffected', () => {
  const gate = setup()
  gate.update([pad([4])], 0, [edge(4, true)], options, 0)
  assert.deepEqual(gate.update([pad()], 0, [edge(4, false)], options, 700).edges, [edge(4, true), edge(4, false)])
  assert.deepEqual(gate.update([pad([1])], 0, [edge(1, true)], options, 800).edges, [edge(1, true)])
})

test('unrelated button releases are never held behind a shortcut candidate', () => {
  const gate = setup()
  gate.update([pad([4])], 0, [edge(4, true)], options, 0)
  assert.deepEqual(gate.update([pad([4, 1])], 0, [edge(1, true)], options, 10).edges, [edge(1, true)])
  assert.deepEqual(gate.update([pad([4])], 0, [edge(1, false)], options, 20).edges, [edge(1, false)])
  assert.deepEqual(gate.update([pad()], 0, [edge(4, false)], options, 30).edges, [edge(4, true), edge(4, false)])
})

test('recording in another window discards input until full release', () => {
  const gate = setup()
  gate.update([pad([1])], 0, [edge(1, true)], { ...options, capturing: true })
  assert.equal(gate.update([pad([1])], 0, [], options).blocked, true)
  assert.deepEqual(gate.update([pad()], 0, [edge(1, false)], options).edges, [])
  assert.deepEqual(gate.update([pad([1])], 0, [edge(1, true)], options).edges, [edge(1, true)])
})

test('controller switches and binding edits discard pending navigation', () => {
  const gate = setup()
  gate.update([pad([4])], 0, [edge(4, true)], options)
  assert.equal(gate.update([pad([4]), pad([2], 1)], 1, [edge(2, true)], options).blocked, true)
  assert.deepEqual(gate.update([pad(), pad([], 1)], 1, [], options).edges, [])
  gate.update([pad([4])], 0, [edge(4, true)], options)
  assert.deepEqual(gate.update([pad()], 0, [edge(4, false)], { ...options, bindings: [] }).edges, [])
})
