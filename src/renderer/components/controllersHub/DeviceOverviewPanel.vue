<template>
  <section class="chub-panel">
    <div class="chub-window" :class="runtimeTone">
      <span class="chub-window-tab">{{ t.deviceHub.runtime.title }}</span>
      <div class="chub-hud-row">
        <div class="chub-hud-state"><span class="chub-status-dot"></span><strong>{{ runtimeLabel }}</strong></div>
        <el-button size="small" :loading="loading" @click="refresh(true)">{{ t.deviceHub.refresh }}</el-button>
      </div>
      <div class="chub-runtime-grid">
        <div><span>{{ t.deviceHub.runtime.host }}</span><strong>{{ hostLabel }}</strong></div>
        <div><span>{{ t.deviceHub.runtime.transport }}</span><strong>{{ transportLabel }}</strong></div>
        <div><span>{{ t.deviceHub.runtime.owner }}</span><strong>Sunshine Core</strong></div>
      </div>
      <p v-if="loadError" class="chub-status-error">{{ t.deviceHub.statusUnavailable }}</p>
    </div>

    <div class="chub-cards chub-overview-cards">
      <article class="chub-card">
        <div class="chub-card-head"><strong>{{ t.deviceHub.overview.controllers }}</strong><el-tag size="small" :type="controllerReady ? 'success' : 'info'" effect="plain">{{ controllerReady ? t.deviceHub.available : t.deviceHub.needsSetup }}</el-tag></div>
        <p class="chub-hint">{{ t.deviceHub.overview.controllersHint }}</p>
        <div class="chub-card-actions"><el-button size="small" type="primary" @click="emit('navigate', 'controllers')">{{ t.deviceHub.overview.manageControllers }}</el-button></div>
      </article>
      <article class="chub-card">
        <div class="chub-card-head"><strong>{{ t.deviceHub.overview.microphone }}</strong><el-tag size="small" :type="microphoneTagType" effect="plain">{{ microphoneTag }}</el-tag></div>
        <p class="chub-hint">{{ microphoneSummary }}</p>
        <div class="chub-card-actions"><el-button size="small" type="primary" @click="emit('navigate', 'microphone')">{{ t.deviceHub.overview.inspectMicrophone }}</el-button></div>
      </article>
      <article class="chub-card">
        <div class="chub-card-head"><strong>{{ t.deviceHub.overview.usb }}</strong><el-tag size="small" :type="usb.ready ? 'success' : 'warning'" effect="plain">{{ usb.ready ? t.deviceHub.available : t.deviceHub.needsSetup }}</el-tag></div>
        <p class="chub-hint">{{ t.deviceHub.overview.usbHint }}</p>
        <div class="chub-card-actions"><el-button size="small" type="primary" @click="emit('navigate', 'usb')">{{ t.deviceHub.overview.inspectUsb }}</el-button></div>
      </article>
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted } from 'vue'
import { deviceRuntimeReady, microphoneOverviewState } from '../../composables/deviceHubStatus.js'
import { useDeviceRuntime } from '../../composables/useDeviceRuntime.js'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['navigate'])
const { t } = useI18n()
const { runtime, loading, loadError, refresh } = useDeviceRuntime()
const usb = computed(() => runtime.usb)
const controllerReady = computed(() => runtime.ds.verified)
const microphoneState = computed(() => microphoneOverviewState(runtime.mic))
const microphoneTag = computed(() => ({
  missing: t.value.deviceHub.overview.statusUnavailable,
  capturing: t.value.deviceHub.overview.statusCapturing,
  idle: t.value.deviceHub.overview.statusReady,
  waiting: t.value.deviceHub.overview.statusWaiting,
})[microphoneState.value])
const microphoneTagType = computed(() => ({
  missing: 'info',
  capturing: 'primary',
  idle: 'success',
  waiting: 'info',
})[microphoneState.value])
const runtimeReady = computed(() => deviceRuntimeReady(runtime.ds, runtime.mic))
const runtimeTone = computed(() => runtimeReady.value ? 'state-ready' : loadError.value ? 'state-error' : '')
const runtimeLabel = computed(() => runtimeReady.value ? t.value.deviceHub.runtime.ready : t.value.deviceHub.runtime.needsAttention)
const hostLabel = computed(() => runtimeReady.value ? t.value.deviceHub.available : t.value.deviceHub.unavailable)
const transportLabel = computed(() => usb.value.ready || runtime.ds.usbip_available
  ? `USB/IP ${usb.value.version || runtime.ds.usbip_version || ''}`.trim()
  : t.value.deviceHub.unavailable)
const microphoneSummary = computed(() => {
  const key = microphoneState.value
  return {
    missing: t.value.deviceHub.overview.microphoneMissing,
    capturing: t.value.deviceHub.overview.microphoneCapturing,
    idle: t.value.deviceHub.overview.microphoneIdle,
    waiting: t.value.deviceHub.overview.microphoneWaiting,
  }[key]
})

onMounted(() => refresh())
</script>
