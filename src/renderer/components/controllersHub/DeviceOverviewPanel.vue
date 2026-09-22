<template>
  <section class="chub-panel">
    <div class="chub-section-head">
      <span class="chub-section-label">{{ t.deviceHub.tabs.overview }}</span><span class="chub-section-rule"></span>
      <el-button size="small" :loading="loading" @click="refresh()">{{ t.deviceHub.refresh }}</el-button>
    </div>
    <p v-if="loadError" class="chub-status-error">{{ t.deviceHub.statusUnavailable }}</p>
    <div v-if="!loaded" class="chub-cards"><el-skeleton :rows="4" animated /></div>
    <div v-else class="chub-cards chub-overview-cards">
      <article class="chub-card">
        <div class="chub-card-head"><strong>{{ t.deviceHub.overview.controllers }}</strong><el-tag size="small" :type="runtime.ds.verified && !probeFailed.ds ? 'success' : 'info'" effect="plain">{{ probeFailed.ds ? t.deviceHub.probeUnavailable : runtime.ds.verified ? t.deviceHub.available : t.deviceHub.components.optional }}</el-tag></div>
        <p class="chub-hint">{{ t.deviceHub.overview.controllersHint }}</p>
        <div class="chub-card-actions"><el-button size="small" type="primary" @click="emit('navigate', 'controllers')">{{ t.deviceHub.overview.manageControllers }}</el-button></div>
      </article>
      <article class="chub-card">
        <div class="chub-card-head"><strong>{{ t.deviceHub.overview.microphone }}</strong><el-tag size="small" effect="plain">{{ t.deviceHub.microphone.statusLabels[microphoneState] }}</el-tag></div>
        <p class="chub-hint">{{ t.deviceHub.microphone.summaries[microphoneState] }}</p>
        <div class="chub-card-actions"><el-button size="small" type="primary" @click="emit('navigate', 'microphone')">{{ t.deviceHub.overview.inspectMicrophone }}</el-button></div>
      </article>
      <article class="chub-card">
        <div class="chub-card-head"><strong>{{ t.deviceHub.overview.usb }}</strong><el-tag size="small" :type="runtime.usb.ready && !probeFailed.usb ? 'success' : 'info'" effect="plain">{{ probeFailed.usb ? t.deviceHub.probeUnavailable : runtime.usb.ready ? t.deviceHub.available : t.deviceHub.components.optional }}</el-tag></div>
        <p class="chub-hint">{{ t.deviceHub.overview.usbHint }}</p>
        <div class="chub-card-actions"><el-button size="small" type="primary" @click="emit('navigate', 'usb')">{{ t.deviceHub.overview.inspectUsb }}</el-button></div>
      </article>
    </div>
    <div class="chub-panel-footer"><span class="chub-hint">{{ t.deviceHub.components.managementHint }}</span><el-button size="small" @click="emit('navigate', 'components')">{{ t.deviceHub.components.manage }}</el-button></div>
  </section>
</template>

<script setup>
import { computed, onMounted } from 'vue'
import { microphoneOverviewState } from '../../composables/deviceHubStatus.js'
import { useDeviceRuntime } from '../../composables/useDeviceRuntime.js'
import { useI18n } from '../../desktop/i18n/index.js'
const emit = defineEmits(['navigate'])
const { t } = useI18n()
const { runtime, loading, loaded, loadError, probeFailed, refresh } = useDeviceRuntime()
const microphoneState = computed(() => probeFailed.mic ? 'unknown' : microphoneOverviewState(runtime.mic))
onMounted(() => refresh())
</script>
