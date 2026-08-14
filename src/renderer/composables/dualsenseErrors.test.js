import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { dualSenseErrorCode, friendlyDualSenseError } from './dualsenseErrors.js'

const messages = {
  unknown: 'Please try again.',
  codes: { 'DS5-DRV-003': 'Restart Windows.' },
  contexts: {
    test: {
      'DS5-PKG-003': 'The test device could not start. Repair the component and try again.',
      fallback: 'The device test failed.',
    },
  },
}

describe('DualSense user-facing errors', () => {
  it('extracts diagnostic codes without exposing them in the friendly message', () => {
    const raw = 'DS5-PKG-003: component test process failed with exit code 1'
    assert.equal(dualSenseErrorCode(raw), 'DS5-PKG-003')
    assert.equal(friendlyDualSenseError(raw, messages, 'test'),
      'The test device could not start. Repair the component and try again.',
    )
  })

  it('uses actionable code guidance outside a specific operation', () => {
    assert.equal(friendlyDualSenseError('DS5-DRV-003: restart required', messages), 'Restart Windows.')
  })

  it('uses the operation fallback for an unknown diagnostic code', () => {
    assert.equal(friendlyDualSenseError('DS5-NEW-999: internal detail', messages, 'test'),
      'The device test failed.',
    )
  })
})
