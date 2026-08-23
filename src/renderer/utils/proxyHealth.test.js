import assert from 'node:assert/strict'
import test from 'node:test'

import { checkLocalProxyHealth } from './proxyHealth.js'

const check = {
  url: 'http://127.0.0.1:48081/__foundation_proxy_health?token=expected',
  token: 'expected',
}

test('accepts only the matching loopback health challenge', async () => {
  const ok = await checkLocalProxyHealth(check, {
    fetchImpl: async () => ({ ok: true, text: async () => 'expected' }),
  })
  const intercepted = await checkLocalProxyHealth(check, {
    fetchImpl: async () => ({ ok: true, text: async () => '<html>proxy page</html>' }),
  })

  assert.equal(ok, true)
  assert.equal(intercepted, false)
})

test('rejects failed and incomplete health checks', async () => {
  assert.equal(await checkLocalProxyHealth(check, {
    fetchImpl: async () => ({ ok: false, text: async () => 'expected' }),
  }), false)
  assert.equal(await checkLocalProxyHealth({}, {
    fetchImpl: async () => ({ ok: true, text: async () => 'expected' }),
  }), false)
})

test('stops waiting when the local proxy does not respond', async () => {
  const result = await checkLocalProxyHealth(check, {
    timeoutMs: 5,
    fetchImpl: (_url, { signal }) => new Promise((_resolve, reject) => {
      signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true })
    }),
  })

  assert.equal(result, false)
})
