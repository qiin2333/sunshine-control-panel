import test from 'node:test'
import assert from 'node:assert/strict'
import { nrSessionRequest } from './nrSessionControls.js'
const active = {
  id: 42,
  nr_toggle_supported: true,
  nr_live_controls_version: 3,
  nr_requested_enabled: true,
  nr_state: 'active',
  nr_requested_scale_percent: 75,
  nr_scale_percent: 75
}
test('live controls cannot redirect an edit or write while a previous setting is pending', () => {
  assert.throws(
    () => nrSessionRequest(active, 43, { scalePercent: 50 }),
    /nr_session_ended/
  )
  assert.throws(
    () =>
      nrSessionRequest({ ...active, nr_requested_scale_percent: 50 }, 42, {
        enabled: false
      }),
    /nr_settings_pending/
  )
  assert.throws(
    () => nrSessionRequest(active, 42, { enabled: false }, false),
    /nr_toggle_unsupported/
  )
})
test('scale requires a compatible enabled stream and the 5 percent grid', () => {
  assert.throws(() =>
    nrSessionRequest({ ...active, nr_live_controls_version: 1 }, 42, {
      scalePercent: 50
    })
  )
  assert.throws(
    () => nrSessionRequest(active, 42, { scalePercent: 67 }),
    /nr_request_invalid/
  )
  assert.deepEqual(nrSessionRequest(active, 42, { scalePercent: 20 }), {
    id: 42,
    enabled: true,
    scalePercent: 20
  })
  assert.deepEqual(
    nrSessionRequest(
      { ...active, nr_requested_enabled: false, nr_state: 'disabled' },
      42,
      { enabled: true }
    ),
    { id: 42, enabled: true }
  )
})

test('unadvertised scale support and disabled streams reject scale writes', () => {
  assert.throws(
    () =>
      nrSessionRequest({ ...active, nr_live_controls_version: undefined }, 42, {
        scalePercent: 50
      }),
    /nr_toggle_unsupported/
  )
  assert.throws(
    () =>
      nrSessionRequest(
        { ...active, nr_requested_enabled: false, nr_state: 'disabled' },
        42,
        { scalePercent: 50 }
      ),
    /nr_toggle_unsupported/
  )
})
