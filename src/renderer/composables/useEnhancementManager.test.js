import assert from 'node:assert/strict'
import test from 'node:test'
import { webcrypto } from 'node:crypto'
import { createSSRApp, ref } from 'vue'
import { renderToString } from 'vue/server-renderer'
import 'element-plus'
import { mockIPC } from '@tauri-apps/api/mocks'
import { rtxHdrMessages } from './rtxHdrMessages.js'
import { dlssNrText } from './dlssNrMessages.js'

// The desktop locale module updates the document language during import.
globalThis.document = { documentElement: {} }
Object.defineProperty(globalThis, 'navigator', { value: { language: 'en' }, configurable: true })
globalThis.window = { addEventListener() {}, crypto: webcrypto }
mockIPC(() => undefined, { shouldMockEvents: true })
const { useEnhancementManager } = await import('./useEnhancementManager.js')

for (const text of [rtxHdrMessages.en, dlssNrText('zh')]) {
  test(`${text.title}: failed initial check stays unknown and a retry restores status`, async () => {
    let resolveStatus
    const api = { getStatus: () => new Promise(resolve => { resolveStatus = resolve }) }
    let manager
    await renderToString(createSSRApp({
      setup() {
        manager = useEnhancementManager({ api, messages: ref(text) })
        return () => null
      },
    }))

    const failed = manager.refresh()
    assert.equal(manager.refreshing.value, true)
    assert.equal(manager.stateLabel.value, text.states.loading)
    assert.ok(manager.healthRows.value.every(row => row.state === text.unknown && row.tone === 'unknown'))
    resolveStatus({ success: false, message: 'HDR-OP-001: busy' })
    await failed
    assert.equal(manager.statusKnown.value, false)
    assert.equal(manager.refreshing.value, false)
    assert.equal(manager.controlsBusy.value, false)
    assert.equal(manager.stateLabel.value, text.states.unavailable)
    assert.ok(manager.healthRows.value.every(row => row.state === text.unknown))
    assert.match(manager.operationError.value, /busy/)

    const retried = manager.refresh()
    assert.equal(manager.stateLabel.value, text.states.loading)
    resolveStatus({ success: true, data: {
      state: 'selected', installed: true, enabled: true,
      adapter_present: true, vc_runtime_present: true, runtime_present: true,
    } })
    await retried
    assert.equal(manager.statusKnown.value, true)
    assert.equal(manager.stateLabel.value, text.states.selected)
    assert.equal(manager.operationError.value, '')
    assert.ok(manager.healthRows.value.every(row => row.state === text.present && row.tone === 'ok'))
  })
}
