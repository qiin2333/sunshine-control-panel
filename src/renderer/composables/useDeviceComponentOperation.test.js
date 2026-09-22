import test from 'node:test'
import assert from 'node:assert/strict'
import { nextTick } from 'vue'
import { useDeviceComponentOperation } from './useDeviceComponentOperation.js'

test('shared driver operations cannot overlap across managers or replacement tabs', async () => {
  const sidecar = useDeviceComponentOperation()
  const transport = useDeviceComponentOperation()
  let release
  const blocked = new Promise(resolve => { release = resolve })
  const install = sidecar.wrap(() => blocked)
  let cleaned = false
  const cleanup = transport.wrap(() => { cleaned = true })
  const pending = install()
  assert.equal(transport.busyElsewhere.value, true)
  await cleanup()
  assert.equal(cleaned, false)
  const replacement = useDeviceComponentOperation()
  assert.equal(replacement.busyElsewhere.value, true)
  release()
  await pending
  assert.equal(replacement.busyElsewhere.value, false)
  await cleanup()
  assert.equal(cleaned, true)
})

test('cancelled operations release the lock without refreshing managers', async () => {
  const first = useDeviceComponentOperation()
  const second = useDeviceComponentOperation()
  let refreshed = 0
  const stop = second.onOtherChange(() => { refreshed++ })
  try {
    await assert.rejects(first.wrap(async () => { throw new Error('cancelled') })())
    await nextTick()
    assert.equal(second.busyElsewhere.value, false)
    assert.equal(refreshed, 0)
  } finally { stop() }
})

test('a mutation refreshes peers once, without refreshing its own manager', async () => {
  const first = useDeviceComponentOperation()
  const second = useDeviceComponentOperation()
  let own = 0
  let peer = 0
  const stopOwn = first.onOtherChange(() => { own++ })
  const stopPeer = second.onOtherChange(() => { peer++ })
  try {
    await first.wrap(async () => { first.markChanged() })()
    await nextTick()
    assert.equal(own, 0)
    assert.equal(peer, 1)
    // Even a failed mutation can leave partial changes that peers must inspect.
    await assert.rejects(first.wrap(async () => {
      first.markChanged()
      throw new Error('partially installed')
    })())
    await nextTick()
    assert.equal(own, 0)
    assert.equal(peer, 2)
    assert.equal(second.busyElsewhere.value, false)
  } finally {
    stopOwn()
    stopPeer()
  }
})
