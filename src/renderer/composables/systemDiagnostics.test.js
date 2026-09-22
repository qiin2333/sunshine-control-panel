import test from 'node:test'
import assert from 'node:assert/strict'
import { probeSystemDiagnostics } from './systemDiagnostics.js'
import { en } from '../desktop/i18n/en.js'

const text = en.tools.status
const invoke = async command => command === 'get_gpus' ? ['NVIDIA RTX', 'Intel Graphics'] : 'http://127.0.0.1:1234'

test('GPU enumeration and valid local response do not certify encoding or remote access', async () => {
  const rows = await probeSystemDiagnostics(invoke, async () => ({ ok: true, json: async () => ({ status: true }) }), text)
  assert.equal(rows.gpu.value, 'NVIDIA RTX / Intel Graphics')
  assert.equal(rows.network.status, 'good')
  assert.equal(rows.encoder.statusText, text.unverified)
  assert.equal(rows.firewall.statusText, text.unverified)
})

test('invalid responses and HTTP failure do not diagnose a blocked firewall', async () => {
  for (const response of [
    ...[{}, { status: false }, { status: 'false' }].map(data => ({ ok: true, json: async () => data })),
    { ok: false },
  ]) {
    const rows = await probeSystemDiagnostics(invoke, async () => response, text)
    assert.equal(rows.network.value, text.localUnconfirmed)
    assert.equal(rows.network.status, 'warning')
    assert.equal(rows.firewall.value, text.remoteNotTested)
  }
})

test('failed GPU enumeration does not infer software fallback and does not prevent local check', async () => {
  const rows = await probeSystemDiagnostics(async command => {
    if (command === 'get_gpus') throw new Error('unavailable')
    return 'http://127.0.0.1:1234'
  }, async () => ({ ok: true, json: async () => ({ status: 'true' }) }), text)
  assert.equal(rows.gpu.statusText, text.unknown)
  assert.equal(rows.encoder.value, text.encoderNotTested)
  assert.equal(rows.network.status, 'good')
})
