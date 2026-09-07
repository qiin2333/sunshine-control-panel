<template>
  <section>
    <header class="ds5-page-header">
      <div class="ds5-title-row">
        <h2>{{ text.title }}</h2>
        <el-tag class="ds5-tag-exp" effect="plain">{{ text.localOnly }}</el-tag>
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
            :disabled="!status.bridge_present || status.in_use || status.maintenance || controlsBusy"
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
        <p>{{ text.boundary }}</p>
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
      v-if="statusKnown && !status.bridge_present"
      class="ds5-notice"
      type="warning"
      :title="text.bridgeMissingNotice"
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
      <div v-for="item in healthRows" :key="item.label" class="ds5-health-row">
        <span class="ds5-health-key">{{ item.label }}</span>
        <span class="ds5-health-state"><i :class="item.tone" aria-hidden="true"></i>{{ item.state }}</span>
        <span class="ds5-health-detail">{{ item.detail }}</span>
      </div>
      <footer class="ds5-panel-footer">
        <el-button v-if="status.installed" link @click="openFolder">{{ text.openFolder }}</el-button>
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

    <section class="ds5-section">
      <div class="ds5-section-head">
        <span class="ds5-section-label">◈ {{ text.securityTitle }}</span>
        <span class="ds5-section-rule"></span>
      </div>
      <p class="ds5-tuning-hint">{{ text.securityHint }}</p>
    </section>

    <section class="ds5-section">
      <div class="ds5-section-head">
        <span class="ds5-section-label">◈ {{ text.ownershipTitle }}</span>
        <span class="ds5-section-rule"></span>
      </div>
      <div class="rtx-ownership-copy">
        <p>{{ text.ownershipBridge }}</p>
        <p>{{ text.ownershipIntegration }}</p>
        <p>{{ text.ownershipNvidia }}</p>
        <p>{{ text.ownershipNvidiaTerms }}</p>
        <p>{{ text.ownershipScope }}</p>
        <p>{{ text.ownershipTrademark }}</p>
        <p class="ds5-tuning-hint">{{ text.licenseReferenceHint }}</p>
      </div>
      <div class="rtx-reference-links">
        <button
          v-for="reference in officialReferences"
          :key="reference.url"
          type="button"
          class="rtx-reference-link"
          @click="openReference(reference.url)"
        >
          <strong>{{ reference.label }}</strong>
          <span>{{ reference.description }}</span>
        </button>
      </div>
    </section>
  </section>
</template>

<script setup>
import { computed } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { useRtxHdrI18n } from '../composables/rtxHdrI18n.js'
import { useRtxHdrManager } from '../composables/useRtxHdrManager.js'
import { openExternalUrl } from '../tauri-adapter.js'

const text = useRtxHdrI18n()
const officialReferences = computed(() => [
  {
    label: text.value.referenceProjectRepository,
    description: text.value.referenceProjectRepositoryDescription,
    url: 'https://github.com/AlkaidLab/foundation-sunshine',
  },
  {
    label: text.value.referenceFoundationLicense,
    description: text.value.referenceFoundationLicenseDescription,
    url: 'https://github.com/AlkaidLab/foundation-sunshine/blob/master/LICENSE',
  },
  {
    label: text.value.referenceRtxVideo,
    description: text.value.referenceRtxVideoDescription,
    url: 'https://developer.nvidia.com/rtx-video-sdk',
  },
  {
    label: text.value.referenceRtxVideoDownload,
    description: text.value.referenceRtxVideoDownloadDescription,
    url: 'https://developer.nvidia.com/rtx-video-sdk/getting-started',
  },
  {
    label: text.value.referenceNgx,
    description: text.value.referenceNgxDescription,
    url: 'https://docs.nvidia.com/ngx/latest/programming-guide/',
  },
  {
    label: text.value.referenceLicense,
    description: text.value.referenceLicenseDescription,
    url: 'https://developer.download.nvidia.com/gameworks/NVIDIA-RTX-SDKs-License-23Jan2023.pdf',
  },
  {
    label: text.value.referenceNotification,
    description: text.value.referenceNotificationDescription,
    url: 'https://developer.nvidia.com/sw-notification',
  },
  {
    label: text.value.referenceTrademark,
    description: text.value.referenceTrademarkDescription,
    url: 'https://www.nvidia.com/en-us/about-nvidia/company-policies/',
  },
])

const openReference = async (url) => {
  if (!await openExternalUrl(url)) ElMessage.error(text.value.openReferenceFailed)
}

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
  recover,
  openFolder,
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

.rtx-ownership-copy {
  color: var(--el-text-color-regular);
  font-size: 14px;
  line-height: 1.65;
}

.rtx-ownership-copy p {
  margin: 0 0 8px;
}

.rtx-reference-links {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr));
  margin-top: 12px;
}

.rtx-reference-link {
  align-items: flex-start;
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  color: var(--el-text-color-regular);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  font: inherit;
  gap: 4px;
  min-width: 0;
  padding: 10px 12px;
  text-align: left;
}

.rtx-reference-link:hover,
.rtx-reference-link:focus-visible {
  border-color: var(--el-color-primary);
}

.rtx-reference-link strong {
  color: var(--el-color-primary);
}

.rtx-reference-link span {
  font-size: 13px;
  line-height: 1.45;
}
</style>
