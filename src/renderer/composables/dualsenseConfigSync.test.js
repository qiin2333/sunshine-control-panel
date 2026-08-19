import assert from 'node:assert/strict'
import test from 'node:test'

import { dualSenseConfigUiState } from './dualsenseConfigSync.js'

const response = {
  enabled: true,
  audio_haptics: false,
  legacy_strength: 1.4,
  legacy_curve: 0.7,
  legacy_noise_gate: 0.008,
}

test('keeps locally edited tuning out of a non-tuning save response', () => {
  assert.deepEqual(dualSenseConfigUiState(response, true), {
    enabled: true,
    audioHaptics: false,
    tuning: null,
  })
})

test('synchronizes tuning from Core when there are no local edits', () => {
  assert.deepEqual(dualSenseConfigUiState(response), {
    enabled: true,
    audioHaptics: false,
    tuning: {
      strength: 1.4,
      curve: 0.7,
      noiseGate: 0.008,
    },
  })
})
