<template>
  <section class="ds5-page">
    <article class="ds5-window" :class="`state-${status.state}`">
      <span class="ds5-window-tab">◈ {{ title || t.deviceHub.components.hostTitle }}</span>
      <p v-if="description" class="ds5-intro">{{ description }}</p>

      <div class="ds5-hud-row" aria-live="polite">
        <div class="ds5-hud-state">
          <span class="ds5-status-dot" aria-hidden="true"></span>
          <strong class="ds5-hud-label">{{ stateLabel }}</strong>
          <span v-if="overallVersion" class="ds5-hud-version">{{ overallVersion }}</span>
        </div>
        <div class="ds5-hud-actions">
          <el-button
            v-if="statusKnown && (componentAction || status.installed)"
            text
            :type="componentAction === 'install' ? 'primary' : 'warning'"
            class="ds5-action"
            :loading="operation === 'install'"
            :disabled="status.in_use || controlsBusy"
            @click="install()"
          >{{ t.dualSense[componentAction || 'repair'] }}</el-button>
          <el-button
            text
            class="ds5-action"
            :loading="refreshing"
            :disabled="refreshing || controlsBusy"
            @click="refresh()"
          >
            <el-icon><Refresh /></el-icon>{{ t.dualSense.refresh }}
          </el-button>
          <el-button
            v-if="statusKnown && (!status.installed || !status.verified || status.update_available)"
            text
            class="ds5-action"
            :disabled="status.in_use || controlsBusy"
            @click="installFromPackage"
          >{{ t.dualSense.installLocalPackage }}</el-button>
        </div>
      </div>

      <article v-if="operation === 'install'" class="ds5-operation-card" aria-live="polite">
        <div><strong>{{ operationStage }}</strong><span>{{ operationProgress }}%</span></div>
        <el-progress :percentage="operationProgress" :show-text="false" />
      </article>

    </article>

    <el-alert
      v-if="showNotice && status.state !== 'not_installed'"
      class="ds5-notice"
      :type="status.state === 'in_use' ? 'error' : 'warning'"
      :title="nextAction"
      :closable="false"
      show-icon
    />

    <el-alert
      v-if="operationError"
      class="ds5-notice"
      type="error"
      :title="t.dualSense.technicalDetails"
      :description="operationError"
      show-icon
      @close="operationError = ''"
    />

    <section class="ds5-section">
      <el-collapse v-model="expandedSections" class="ds5-details-collapse">
        <el-collapse-item name="health" :title="`◈ ${t.dualSense.componentHealth}`">
          <div v-for="item in healthRows" :key="item.label" class="ds5-health-row">
            <span class="ds5-health-key">{{ item.label }}</span>
            <span class="ds5-health-state"><i :class="item.tone" aria-hidden="true"></i>{{ item.state }}</span>
            <span class="ds5-health-detail">{{ item.detail }}</span>
          </div>
          <footer class="ds5-panel-footer">
            <details v-if="safeStatusDetail">
              <summary>{{ status.error_code || t.dualSense.technicalDetails }}</summary>
              <pre>{{ safeStatusDetail }}</pre>
            </details>
            <el-button
              v-if="status.installed"
              link
              type="danger"
              :loading="operation === 'uninstall'"
              :disabled="status.in_use || controlsBusy"
              @click="uninstall"
            >{{ t.dualSense.uninstall }}</el-button>
          </footer>
        </el-collapse-item>
      </el-collapse>
    </section>
  </section>
</template>

<script setup>
import '../../styles/DualSenseSettings.less'
import { Refresh } from '@element-plus/icons-vue'
import { useI18n } from '../../desktop/i18n/index.js'
const { t } = useI18n()
const { component } = defineProps({
  component: { type: Object, required: true },
  title: { type: String, default: '' },
  description: { type: String, default: '' },
})
const {
  status, statusKnown, refreshing,
  operation, operationProgress, operationStage, operationError,
  controlsBusy, componentAction, stateLabel, nextAction, overallVersion,
  showNotice, healthRows, safeStatusDetail, expandedSections,
  install, installFromPackage, refresh, uninstall,
} = component
</script>
