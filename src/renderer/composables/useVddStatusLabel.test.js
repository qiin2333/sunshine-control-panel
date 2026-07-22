import test from 'node:test'
import assert from 'node:assert/strict'
import { reactive, ref } from 'vue'

import { useVddStatusLabel } from './useVddStatusLabel.js'

const labels = {
  ready: 'driverStateReady',
  degraded: 'driverStateDegraded',
  not_installed: 'driverStateNotInstalled',
  unhealthy: 'driverStateUnhealthy',
  reboot_required: 'driverStateRebootRequired',
  payload_missing: 'driverStatePayloadMissing',
  unsupported: 'driverStateUnsupported',
  unknown: 'driverStateUnknown',
}

test('maps every VDD state and unknown values through the shared labels', () => {
  const t = ref({ vddSettings: Object.fromEntries(Object.values(labels).map((key) => [key, key])) })
  const status = reactive({ state: 'ready' })
  const statusLabel = useVddStatusLabel(t, status)

  for (const [state, expected] of Object.entries(labels)) {
    status.state = state
    assert.equal(statusLabel.value, expected)
  }

  status.state = 'future_state'
  assert.equal(statusLabel.value, labels.unknown)

  for (const inheritedState of ['toString', 'constructor', 'valueOf']) {
    status.state = inheritedState
    assert.equal(statusLabel.value, labels.unknown)
  }
})
