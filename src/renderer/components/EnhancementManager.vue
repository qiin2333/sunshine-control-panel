<template>
  <section class="enhancement-card">
    <EnhancementSketch :kind="kind" />
    <header class="component-page-header">
      <div class="component-title-row">
        <span class="enhancement-icon"
          ><el-icon
            ><component :is="kind === 'nr' ? MagicStick : Sunny" /></el-icon
        ></span>
        <h2>{{ kind === 'nr' ? 'DLSS NR' : 'RTX HDR' }}</h2>
        <span class="component-hud-state" aria-live="polite"
          ><i
            class="component-status-dot"
            :class="`state-${status.state}`"
            aria-hidden="true"
          /><span>{{ componentState }}</span></span
        >
      </div>
      <p class="component-intro">
        {{ kind === 'nr' ? controlsText.nrIntro : controlsText.hdrIntro }}
      </p>
    </header>
    <div v-if="statusKnown && !status.installed" class="first-setup">
      <p>{{ controlsText.setupHint }}</p>
      <el-button
        type="primary"
        :loading="operation === 'install'"
        :disabled="
          controlsBusy ||
          !status.host_supported ||
          !status.adapter_present ||
          status.in_use ||
          status.maintenance
        "
        @click="install"
        >{{ controlsText.installSetup }}</el-button
      ><el-button text @click="showAcquisition">{{
        controlsText.setupHelp
      }}</el-button>
    </div>

    <article class="component-window" :class="`state-${status.state}`">
      <div v-if="status.installed" class="component-headline">
        <el-switch
          :model-value="status.enabled"
          :aria-label="
            controlsText.allowUse + (kind === 'nr' ? 'DLSS NR' : 'RTX HDR')
          "
          :disabled="
            !statusKnown ||
            !status.installed ||
            (!status.enabled && status.state === 'repair_required') ||
            status.maintenance ||
            controlsBusy
          "
          :loading="operation === 'saving'"
          :active-text="
            controlsText.allowUse + (kind === 'nr' ? 'DLSS NR' : 'RTX HDR')
          "
          @change="setEnabled"
        />
        <p>
          {{
            kind === 'nr'
              ? controlsText.nrPermissionHint
              : controlsText.hdrPermissionHint
          }}
        </p>
        <el-button
          v-if="kind === 'nr' && status.enabled"
          @click="openApplicationSettings"
          >{{ controlsText.appDefaults }}</el-button
        >
      </div>
    </article>

    <el-alert
      v-if="statusKnown && !status.host_supported"
      class="component-notice"
      type="warning"
      :title="text.bridgeMissingNotice"
      :closable="false"
      show-icon
    />
    <el-alert
      v-else-if="statusKnown && !status.adapter_present"
      class="component-notice"
      type="warning"
      :title="text.adapterMissingNotice"
      :closable="false"
      show-icon
    />
    <div
      v-else-if="statusKnown && !status.vc_runtime_present"
      class="component-notice"
    >
      <el-alert
        type="warning"
        :title="text.vcRuntimeMissingNotice"
        :closable="false"
        show-icon
      />
      <el-button
        class="mt-2"
        type="primary"
        plain
        @click="openVcRuntimeDownload"
      >
        {{ text.vcRuntimeDownload }}
      </el-button>
    </div>
    <el-alert
      v-else-if="statusKnown && !status.runtime_present"
      class="component-notice"
      type="info"
      :title="text.runtimeMissingNotice"
      :closable="false"
      show-icon
    />
    <el-alert
      v-if="status.maintenance"
      class="component-notice"
      type="warning"
      :title="text.maintenanceNotice"
      :closable="false"
    >
      <el-button
        :disabled="controlsBusy"
        :loading="operation === 'recovering'"
        @click="recover"
        >{{ text.recover }}</el-button
      >
    </el-alert>
    <el-alert
      v-if="status.in_use"
      class="component-notice"
      type="warning"
      :title="text.inUseNotice"
      :closable="false"
      show-icon
    />
    <el-alert
      v-if="operationError"
      class="component-notice"
      type="error"
      :title="text.technicalDetails"
      :description="operationError"
      show-icon
      @close="operationError = ''"
    />

    <details class="component-details">
      <summary>{{ controlsText.maintenance }}</summary>
      <div class="component-hud-actions">
        <el-button text class="component-action" @click="showAcquisition">{{
          text.acquisitionTitle
        }}</el-button>
        <el-button
          v-if="statusKnown"
          text
          type="primary"
          class="component-action"
          :loading="operation === 'install'"
          :disabled="
            !status.host_supported ||
            !status.adapter_present ||
            status.in_use ||
            status.maintenance ||
            controlsBusy
          "
          @click="install"
          >{{
            status.installed
              ? controlsText.replaceFile
              : controlsText.installSetup
          }}</el-button
        >
        <el-button
          text
          class="component-action"
          :loading="refreshing"
          :disabled="controlsBusy"
          @click="refresh()"
          ><el-icon><Refresh /></el-icon>{{ text.refresh }}</el-button
        >
      </div>
      <div class="maintenance-checks">
        <div
          v-for="item in healthRows"
          :key="item.label"
          class="component-health-row"
        >
          <span class="component-health-key">{{ item.label }}</span>
          <span class="component-health-state"
            ><i :class="item.tone" aria-hidden="true"></i>{{ item.state }}</span
          >
        </div>
      </div>
      <footer class="component-panel-footer">
        <details class="maintenance-notes">
          <summary>{{ controlsText.usageNotes }}</summary>
          <p>{{ text.intro }}</p>
        </details>
        <el-button
          v-if="status.installed"
          link
          type="danger"
          :loading="operation === 'uninstall'"
          :disabled="status.in_use || status.maintenance || controlsBusy"
          @click="uninstall"
          >{{ text.uninstall }}</el-button
        >
      </footer>
    </details>
  </section>
