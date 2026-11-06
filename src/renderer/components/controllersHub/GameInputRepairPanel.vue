<template>
  <article class="chub-card">
    <div class="chub-card-head"><strong>{{ labels.title }}</strong></div>
    <p class="chub-hint">{{ labels.hint }}</p>
    <div class="chub-card-actions">
      <el-button size="small" :loading="busy" :disabled="disabled" @click="check">{{ labels.check }}</el-button>
      <el-button v-if="canRepairGameInput(report)" size="small" type="primary"
        :disabled="disabled" @click="change(false)">{{ labels.repair }}</el-button>
    </div>
    <p v-if="report" class="chub-hint" role="status">{{ labels.states[report.state] || report.state }}</p>
    <el-alert v-if="lastResult?.restart_required" :title="labels.restart" type="warning" :closable="false" />
    <el-alert v-if="gameInputReportFailed(lastResult)" :title="labels.failed"
      :description="safeDualSenseTechnicalError(lastResult.detail || lastResult.state)" type="error" :closable="false" />
    <el-alert v-if="error || report?.detail" :title="error || safeDualSenseTechnicalError(report.detail)" type="warning" :closable="false" />
    <div v-if="backupIds.length" class="chub-card-actions">
      <el-select v-model="backupId" :placeholder="labels.backup" :disabled="disabled" size="small" style="min-width: 240px">
        <el-option v-for="id in backupIds" :key="id" :label="id" :value="id" />
      </el-select>
      <el-button size="small" :disabled="disabled || !backupId" @click="change(true)">{{ labels.restore }}</el-button>
    </div>
    <details v-if="report || lastResult">
      <summary>{{ labels.details }}</summary>
      <pre style="white-space: pre-wrap; overflow-wrap: anywhere">{{ JSON.stringify({ check: report, operation: lastResult }, null, 2) }}</pre>
    </details>
  </article>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ElMessageBox } from 'element-plus'
import { dualsense } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'
import { useDeviceComponentOperation } from '../../composables/useDeviceComponentOperation.js'
import { safeDualSenseTechnicalError } from '../../composables/dualsenseErrors.js'
import { canRepairGameInput, gameInputReportFailed, gameInputBackupIds } from '../../composables/gameInputRepairState.js'

const { t } = useI18n()
const labels = computed(() => t.value.dualSense.gameInput)
const shared = useDeviceComponentOperation()
const report = ref(null)
const lastResult = ref(null)
const backupId = ref('')
const error = ref('')
const busy = ref(false)
const disabled = computed(() => busy.value || shared.busyElsewhere.value)
const backupIds = computed(() => gameInputBackupIds(report.value, lastResult.value))

/** Refresh the read-only diagnosis while keeping the last operation's restart notice. */
async function loadReport() {
  let response
  try {
    response = await dualsense.checkGameInput()
    if (!response?.success) throw new Error(response?.message || labels.value.failed)
  } catch (e) {
    // An old diagnosis must not enable another repair after a failed refresh.
    report.value = null
    throw e
  }
  report.value = response.data
  if (!backupIds.value.includes(backupId.value)) backupId.value = response.data.backup_id || backupIds.value[0] || ''
}
const check = shared.wrap(async () => {
  busy.value = true
  error.value = ''
  try { await loadReport() } catch (e) { error.value = safeDualSenseTechnicalError(e.message || e) }
  finally { busy.value = false }
})
const change = shared.wrap(async (restore) => {
  busy.value = true
  error.value = ''
  try {
    try {
      await ElMessageBox.confirm(restore ? labels.value.restoreConfirm : labels.value.repairConfirm,
        restore ? labels.value.restore : labels.value.repair, { type: 'warning', confirmButtonText: restore ? labels.value.restore : labels.value.repair,
          cancelButtonText: t.value.deviceHub.usb.cancel })
    } catch { return }
    const response = await dualsense.repairGameInput(restore ? backupId.value : null)
    if (!response?.success) throw new Error(response?.message || labels.value.failed)
    lastResult.value = response.data
    const operationBackup = gameInputBackupIds(response.data)[0]
    if (operationBackup) backupId.value = operationBackup
    shared.markChanged()
    await loadReport()
  } catch (e) { error.value = safeDualSenseTechnicalError(e.message || e) }
  finally { busy.value = false }
})
shared.onOtherChange(() => { report.value = null })
</script>
