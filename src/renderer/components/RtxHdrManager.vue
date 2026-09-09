<template>
  <section>
    <header class="ds5-page-header">
      <div class="ds5-title-row">
        <h2>{{ text.title }}</h2>
      </div>
      <p class="ds5-intro">{{ text.intro }}</p>
    </header>

    <article class="ds5-window" :class="`state-${status.state}`">
      <span class="ds5-window-tab">◈ {{ text.componentTitle }}</span>
      <div class="ds5-hud-row" aria-live="polite">
        <div class="ds5-hud-state">
          <span class="ds5-status-dot" aria-hidden="true"></span>
          <strong class="ds5-hud-label">{{ stateLabel }}</strong>
        </div>
        <div class="ds5-hud-actions">
          <el-button text class="ds5-action" @click="showAcquisition">{{ text.acquisitionTitle }}</el-button>
          <el-button
            v-if="statusKnown"
            text
            type="primary"
            class="ds5-action"
            :loading="operation === 'install'"
            :disabled="!status.host_supported || !status.adapter_present || status.in_use || status.maintenance || controlsBusy"
            @click="install"
          >{{ actionLabel }}</el-button>
          <el-button
            text
            class="ds5-action"
            :loading="refreshing"
            :disabled="controlsBusy"
            @click="refresh()"
          ><el-icon><Refresh /></el-icon>{{ text.refresh }}</el-button>
        </div>
      </div>
      <div class="ds5-headline">
        <el-switch
          :model-value="status.enabled"
          :disabled="!statusKnown || !status.installed || (!status.enabled && status.state === 'repair_required') || status.maintenance || controlsBusy"
          :loading="operation === 'saving'"
          :active-text="text.enable"
          @change="setEnabled"
        />
        <p>{{ text.enableHint }}</p>
      </div>
    </article>

    <el-alert
      v-if="statusKnown && !status.host_supported"
      class="ds5-notice"
      type="warning"
      :title="text.bridgeMissingNotice"
      :closable="false"
      show-icon
    />
    <el-alert
      v-else-if="statusKnown && !status.adapter_present"
      class="ds5-notice"
      type="warning"
      :title="text.adapterMissingNotice"
      :closable="false"
      show-icon
    />
    <div v-else-if="statusKnown && !status.vc_runtime_present" class="ds5-notice">
      <el-alert
        type="warning"
        :title="text.vcRuntimeMissingNotice"
        :closable="false"
        show-icon
      />
      <el-button class="mt-2" type="primary" plain @click="openVcRuntimeDownload">
        {{ text.vcRuntimeDownload }}
      </el-button>
    </div>
    <el-alert
      v-else-if="statusKnown && !status.runtime_present"
      class="ds5-notice"
      type="info"
      :title="text.runtimeMissingNotice"
      :closable="false"
      show-icon
    />
    <el-alert v-if="status.maintenance" class="ds5-notice" type="warning" :title="text.maintenanceNotice" :closable="false">
      <el-button :disabled="controlsBusy" :loading="operation === 'recovering'" @click="recover">{{ text.recover }}</el-button>
    </el-alert>
    <el-alert
      v-if="status.in_use"
      class="ds5-notice"
      type="warning"
      :title="text.inUseNotice"
      :closable="false"
      show-icon
    />
    <el-alert
      v-if="operationError"
      class="ds5-notice"
      type="error"
      :title="text.technicalDetails"
      :description="operationError"
      show-icon
      @close="operationError = ''"
    />

    <section class="ds5-section">
      <div class="ds5-section-head">
        <span class="ds5-section-label">◈ {{ text.health }}</span>
        <span class="ds5-section-rule"></span>
      </div>
      <div v-for="item in healthRows" :key="item.label" class="ds5-health-row rtx-health-row">
        <span class="ds5-health-key">{{ item.label }}</span>
        <span class="ds5-health-state"><i :class="item.tone" aria-hidden="true"></i>{{ item.state }}</span>
      </div>
      <footer class="ds5-panel-footer">
        <el-button
          v-if="status.installed"
          link
          type="danger"
          :loading="operation === 'uninstall'"
          :disabled="status.in_use || status.maintenance || controlsBusy"
          @click="uninstall"
        >{{ text.uninstall }}</el-button>
      </footer>
    </section>

  </section>
</template>

<script setup>
import { Refresh } from '@element-plus/icons-vue'
import { useRtxHdrI18n } from '../composables/rtxHdrI18n.js'
import { useRtxHdrManager } from '../composables/useRtxHdrManager.js'

const text = useRtxHdrI18n()

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
  recover,
} = useRtxHdrManager()
</script>

<style scoped lang="less">
@import '../styles/DualSenseSettings.less';

.state-active .ds5-status-dot {
  background: var(--el-color-success);
  box-shadow: 0 0 0 3px var(--el-color-success-light-8);
}

.state-degraded .ds5-status-dot {
  background: var(--el-color-warning);
  box-shadow: 0 0 0 3px var(--el-color-warning-light-8);
}

.rtx-health-row {
  grid-template-columns: minmax(180px, 1fr) auto;
}
</style>
