<template>
  <section class="enhancement-card">
    <header class="component-page-header">
      <div class="component-title-row">
        <span class="enhancement-icon"
          ><el-icon
            ><component :is="kind === 'nr' ? MagicStick : Sunny" /></el-icon
        ></span>
        <h2>{{ kind === 'nr' ? 'DLSS NR' : 'RTX HDR' }}</h2>
      </div>
      <p class="component-intro">{{ kind === 'nr' ? controlsText.nrIntro : controlsText.hdrIntro }}</p>
    </header>

    <article class="component-window" :class="`state-${status.state}`">
      <div class="component-hud-row" aria-live="polite">
        <div class="component-hud-state">
          <span class="component-status-dot" aria-hidden="true"></span>
          <strong class="component-hud-label">{{ stateLabel }}</strong>
        </div>
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
            >{{ actionLabel }}</el-button
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
      </div>
      <div class="component-headline">
        <el-switch
          :model-value="status.enabled"
          :aria-label="text.enable"
          :disabled="
            !statusKnown ||
            !status.installed ||
            (!status.enabled && status.state === 'repair_required') ||
            status.maintenance ||
            controlsBusy
          "
          :loading="operation === 'saving'"
          :active-text="text.enable"
          @change="setEnabled"
        />
        <p>{{ text.enableHint }}</p>
        <el-button v-if="kind === 'nr' && status.enabled" @click="openApplicationSettings">{{
          text.appSettings
        }}</el-button>
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
      <summary>{{ text.health }}</summary>
      <p class="component-intro">{{ text.intro }}</p>
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
      <footer class="component-panel-footer">
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
const text = computed(() =>
  props.kind === 'nr' ? dlssNrText(locale.value) : hdrText.value
)

const {
  status,
  statusKnown,
  refreshing,
  operation,
  operationError,
  controlsBusy,
  stateLabel,
  actionLabel,
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
</script>

<style scoped lang="less">
.enhancement-card {
  padding: 24px;
  border: 1px solid var(--el-border-color-light);
  border-radius: 16px;
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
}
.component-title-row {
  display: flex;
  align-items: center;
  gap: 12px;
  h2 {
    margin: 0;
    font-size: 19px;
    font-weight: 600;
  }
}
.enhancement-icon {
  display: inline-flex;
  padding: 10px;
  border-radius: 12px;
  color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  font-size: 22px;
}
.component-intro {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  line-height: 1.7;
  margin: 12px 0 18px;
}
.component-hud-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  padding: 12px 0;
  border-top: 1px solid var(--el-border-color-lighter);
}
.component-hud-state {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}
.component-hud-label {
  font-weight: 500;
}
.component-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--el-text-color-placeholder);
}
.state-active .component-status-dot {
  background: var(--el-color-success);
}
.state-degraded .component-status-dot {
  background: var(--el-color-warning);
}
.component-hud-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.component-headline {
  padding: 16px;
  border-radius: 10px;
  background: var(--el-fill-color-light);
  p {
    color: var(--el-text-color-secondary);
    line-height: 1.6;
    font-size: 13px;
    margin: 8px 0;
  }
}
.component-notice {
  margin-top: 12px;
}
.component-details {
  margin-top: 18px;
  border-top: 1px solid var(--el-border-color-lighter);
  padding-top: 16px;
  summary {
    cursor: pointer;
    font-weight: 500;
    font-size: 13px;
  }
}
.component-health-row {
  display: flex;
  justify-content: space-between;
  gap: 20px;
  padding: 12px 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
  font-size: 12px;
}
.component-health-key {
  color: var(--el-text-color-secondary);
}
.component-health-state {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  text-align: right;
  i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--el-text-color-placeholder);
  }
  i.ok {
    background: var(--el-color-success);
  }
  i.bad {
    background: var(--el-color-danger);
  }
  i.warn {
    background: var(--el-color-warning);
  }
}
.component-panel-footer {
  padding-top: 14px;
  display: flex;
  justify-content: flex-end;
}
.enhancement-card {
  position: relative;
  border-radius: 17px 21px 18px 15px / 18px 16px 22px 19px;
  &::after {
    content: '';
    position: absolute;
    top: 14px;
    right: 17px;
    width: 23px;
    height: 7px;
    border-top: 1px solid var(--el-color-primary-light-7);
    border-bottom: 1px solid var(--el-color-primary-light-8);
    border-radius: 45%;
    transform: rotate(-13deg);
    pointer-events: none;
  }
}
.enhancement-icon {
  border: 1px solid var(--el-color-primary-light-8);
  border-radius: 12px 15px 11px 14px;
}
.component-details { border-top-style: dashed; }

@media (max-width: 600px) {
  .enhancement-card {
    padding: 18px;
  }
  .component-hud-actions {
    width: 100%;
  }
}
</style>
