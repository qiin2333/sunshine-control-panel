<template>
  <div class="chub-section usb-forwarding-settings">
    <div class="chub-section-head">
      <span class="chub-section-label">◈ {{ text.forwardingTitle }}</span>
      <span class="chub-section-rule"></span>
    </div>
    <p class="chub-hint">{{ text.forwardingHint }}</p>
    <div v-if="loading" role="status" :aria-label="text.loadingConfig">
      <el-skeleton :rows="2" animated />
    </div>
    <template v-else-if="loadError">
      <el-alert class="chub-notice" type="error" :closable="false" :title="text.loadConfigFailed" />
      <el-button size="small" @click="load">{{ text.retryLoad }}</el-button>
    </template>
    <el-alert v-else-if="!supported" class="chub-notice" type="info" :closable="false" :title="text.updateHost" />
    <template v-else>
      <div class="forwarding-row">
        <label for="usb-forwarding-enabled">{{ text.allowClients }}</label>
        <el-switch id="usb-forwarding-enabled" v-model="enabled" :disabled="saving" :aria-label="text.allowClients" />
      </div>
      <p v-if="enabled && !transportReady" class="chub-hint">{{ text.prepareTransport }}</p>
      <details class="chub-disclosure chub-disclosure--inline">
        <summary>{{ text.advancedPort }}<span class="chub-disclosure-mark" aria-hidden="true">▸</span></summary>
        <label class="forwarding-row" for="usb-forwarding-port">
          <span>{{ text.forwardingPort }}</span>
          <el-input-number id="usb-forwarding-port" v-model="port" :min="0" :max="65535"
            :controls="false" :disabled="saving" :aria-label="text.forwardingPort" />
        </label>
        <p class="chub-hint">{{ text.autoPortHint }}</p>
      </details>
      <p v-if="!validPort" aria-live="polite" class="chub-status-error">{{ text.invalidForwardingPort }}</p>
      <div class="forwarding-actions">
        <el-button type="primary" :loading="saving" :disabled="!dirty || !validPort || saving" @click="save">{{ text.saveConfig }}</el-button>
        <span class="chub-hint">{{ text.restartHint }}</span>
      </div>
      <el-alert v-if="saveError" class="chub-notice" type="error" :closable="false" :title="text.saveConfigFailed" />
    </template>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { usbip } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'

defineProps({ transportReady: Boolean })
const { t } = useI18n()
const text = computed(() => t.value.deviceHub.usb)
const loading = ref(true)
const loadError = ref(false)
const supported = ref(false)
const saving = ref(false)
const saveError = ref(false)
const enabled = ref(false)
const port = ref(0)
const baseline = ref({ enabled: false, port: 0 })
const dirty = computed(() => enabled.value !== baseline.value.enabled || port.value !== baseline.value.port)
const validPort = computed(() => Number.isInteger(port.value) && (port.value === 0 || (port.value >= 1024 && port.value <= 65535)))

async function load() {
  loading.value = true
  loadError.value = false
  try {
    const result = await usbip.getForwardingConfig()
    if (!result?.success) throw new Error('load failed')
    supported.value = result.data.supported === true
    enabled.value = result.data.enabled === true
    port.value = result.data.port
    baseline.value = { enabled: enabled.value, port: port.value }
  } catch {
    loadError.value = true
  } finally {
    loading.value = false
  }
}

async function save() {
  if (saving.value || !supported.value || !dirty.value || !validPort.value) return
  saving.value = true
  saveError.value = false
  try {
    const result = await usbip.saveForwardingConfig(enabled.value, port.value)
    if (!result?.success) throw new Error('save failed')
    baseline.value = { enabled: enabled.value, port: port.value }
    ElMessage.success(text.value.savedToast)
  } catch {
    saveError.value = true
  } finally {
    saving.value = false
  }
}
onMounted(load)
</script>

<style scoped>
.forwarding-row, .forwarding-actions { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; margin: 12px 0; }
.forwarding-actions { justify-content: flex-start; }
</style>
