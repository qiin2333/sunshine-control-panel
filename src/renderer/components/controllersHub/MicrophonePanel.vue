<template>
  <section class="chub-panel">
    <div class="chub-window" :class="statusTone">
      <span class="chub-window-tab">{{ text.title }}</span>
      <div class="chub-hud-row">
        <div class="chub-hud-state"><span class="chub-status-dot"></span><strong>{{ text.statusLabels[state] }}</strong></div>
        <div class="chub-hud-actions">
          <el-button size="small" :loading="loading" :disabled="testing" @click="refresh">{{ t.deviceHub.refresh }}</el-button>
          <el-button size="small" type="primary" :loading="testing" :disabled="!canTest || testing" @click="runTest">{{ text.test }}</el-button>
        </div>
      </div>
      <p class="chub-hint">{{ text.intro }}</p>
      <p>{{ text.summaries[state] }}</p>
      <div class="chub-runtime-grid">
        <div><span>{{ text.configuredBackend }}</span><strong>{{ backendLabel(status.configured_backend) }}</strong></div>
        <div><span>{{ text.activeBackend }}</span><strong>{{ backendLabel(status.active_backend) }}</strong></div>
      </div>
      <el-alert v-if="!loadError && (status.fallback_reason || (usesUsbip && status.error_code))" :title="text.fallback" :description="status.fallback_reason || status.error_code" type="warning" :closable="false" class="chub-notice" />
      <p v-if="loadError" role="alert" class="chub-status-error">{{ t.deviceHub.statusUnavailable }}</p>
      <div class="chub-panel-footer chub-mic-actions">
        <span class="chub-hint">{{ text.webuiHint }}</span>
        <el-button size="small" @click="openAudioSettings">{{ text.openWebui }}</el-button>
      </div>
    </div>
    <details class="chub-section chub-disclosure chub-disclosure--section">
      <summary>{{ text.advanced }}</summary>
      <p class="chub-hint">{{ text.backendHint }}</p>
      <div class="chub-context-note"><strong>{{ text.experimentalTitle }}</strong><span>{{ text.experimentalHint }}</span></div>
      <template v-if="!loadError && experimentalSelected">
        <p>{{ text.component }}: {{ status.component_available ? t.deviceHub.available : t.deviceHub.unavailable }}</p>
        <div class="chub-card-actions"><el-button size="small" @click="emit('manage-components')">{{ t.deviceHub.components.manage }}</el-button></div>
      </template>
      <details v-if="!loadError && usesUsbip" class="chub-diagnostics-details">
        <summary>{{ text.diagnostics }}</summary>
        <div class="chub-runtime-grid chub-mic-grid">
          <div><span>{{ text.endpoint }}</span><strong>{{ status.device_created ? text.endpointReady : text.endpointNotCreated }}</strong></div>
          <div><span>{{ text.capture }}</span><strong>{{ status.host_streaming ? text.captureActive : text.captureIdle }}</strong></div>
          <div><span>{{ text.buffer }}</span><strong>{{ status.buffered_bytes }} B</strong></div>
          <div><span>{{ text.underruns }}</span><strong>{{ status.underruns }}</strong></div>
          <div><span>{{ text.droppedFrames }}</span><strong>{{ status.dropped_frames }}</strong></div>
          <div><span>{{ text.submitErrors }}</span><strong>{{ status.submit_errors }}</strong></div>
        </div>
      </details>
    </details>
  </section>
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { openExternalUrl, sunshine, virtualMicrophone } from '../../tauri-adapter.js'
import { canTestMicrophone, microphoneOverviewState, microphoneStatusTone, microphoneUsesUsbip } from '../../composables/deviceHubStatus.js'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['manage-components'])
const { t } = useI18n()
const text = computed(() => t.value.deviceHub.microphone)
const loading = ref(true)
const testing = ref(false)
const loadError = ref(false)
const status = reactive({})
let pollTimer
let disposed = false
let pending = false
const state = computed(() => loadError.value || !status.configured_backend ? 'unknown' : microphoneOverviewState(status))
const statusTone = computed(() => loadError.value ? 'state-error' : microphoneStatusTone(status))
const canTest = computed(() => !loadError.value && canTestMicrophone(status, loading.value))
const usesUsbip = computed(() => microphoneUsesUsbip(status))
const experimentalSelected = computed(() => ['auto', 'usbip_experimental'].includes(status.configured_backend) || usesUsbip.value)
const backendLabel = backend => text.value.backends[backend] || backend || '—'

async function refresh() {
  if (pending || testing.value) return
  pending = true
  loading.value = true
  try {
    const result = await virtualMicrophone.getStatus()
    if (disposed) return
    loadError.value = !result?.success
    if (result?.success) Object.assign(status, result.data)
  } catch {
    if (!disposed) loadError.value = true
  } finally {
    loading.value = false
    pending = false
  }
}
async function runTest() {
  if (testing.value || !canTest.value) return
  testing.value = true
  try {
    const result = await virtualMicrophone.test()
    if (result?.success && result.data?.success) ElMessage.success(text.value.testSuccess)
    else ElMessage.error(result?.data?.error_code || text.value.testFailed)
  } catch {
    ElMessage.error(text.value.testFailed)
  } finally {
    testing.value = false
    if (!disposed) await refresh()
  }
}
async function openAudioSettings() {
  try {
    const base = await sunshine.getUrl()
    const url = `${String(base || 'https://localhost:47990/').replace(/\/$/, '')}/config/#av`
    if (!await openExternalUrl(url)) ElMessage.error(text.value.openFailed)
  } catch {
    ElMessage.error(text.value.openFailed)
  }
}
onMounted(() => {
  refresh()
  pollTimer = window.setInterval(refresh, 3000)
})
onUnmounted(() => { disposed = true; window.clearInterval(pollTimer) })
</script>