</template>

<script setup>
import { enhancementControlsText } from '../composables/enhancementControls.js'
import EnhancementSketch from './EnhancementSketch.vue'
import { computed } from 'vue'
import { dlssNr, rtxHdr } from '../tauri-adapter.js'
import { dlssNrText } from '../composables/dlssNrMessages.js'
import { useI18n } from '../desktop/i18n/index.js'
import { Refresh, MagicStick, Sunny } from '@element-plus/icons-vue'
import { useRtxHdrI18n } from '../composables/rtxHdrI18n.js'
import { useEnhancementManager } from '../composables/useEnhancementManager.js'

const props = defineProps({ kind: { type: String, default: 'hdr' } })
const { locale } = useI18n()
const controlsText = computed(() => enhancementControlsText(locale.value))
const hdrText = useRtxHdrI18n()
const text = computed(() => ({
  ...(props.kind === 'nr' ? dlssNrText(locale.value) : hdrText.value),
  saveSuccess: controlsText.value.permissionSaved
}))

const {
  status,
  statusKnown,
  refreshing,
  operation,
  operationError,
  controlsBusy,
  stateLabel,
  healthRows,
  refresh,
  install,
  uninstall,
  setEnabled,
  showAcquisition,
  openVcRuntimeDownload,
  openApplicationSettings,
  recover
} = useEnhancementManager({
  api: props.kind === 'nr' ? dlssNr : rtxHdr,
  messages: text,
  runtimeName: props.kind === 'nr' ? 'nvngx_dlssnr.dll' : 'nvngx_truehdr.dll'
})
const componentState = computed(() => {
  if (
    !statusKnown.value ||
    ['repair_required', 'unavailable', 'degraded'].includes(
      status.value.state
    ) ||
    status.value.maintenance
  )
    return stateLabel.value
  if (!status.value.installed) return controlsText.value.needsInstall
  if (
    !status.value.adapter_present ||
    !status.value.vc_runtime_present ||
    !status.value.runtime_present
  )
    return controlsText.value.needsSetup
  return status.value.enabled
    ? controlsText.value.ready
    : controlsText.value.installedDisabled
})
</script>
