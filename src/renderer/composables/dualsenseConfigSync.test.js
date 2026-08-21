import assert from 'node:assert/strict'
import test from 'node:test'

import {
  dualSenseConfigReadable,
  dualSenseConfigUiState,
  mergeDualSenseStatus,
} from './dualsenseConfigSync.js'

const response = {
  enabled: true,
  audio_haptics: false,
  genshin_compatibility: false,
  legacy_strength: 1.4,
  legacy_curve: 0.7,
  legacy_noise_gate: 0.008,
  config_readable: true,
}

test('keeps locally edited tuning out of a non-tuning save response', () => {
  assert.deepEqual(dualSenseConfigUiState(response, true), {
    enabled: true,
    audioHaptics: false,
    genshinCompatibility: false,
    tuning: null,
  })
})

test('synchronizes tuning from Core when there are no local edits', () => {
  assert.deepEqual(dualSenseConfigUiState(response), {
    enabled: true,
    audioHaptics: false,
    genshinCompatibility: false,
    tuning: {
      strength: 1.4,
      curve: 0.7,
      noiseGate: 0.008,
    },
  })
})

test('preserves confirmed config when a status refresh cannot read Core settings', () => {
  const current = {
    ...response,
    config_revision: 4,
    verified: true,
  }
  const incoming = {
    ...response,
    enabled: false,
    audio_haptics: true,
    genshin_compatibility: true,
    legacy_strength: 1,
    legacy_curve: 0.5,
    legacy_noise_gate: 0.02,
    config_revision: 0,
    config_readable: false,
    verified: true,
    error_code: 'DS5-CFG-001',
  }

  assert.equal(dualSenseConfigReadable(incoming), false)
  assert.deepEqual(mergeDualSenseStatus(current, incoming), {
    ...incoming,
    enabled: true,
    audio_haptics: false,
    genshin_compatibility: false,
    legacy_strength: 1.4,
    legacy_curve: 0.7,
    legacy_noise_gate: 0.008,
    config_revision: 4,
  })
})
