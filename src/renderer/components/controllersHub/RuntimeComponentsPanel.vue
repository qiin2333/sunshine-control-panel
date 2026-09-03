<template>
  <section class="chub-panel chub-runtime-components">
    <div class="chub-section">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.deviceHub.components.runtimeTitle }}</span><span class="chub-section-rule"></span>
        <el-button size="small" :loading="loading" @click="refresh(true)">{{ t.deviceHub.refresh }}</el-button>
      </div>
      <div v-if="!loaded" class="chub-cards" aria-live="polite">
        <article v-for="index in 2" :key="index" class="chub-card chub-card-placeholder">
          <el-skeleton :rows="2" animated />
        </article>
      </div>
      <div v-else class="chub-cards">
        <article class="chub-card">
          <div class="chub-card-head"><strong>Virtual Device Host</strong><el-tag size="small" :type="hostReady ? 'success' : 'info'" effect="plain">{{ hostReady ? t.deviceHub.available : t.deviceHub.unavailable }}</el-tag></div>
          <p class="chub-hint">{{ t.deviceHub.components.hostHint }}</p><p class="chub-status-text">{{ runtime.ds.component_version || runtime.ds.runtime_version || t.deviceHub.unknown }}</p>
        </article>
        <article class="chub-card">
          <div class="chub-card-head"><strong>USB/IP Transport</strong><el-tag size="small" :type="runtime.usb.ready ? 'success' : 'warning'" effect="plain">{{ runtime.usb.ready ? t.deviceHub.available : t.deviceHub.unavailable }}</el-tag></div>
          <p class="chub-hint">{{ t.deviceHub.components.transportHint }}</p><p class="chub-status-text">{{ runtime.usb.version || t.deviceHub.unknown }}</p>
        </article>
      </div>
      <p v-if="loadError" class="chub-status-error">{{ t.deviceHub.statusUnavailable }}</p>
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted } from 'vue'
import { deviceRuntimeReady } from '../../composables/deviceHubStatus.js'
import { useDeviceRuntime } from '../../composables/useDeviceRuntime.js'
import { useI18n } from '../../desktop/i18n/index.js'
const { t } = useI18n()
const { runtime, loading, loaded, loadError, refresh } = useDeviceRuntime()
const hostReady = computed(() => deviceRuntimeReady(runtime.ds, runtime.mic))
onMounted(() => refresh())
</script>
