import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { dualSenseErrorCode, friendlyDualSenseError } from './dualsenseErrors.js'

const messages = {
  unknown: 'Please try again.',
  codes: {
    'DS5-DRV-001': 'Repair the USB/IP transport.',
    'DS5-CFG-003': 'Make sure Sunshine is running.',
    'DS5-CFG-004': 'Check the current switch state.',
    'DS5-CFG-005': 'Remove the damaged DualSense settings file.',
    'DS5-CFG-006': 'Refresh before saving again.',
    'DS5-CFG-007': 'Update Sunshine and Control Panel together.',
  },
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
    assert.equal(friendlyDualSenseError('DS5-DRV-001: installer failed', messages), 'Repair the USB/IP transport.')
  })

  it('maps configuration write failures without exposing backend details', () => {
    assert.equal(
      friendlyDualSenseError('DS5-CFG-003: unable to save Sunshine configuration: connection reset', messages, 'config'),
      'Make sure Sunshine is running.',
    )
  })

  it('maps an uncertain save timeout to state verification guidance', () => {
    assert.equal(
      friendlyDualSenseError('DS5-CFG-004: timed out while applying DualSense configuration', messages, 'config'),
      'Check the current switch state.',
    )
  })

  it('maps an invalid independent settings file without exposing its path', () => {
    assert.equal(
      friendlyDualSenseError('DS5-CFG-005: invalid local file', messages, 'config'),
      'Remove the damaged DualSense settings file.',
    )
  })

  it('maps a blocked stale update to a refresh action', () => {
    assert.equal(
      friendlyDualSenseError('DS5-CFG-006: stale entity tag', messages, 'config'),
      'Refresh before saving again.',
    )
  })

  it('maps a conditional protocol mismatch to a coordinated update action', () => {
    assert.equal(
      friendlyDualSenseError('DS5-CFG-007: missing strong entity tag', messages, 'config'),
      'Update Sunshine and Control Panel together.',
    )
  })

  it('uses the operation fallback for an unknown diagnostic code', () => {
    assert.equal(friendlyDualSenseError('DS5-NEW-999: internal detail', messages, 'test'),
      'The device test failed.',
    )
  })
})
