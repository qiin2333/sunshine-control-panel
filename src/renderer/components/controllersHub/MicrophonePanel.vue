<template>
  <section class="chub-panel">
    <div class="chub-window" :class="statusTone">
      <span class="chub-window-tab">{{ t.deviceHub.microphone.title }}</span>
      <div class="chub-hud-row">
        <div class="chub-hud-state"><span class="chub-status-dot"></span><strong>{{ stateLabel }}</strong></div>
        <div class="chub-hud-actions">
          <el-button size="small" :loading="loading" @click="refresh(false)">{{ t.deviceHub.refresh }}</el-button>
          <el-button size="small" type="primary" :loading="testing" :disabled="!canTest" @click="runTest">{{ t.deviceHub.microphone.test }}</el-button>
        </div>
      </div>
      <div class="chub-runtime-grid chub-mic-grid">
        <div><span>{{ t.deviceHub.microphone.configuredBackend }}</span><strong>{{ backendLabel(status.configured_backend) }}</strong></div>
        <div><span>{{ t.deviceHub.microphone.activeBackend }}</span><strong>{{ backendLabel(status.active_backend) }}</strong></div>
        <div><span>{{ t.deviceHub.microphone.component }}</span><strong>{{ componentLabel }}</strong></div>
        <div><span>{{ t.deviceHub.microphone.endpoint }}</span><strong>{{ endpointLabel }}</strong></div>
        <div><span>{{ t.deviceHub.microphone.capture }}</span><strong>{{ captureLabel }}</strong></div>
        <div><span>{{ t.deviceHub.microphone.buffer }}</span><strong>{{ status.buffered_bytes }} B</strong></div>
        <div><span>{{ t.deviceHub.microphone.health }}</span><strong :class="{ 'chub-value-danger': healthNeedsAttention }">{{ healthLabel }}</strong></div>
      </div>
      <details class="chub-diagnostics-details">
        <summary>{{ t.deviceHub.microphone.diagnostics }}</summary>
        <div class="chub-runtime-grid chub-diagnostics-grid">
          <div><span>{{ t.deviceHub.microphone.underruns }}</span><strong>{{ status.underruns }}</strong></div>
          <div><span>{{ t.deviceHub.microphone.droppedFrames }}</span><strong>{{ status.dropped_frames }}</strong></div>
          <div><span>{{ t.deviceHub.microphone.submitErrors }}</span><strong>{{ status.submit_errors }}</strong></div>
        </div>
      </details>
      <el-alert v-if="status.fallback_reason || status.error_code" :title="status.error_code || t.deviceHub.microphone.fallback" :description="status.fallback_reason || t.deviceHub.microphone.errorHint" type="warning" :closable="false" class="chub-notice" />
      <p v-if="loadError" class="chub-status-error">{{ loadError }}</p>
      <div class="chub-context-note">
        <strong>{{ t.deviceHub.microphone.experimentalTitle }}</strong>
        <span>{{ t.deviceHub.microphone.experimentalHint }}</span>
      </div>
      <div class="chub-panel-footer chub-mic-actions">
        <span class="chub-hint">{{ t.deviceHub.microphone.webuiHint }}</span>
        <el-button size="small" @click="openAudioSettings">{{ t.deviceHub.microphone.openWebui }}</el-button>
      </div>
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { openExternalUrl, sunshine, virtualMicrophone } from '../../tauri-adapter.js'
import { canTestMicrophone, microphoneStatusTone } from '../../composables/deviceHubStatus.js'
import { useI18n } from '../../desktop/i18n/index.js'

const { t } = useI18n()
const loading = ref(false)
const testing = ref(false)
const loadError = ref('')
const status = reactive({ configured_backend: '', active_backend: '', fallback_reason: '', component_available: false, online: false, device_created: false, host_streaming: false, generation: 0, state: 'absent', buffered_bytes: 0, underruns: 0, dropped_frames: 0, submit_errors: 0, last_error: 0, error_code: '' })
let pollTimer
const stateLabel = computed(() => t.value.deviceHub.microphone.states[status.state] || status.state || t.value.deviceHub.unknown)
const statusTone = computed(() => microphoneStatusTone(status))
const canTest = computed(() => canTestMicrophone(status, loading.value))
const componentLabel = computed(() => status.component_available
  ? t.value.deviceHub.available
  : t.value.deviceHub.unavailable)
const endpointLabel = computed(() => status.device_created
  ? t.value.deviceHub.microphone.endpointReady
  : t.value.deviceHub.microphone.endpointNotCreated)
const captureLabel = computed(() => status.host_streaming
  ? t.value.deviceHub.microphone.captureActive
  : t.value.deviceHub.microphone.captureIdle)
const healthNeedsAttention = computed(() => Boolean(status.error_code || status.submit_errors > 0))
const healthLabel = computed(() => healthNeedsAttention.value
  ? t.value.deviceHub.microphone.healthAttention
  : t.value.deviceHub.microphone.healthOk)
const backendLabel = (backend) => {
  if (!backend) return '—'
  return t.value.deviceHub.microphone.backends[backend] || backend
}

async function refresh(quiet = true) {
  if (loading.value) return
  loading.value = true
  if (!quiet) loadError.value = ''
  const result = await virtualMicrophone.getStatus()
  if (result?.success) Object.assign(status, result.data)
  else if (!quiet) {
    loadError.value = typeof result?.message === 'string'
      ? result.message
      : t.value.deviceHub.statusUnavailable
  }
  loading.value = false
}
async function runTest() {
  if (testing.value || !canTest.value) return
  testing.value = true
  const result = await virtualMicrophone.test()
  if (result?.success && result.data?.success) ElMessage.success(t.value.deviceHub.microphone.testSuccess)
  else ElMessage.error(result?.data?.error_code || result?.message || t.value.deviceHub.microphone.testFailed)
  testing.value = false
  await refresh(false)
}
async function openAudioSettings() {
  const base = await sunshine.getUrl()
  const url = `${String(base || 'https://localhost:47990/').replace(/\/$/, '')}/config/#av`
  if (!await openExternalUrl(url)) ElMessage.error(t.value.deviceHub.microphone.openFailed)
}
onMounted(() => {
  refresh(false)
  pollTimer = window.setInterval(() => refresh(true), 3000)
})
onUnmounted(() => window.clearInterval(pollTimer))
</script>
