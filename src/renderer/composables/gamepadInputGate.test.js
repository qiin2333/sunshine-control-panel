import test from 'node:test'
import assert from 'node:assert/strict'
import { createGamepadInputGate } from './gamepadInputGate.js'
const pad = (buttons = [], index = 0) => ({ index, mapping: 'standard', buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: buttons.includes(i) })) })
const edge = (index, pressed) => ({ index, pressed })
const options = { capturing: false, bindings: ['LB+A', ''] }
function setup() { const gate = createGamepadInputGate(); gate.update([pad()], 0, [], options); return gate }
test('LB+A never leaks navigation, including its releases', () => {
  const gate = setup()
  assert.equal(gate.update([pad([4])], 0, [edge(4, true)], options).blocked, true)
  assert.equal(gate.update([pad([4, 0])], 0, [edge(0, true)], options).blocked, true)
  assert.equal(gate.update([pad([0])], 0, [edge(4, false)], options).blocked, true)
  assert.deepEqual(gate.update([pad()], 0, [edge(0, false)], options).edges, [])
})
test('a reserved button used alone still performs its navigation action on release', () => {
  const gate = setup()
  gate.update([pad([4])], 0, [edge(4, true)], options)
  assert.deepEqual(gate.update([pad()], 0, [edge(4, false)], options).edges, [edge(4, true), edge(4, false)])
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
  assert.equal(gate.update([pad([4]), pad([0], 1)], 1, [edge(0, true)], options).blocked, true)
  assert.deepEqual(gate.update([pad(), pad([], 1)], 1, [], options).edges, [])
  gate.update([pad([4])], 0, [edge(4, true)], options)
  assert.deepEqual(gate.update([pad()], 0, [edge(4, false)], { ...options, bindings: [] }).edges, [])
})
