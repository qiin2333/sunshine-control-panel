<template>
  <section class="chub-panel">
    <div class="chub-window" :class="statusClass">
      <span class="chub-window-tab">{{ t.deviceHub.components.transportTitle }}</span>
      <p class="chub-hint">{{ t.deviceHub.components.transportHint }}</p>
      <div class="chub-hud-row">
        <div class="chub-hud-state">
          <span class="chub-status-dot"></span>
          <strong>{{ statusTitle }}</strong>
        </div>
        <div class="chub-hud-actions">
          <span v-if="status.version" class="chub-hud-version">v{{ status.version }}</span>
          <el-button size="small" :loading="statusLoading" :disabled="transportBusy" @click="refreshStatus()">{{ t.deviceHub.refresh }}</el-button>
        </div>
      </div>

      <p v-if="statusProbeFailed || (status.detail && status.installed)" class="chub-status-error">{{ friendlyError(status.detail) }}</p>
      <div v-if="statusLoaded && !statusProbeFailed && status.supported && !status.ready && !needsCleanup" class="chub-usb-setup">
        <p>{{ t.deviceHub.usb.setupHint }}</p>
        <el-button type="primary" :loading="installing" :disabled="transportBusy" @click="installTransport">
          {{ t.deviceHub.usb.installTransport }}
        </el-button>
      </div>
      <div v-if="statusLoaded && !statusProbeFailed && status.supported && needsCleanup" class="chub-usb-setup chub-usb-residual">
        <p>{{ t.deviceHub.usb.residualHint }}</p>
        <el-button type="danger" :loading="cleaning" :disabled="transportBusy" @click="cleanupResidual">
          {{ t.deviceHub.usb.cleanupResidual }}
        </el-button>
      </div>
      <el-alert
        v-if="status.reboot_recommended"
        class="chub-notice"
        type="warning"
        :closable="false"
        :title="t.deviceHub.usb.rebootRequired"
      />
    </div>

    <details v-if="maintenance && statusLoaded && !statusProbeFailed && status.installed && !needsCleanup" class="chub-diagnostics-details">
      <summary>{{ t.deviceHub.components.maintenance }}</summary>
      <p class="chub-hint">{{ t.deviceHub.components.cleanupHint }}</p>
      <el-button size="small" type="danger" :loading="cleaning" :disabled="transportBusy || attachedDevices.length > 0" @click="cleanupResidual">{{ t.deviceHub.components.uninstallTransport }}</el-button>
      <p v-if="attachedDevices.length" class="chub-hint">{{ t.deviceHub.components.releaseDevices }}</p>
    </details>
  </section>
</template>

<script setup>
import { useI18n } from '../../desktop/i18n/index.js'
const { t } = useI18n()
const { transport } = defineProps({ transport: { type: Object, required: true }, maintenance: Boolean })
const { status, statusLoading, statusLoaded, statusProbeFailed, installing, cleaning, attachedDevices, needsCleanup, transportBusy, statusClass, statusTitle, friendlyError, refreshStatus, installTransport, cleanupResidual } = transport
</script>
