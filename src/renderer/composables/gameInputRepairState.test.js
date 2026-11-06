import test from 'node:test'
import assert from 'node:assert/strict'
import { canRepairGameInput, gameInputReportFailed, gameInputBackupIds } from './gameInputRepairState.js'

test('only exact historical mappings enable repair', () => {
  for (const state of ['legacy_mapping', 'axes_fixed', 'navigation_fixed']) assert.equal(canRepairGameInput({ state }), true)
  for (const state of ['corrected_mapping', 'mapping_missing', 'custom_mapping', 'recovery_required', 'access_denied']) {
    assert.equal(canRepairGameInput({ state }), false)
  }
  assert.equal(canRepairGameInput(null), false)
})
test('recovery is shown as incomplete and its bounded backup ID remains available', () => {
  const id = '00112233445566778899aabbccddeeff'
  const report = { state: 'recovery_required', backup_id: id, backups: [id, '../outside', null] }
  assert.equal(gameInputReportFailed(report), true)
  assert.deepEqual(gameInputBackupIds(report), [id])
  assert.equal(gameInputReportFailed({ state: 'repaired' }), false)
  assert.deepEqual(gameInputBackupIds({ backups: 'invalid' }), [])
})

test('an operation backup stays selectable when the following check fails or omits it', () => {
  const id = '00112233445566778899aabbccddeeff'
  const operation = { state: 'recovery_required', backup_id: id }
  assert.deepEqual(gameInputBackupIds(null, operation), [id])
  assert.deepEqual(gameInputBackupIds({ state: 'inspection_failed', backups: [] }, operation), [id])
  assert.deepEqual(gameInputBackupIds({ backups: [id] }, operation), [id])
  assert.deepEqual(gameInputBackupIds(null, { backup_id: '../invalid' }), [])
})
