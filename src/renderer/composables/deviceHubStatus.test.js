import test from 'node:test'
import assert from 'node:assert/strict'
import {
  canTestMicrophone,
  deviceRuntimeReady,
  dualSenseComponentAction,
  dualSenseComponentOperational,
  microphoneOverviewState,
  microphoneStatusTone,
} from './deviceHubStatus.js'

test('device runtime is ready when either shared host probe succeeds', () => {
  assert.equal(deviceRuntimeReady({ verified: true }, {}), true)
  assert.equal(deviceRuntimeReady({}, { component_available: true }), true)
  assert.equal(deviceRuntimeReady({}, {}), false)
})

test('DualSense actions distinguish repair from a compatible update', () => {
  assert.equal(dualSenseComponentAction({ installed: false }), 'install')
  assert.equal(dualSenseComponentAction({ installed: true, verified: false, update_available: true }), 'repair')
  assert.equal(dualSenseComponentAction({ installed: true, verified: true, update_available: true }), 'update')
  assert.equal(dualSenseComponentAction({ installed: true, verified: true }), '')
})

test('DualSense settings remain available for verified components with an update', () => {
  assert.equal(dualSenseComponentOperational({ verified: true, update_available: true }), true)
  assert.equal(dualSenseComponentOperational({ verified: false, update_available: true }), false)
})

test('microphone status prioritizes faults over an old online flag', () => {
  assert.equal(microphoneStatusTone({ state: 'faulted', online: true }), 'state-error')
  assert.equal(microphoneStatusTone({ error_code: 'MIC_FAILURE', device_created: true }), 'state-error')
  assert.equal(microphoneStatusTone({ device_created: true }), 'state-ready')
  assert.equal(microphoneStatusTone({ state: 'absent' }), '')
})

test('microphone test requires an available component and enabled backend', () => {
  assert.equal(canTestMicrophone({ component_available: true, configured_backend: 'usbip_experimental' }), true)
  assert.equal(canTestMicrophone({ component_available: true, configured_backend: 'disabled' }), false)
  assert.equal(canTestMicrophone({ component_available: false, configured_backend: 'usbip_experimental' }), false)
  assert.equal(canTestMicrophone({ component_available: true, configured_backend: 'usbip_experimental' }, true), false)
})

test('microphone overview distinguishes missing, waiting, idle and capturing', () => {
  assert.equal(microphoneOverviewState({}), 'missing')
  assert.equal(microphoneOverviewState({ component_available: true }), 'waiting')
  assert.equal(microphoneOverviewState({ component_available: true, device_created: true }), 'idle')
  assert.equal(microphoneOverviewState({ component_available: true, device_created: true, host_streaming: true }), 'capturing')
})
