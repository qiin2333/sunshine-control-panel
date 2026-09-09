import assert from 'node:assert/strict'
import test from 'node:test'

import { isAllowedExternalUrl, usbip } from './tauri-adapter.js'

test('external URL validation rejects local paths and unapproved protocols', () => {
  assert.equal(isAllowedExternalUrl('https://example.com'), true)
  assert.equal(isAllowedExternalUrl('HTTP://localhost:47990'), true)
  assert.equal(isAllowedExternalUrl('ms-windows-store://pdp/?ProductId=example'), true)
  assert.equal(isAllowedExternalUrl('C:\\Windows\\System32'), false)
  assert.equal(isAllowedExternalUrl('file:///C:/Windows/System32'), false)
  assert.equal(isAllowedExternalUrl('shell:AppsFolder/example'), false)
})

test('USB forwarding preserves the automatic port and reports save failures', async () => {
  const previous = globalThis.window
  const calls = []
  globalThis.window = { __TAURI_INTERNALS__: { invoke: async (command, args) => {
    calls.push({ command, args })
    if (command === 'usbip_save_forwarding_config') throw new Error('offline')
    return { supported: true, enabled: false, port: 0 }
  } } }
  try {
    assert.deepEqual(await usbip.getForwardingConfig(), {
      success: true, data: { supported: true, enabled: false, port: 0 },
    })
    assert.equal((await usbip.saveForwardingConfig(true, 0)).success, false)
    assert.deepEqual(calls[1], {
      command: 'usbip_save_forwarding_config', args: { enabled: true, port: 0 },
    })
  } finally {
    if (previous === undefined) delete globalThis.window
    else globalThis.window = previous
  }
})
