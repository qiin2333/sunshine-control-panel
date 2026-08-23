import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createLatestIntentQueue,
  dualSenseConfigMatches,
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

test('recognizes a requested switch state after an ambiguous save response', () => {
  assert.equal(dualSenseConfigMatches(response, {
    enabled: true,
    audioHaptics: false,
    genshinCompatibility: false,
  }), true)
  assert.equal(dualSenseConfigMatches(response, {
    enabled: false,
    audioHaptics: false,
    genshinCompatibility: false,
  }), false)
  assert.equal(dualSenseConfigMatches({ ...response, config_readable: false }, {
    enabled: true,
    audioHaptics: false,
    genshinCompatibility: false,
  }), false)
})

test('serializes config saves and keeps only the latest pending intent', async () => {
  const observed = []
  let releaseFirst
  const firstBlocked = new Promise((resolve) => {
    releaseFirst = resolve
  })
  const queue = createLatestIntentQueue(async (intent) => {
    observed.push(intent)
    if (intent === 'enable') await firstBlocked
  })

  const first = queue.submit('enable')
  queue.submit('disable')
  queue.submit('enable-latest')
  assert.equal(queue.hasPending(), true)
  assert.equal(queue.peekPending(), 'enable-latest')

  releaseFirst()
  await first
  assert.deepEqual(observed, ['enable', 'enable-latest'])
  assert.equal(queue.hasPending(), false)
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
