import test from 'node:test'
import assert from 'node:assert/strict'
import {
  installVddWithRecovery,
  isVddConfirmationRequired,
  isVddRepairVerified,
  VDD_CONFIRM_REQUIRED,
} from './vddInstallRecovery.js'

const readyStatus = {
  state: 'ready',
  running: true,
  version_match: true,
}

test('requires ready, running, matching driver state', () => {
  assert.equal(isVddRepairVerified(readyStatus), true)
  assert.equal(isVddRepairVerified({ ...readyStatus, state: 'degraded' }), false)
  assert.equal(isVddRepairVerified({ ...readyStatus, running: false }), false)
  assert.equal(isVddRepairVerified({ ...readyStatus, version_match: false }), false)
})

test('recognizes the active-stream confirmation request', () => {
  assert.equal(isVddConfirmationRequired(new Error(VDD_CONFIRM_REQUIRED)), true)
  assert.equal(isVddConfirmationRequired(VDD_CONFIRM_REQUIRED), true)
  assert.equal(isVddConfirmationRequired('stream is active'), false)
})

test('verifies the driver after a normal install response', async () => {
  const statuses = []
  const probes = [
    { success: true, data: { state: 'degraded', running: true, version_match: false } },
    { success: true, data: readyStatus },
  ]
  const result = await installVddWithRecovery({
    install: async () => ({ success: true, data: 'installed' }),
    getStatus: async () => probes.shift() || { success: true, data: readyStatus },
    onStatus: (status) => statuses.push(status),
    timeoutMs: 100,
    pollIntervalMs: 1,
  })

  assert.equal(result.success, true)
  assert.equal(isVddRepairVerified(statuses.at(-1)), true)
})

test('recovers when the install IPC response never returns', async () => {
  const pendingInstall = new Promise(() => {})
  const probes = [
    { success: true, data: { state: 'degraded', running: true, version_match: false } },
    { success: true, data: readyStatus },
  ]

  const result = await installVddWithRecovery({
    install: () => pendingInstall,
    getStatus: async () => probes.shift() || { success: true, data: readyStatus },
    timeoutMs: 100,
    pollIntervalMs: 1,
  })

  assert.equal(result.recovered, true)
  assert.equal(result.data.state, 'ready')
})

test('surfaces an immediate install refusal', async () => {
  await assert.rejects(
    installVddWithRecovery({
      install: async () => ({ success: false, message: 'stream is active' }),
      getStatus: async () => new Promise((resolve) => setTimeout(
        () => resolve({ success: true, data: { state: 'degraded' } }),
        20
      )),
      timeoutMs: 100,
      pollIntervalMs: 1,
    }),
    /stream is active/
  )
})

test('releases the UI after verification times out', async () => {
  await assert.rejects(
    installVddWithRecovery({
      install: () => new Promise(() => {}),
      getStatus: async () => ({
        success: true,
        data: { state: 'degraded', running: true, version_match: false },
      }),
      timeoutMs: 20,
      pollIntervalMs: 1,
      verificationError: 'verification timed out',
    }),
    /verification timed out/
  )
})
