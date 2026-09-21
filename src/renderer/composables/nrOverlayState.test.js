import assert from 'node:assert/strict'
import test from 'node:test'
import { nrOverlayState, overlayOpacity } from './nrOverlayState.js'

test('NR indicator reports actual processing, not just requested state', () => {
  const p = { nr_toggle_supported: true, nr_requested_enabled: true, nr_state: 'disabled' }
  assert.equal(nrOverlayState(p, true), 'warming_up')
  p.nr_state = 'active'
  assert.equal(nrOverlayState(p, true), 'active')
  p.nr_requested_enabled = false
  assert.equal(nrOverlayState(p, true), 'stopping')
  p.nr_state = 'disabled'
  assert.equal(nrOverlayState(p, true), 'disabled')
  p.nr_requested_enabled = true
  p.nr_state = 'degraded'
  assert.equal(nrOverlayState(p, true), 'degraded')
})

test('Unavailable, ended, and unsupported sessions cannot appear enabled', () => {
  assert.equal(nrOverlayState({ nr_state: 'active' }, false), 'unavailable')
  assert.equal(nrOverlayState(undefined, true), 'idle')
  assert.equal(nrOverlayState({ nr_state: 'active', nr_toggle_supported: false }, true), 'blocked')
})

test('Saved opacity has safe defaults and keeps controls legible', () => {
  assert.equal(overlayOpacity(null), 72)
  assert.equal(overlayOpacity('invalid'), 72)
  assert.equal(overlayOpacity('0'), 35)
  assert.equal(overlayOpacity('100'), 95)
  assert.equal(overlayOpacity('65'), 65)
})
