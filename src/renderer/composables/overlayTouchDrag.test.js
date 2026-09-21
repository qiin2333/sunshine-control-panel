import test from 'node:test'
import assert from 'node:assert/strict'
import { overlayTouchDrag } from './overlayTouchDrag.js'
const flush = () => new Promise(resolve => setImmediate(resolve))
function event(patch = {}) {
  return { pointerId: 1, pointerType: 'touch', isPrimary: true, button: 0,
    screenX: 100, screenY: 100, preventDefault() {},
    currentTarget: { setPointerCapture() {}, hasPointerCapture() { return true }, releasePointerCapture() {} }, ...patch }
}
test('touch movement uses screen coordinates and display scaling, preserving the final position', async () => {
  const positions = []
  const drag = overlayTouchDrag(async () => ({ position: { x: -400, y: 60 }, scale: 1.5 }), p => positions.push(p))
  await drag.start(event())
  drag.move(event({ screenX: 140, screenY: 120 }))
  drag.end(event({ screenX: 150, screenY: 130 }))
  await flush()
  assert.deepEqual(positions.at(-1), { x: -325, y: 105 })
  assert.equal(drag.active, false)
})
test('mouse and secondary contacts do not start touch dragging', async () => {
  let reads = 0
  const drag = overlayTouchDrag(() => { ++reads }, () => {})
  await drag.start(event({ pointerType: 'mouse' }))
  await drag.start(event({ isPrimary: false }))
  assert.equal(reads, 0)
})
test('cancellation and disposal invalidate a delayed position read', async () => {
  for (const dispose of [false, true]) {
    let resolve, moves = 0
    const drag = overlayTouchDrag(() => new Promise(r => { resolve = r }), () => { ++moves })
    const started = drag.start(event())
    await flush()
    drag.move(event({ screenX: 150 }))
    if (dispose) drag.dispose(); else drag.cancel(event())
    resolve({ position: { x: 0, y: 0 }, scale: 2 })
    await started
    assert.equal(moves, 0)
    assert.equal(drag.active, false)
  }
})
test('a second pointer cannot move or finish the captured gesture', async () => {
  const positions = []
  const drag = overlayTouchDrag(async () => ({ position: { x: 0, y: 0 }, scale: 1 }), p => positions.push(p))
  await drag.start(event({ pointerType: 'pen' }))
  drag.move(event({ pointerId: 2, screenX: 400 }))
  drag.end(event({ pointerId: 2 }))
  assert.equal(drag.active, true)
  assert.deepEqual(positions, [])
  drag.cancel()
})
