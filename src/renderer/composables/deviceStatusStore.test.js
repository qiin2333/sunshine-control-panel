import test from 'node:test'
import assert from 'node:assert/strict'
import { createStatusChannel } from './deviceStatusStore.js'

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

test('concurrent fetches share one backend call', async () => {
  let calls = 0
  const channel = createStatusChannel(async () => {
    calls += 1
    await sleep(5)
    return { success: true, data: { n: calls } }
  }, 50)
  const [a, b, c] = await Promise.all([channel.fetch(), channel.fetch(), channel.fetch()])
  assert.equal(calls, 1)
  assert.equal(a.data.n, 1)
  assert.equal(b.data.n, 1)
  assert.deepEqual(a, b)
})

test('successful results are reused within the TTL', async () => {
  let calls = 0
  const channel = createStatusChannel(async () => {
    calls += 1
    return { success: true, data: { n: calls } }
  }, 50)
  await channel.fetch()
  const second = await channel.fetch()
  assert.equal(calls, 1)
  assert.equal(second.cached, true)
  assert.equal(second.data.n, 1)
})

test('force bypasses a fresh cache', async () => {
  let calls = 0
  const channel = createStatusChannel(async () => {
    calls += 1
    return { success: true, data: { n: calls } }
  }, 50000)
  await channel.fetch()
  const forced = await channel.fetch({ force: true })
  assert.equal(calls, 2)
  assert.equal(forced.cached, undefined)
})

test('invalidate makes the next plain fetch refetch', async () => {
  let calls = 0
  const channel = createStatusChannel(async () => {
    calls += 1
    return { success: true, data: { n: calls } }
  }, 50000)
  await channel.fetch()
  channel.invalidate()
  await channel.fetch()
  assert.equal(calls, 2)
})

test('prime seeds the cache and satisfies plain fetches', async () => {
  let calls = 0
  const channel = createStatusChannel(async () => {
    calls += 1
    return { success: true, data: { n: calls } }
  }, 50000)
  channel.prime({ success: true, data: { n: 42 } })
  const result = await channel.fetch()
  assert.equal(calls, 0)
  assert.equal(result.data.n, 42)
  assert.equal(result.cached, true)
  channel.invalidate()
  const refetched = await channel.fetch()
  assert.equal(calls, 1)
  assert.equal(refetched.data.n, 1)
})

test('failed probes are retried on the next plain fetch', async () => {
  let calls = 0
  const channel = createStatusChannel(async () => {
    calls += 1
    return calls === 1
      ? { success: false, message: 'boom' }
      : { success: true, data: { n: calls } }
  }, 50000)
  const failed = await channel.fetch()
  assert.equal(failed.success, false)
  const retried = await channel.fetch()
  assert.equal(calls, 2)
  assert.equal(retried.success, true)
})

test('expired cache refetches instead of returning stale data', async () => {
  let calls = 0
  const channel = createStatusChannel(async () => {
    calls += 1
    return { success: true, data: { n: calls } }
  }, 5)
  await channel.fetch()
  await sleep(10)
  const result = await channel.fetch()
  assert.equal(calls, 2)
  assert.equal(result.data.n, 2)
})

test('invalidate during an in-flight fetch starts a fresh call; stale result never wins the cache', async () => {
  let calls = 0
  let resolveFirst
  const channel = createStatusChannel(() => {
    calls += 1
    if (calls === 1) {
      return new Promise((resolve) => { resolveFirst = resolve })
    }
    return Promise.resolve({ success: true, data: { n: calls, tag: 'post' } })
  }, 50000)
  const first = channel.fetch()
  await sleep(1)
  channel.invalidate()
  const second = channel.fetch()
  await sleep(1)
  resolveFirst({ success: true, data: { n: 1, tag: 'pre' } })
  const firstResult = await first
  const secondResult = await second
  assert.equal(calls, 2)
  assert.equal(firstResult.data.tag, 'pre')
  assert.equal(secondResult.data.tag, 'post')
  const third = await channel.fetch()
  assert.equal(third.cached, true)
  assert.equal(third.data.tag, 'post')
})

test('prime advances the epoch so an in-flight probe cannot overwrite the primed result', async () => {
  let calls = 0
  let resolveProbe
  const channel = createStatusChannel(() => {
    calls += 1
    if (calls === 1) return new Promise((resolve) => { resolveProbe = resolve })
    return Promise.resolve({ success: true, data: { n: calls } })
  }, 50000)
  const probe = channel.fetch()
  await sleep(1)
  channel.prime({ success: true, data: { n: 99, tag: 'primed' } })
  resolveProbe({ success: true, data: { n: 1, tag: 'stale' } })
  await probe
  const next = await channel.fetch()
  assert.equal(next.cached, true)
  assert.equal(next.data.tag, 'primed')
})
