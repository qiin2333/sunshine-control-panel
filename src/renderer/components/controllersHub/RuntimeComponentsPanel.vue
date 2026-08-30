<template>
  <section class="chub-panel chub-runtime-components">
    <div class="chub-section">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.deviceHub.components.runtimeTitle }}</span><span class="chub-section-rule"></span>
        <el-button size="small" :loading="loading" @click="refresh">{{ t.deviceHub.refresh }}</el-button>
      </div>
      <div v-if="!loaded" class="chub-cards" aria-live="polite">
        <article v-for="index in 2" :key="index" class="chub-card chub-card-placeholder">
          <el-skeleton :rows="2" animated />
        </article>
      </div>
      <div v-else class="chub-cards">
        <article class="chub-card">
          <div class="chub-card-head"><strong>Virtual Device Host</strong><el-tag size="small" :type="hostReady ? 'success' : 'info'" effect="plain">{{ hostReady ? t.deviceHub.available : t.deviceHub.unavailable }}</el-tag></div>
          <p class="chub-hint">{{ t.deviceHub.components.hostHint }}</p><p class="chub-status-text">{{ ds.component_version || ds.runtime_version || t.deviceHub.unknown }}</p>
        </article>
        <article class="chub-card">
          <div class="chub-card-head"><strong>USB/IP Transport</strong><el-tag size="small" :type="ds.usbip_available ? 'success' : 'warning'" effect="plain">{{ ds.usbip_available ? t.deviceHub.available : t.deviceHub.unavailable }}</el-tag></div>
          <p class="chub-hint">{{ t.deviceHub.components.transportHint }}</p><p class="chub-status-text">{{ ds.usbip_version || t.deviceHub.unknown }}</p>
        </article>
      </div>
      <p v-if="loadError" class="chub-status-error">{{ t.deviceHub.statusUnavailable }}</p>
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { dualsense, virtualMicrophone } from '../../tauri-adapter.js'
import { deviceRuntimeReady } from '../../composables/deviceHubStatus.js'
import { useI18n } from '../../desktop/i18n/index.js'
const { t } = useI18n()
const loading = ref(false)
const loaded = ref(false)
const loadError = ref(false)
const ds = reactive({ verified: false, component_version: '', runtime_version: '', usbip_available: false, usbip_version: '' })
const mic = reactive({ component_available: false })
const hostReady = computed(() => deviceRuntimeReady(ds, mic))
async function refresh() {
  if (loading.value) return
  loading.value = true
  const [dsResult, micResult] = await Promise.all([dualsense.getStatus(), virtualMicrophone.getStatus()])
  if (dsResult?.success) Object.assign(ds, dsResult.data)
  if (micResult?.success) Object.assign(mic, micResult.data)
  loadError.value = !dsResult?.success && !micResult?.success
  loaded.value = true
  loading.value = false
}
onMounted(refresh)
</script>
