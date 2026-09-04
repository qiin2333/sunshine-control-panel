<template>
  <section class="ds5-page">
    <header class="ds5-page-header">
      <p class="ds5-eyebrow">MANAGE / RTX HDR</p>
      <div class="ds5-title-row">
        <h1>{{ text.title }}</h1>
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
          <el-button
            v-if="statusKnown"
            text
            type="primary"
            class="ds5-action"
            :loading="operation === 'install'"
            :disabled="status.in_use || controlsBusy"
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
    </article>

    <el-alert
      v-if="status.in_use"
      class="ds5-notice"
      type="warning"
      :title="text.inUseNotice"
      :closable="false"
      show-icon
    />
    <el-alert
      v-if="operationError || status.detail"
      class="ds5-notice"
      type="error"
      :title="text.technicalDetails"
      :description="operationError || status.detail"
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
          :disabled="status.in_use || controlsBusy"
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
  openFolder,
} = useRtxHdrManager()
</script>

<style scoped lang="less">
@import '../styles/DualSenseSettings.less';
</style>
