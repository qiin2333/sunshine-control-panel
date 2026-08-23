<template>
  <section class="ds5-page">
    <header class="ds5-page-header">
      <p class="ds5-eyebrow">{{ t.controllers.eyebrow }}</p>
      <div class="ds5-title-row">
        <h1>{{ t.controllers.title }}</h1>
        <el-tag class="ds5-tag-exp" effect="plain">{{ t.dualSense.experimental }}</el-tag>
      </div>
      <p class="ds5-intro">{{ t.controllers.intro }}</p>
    </header>

    <article class="ds5-window" :class="`state-${status.state}`">
      <span class="ds5-window-tab">◈ {{ t.dualSense.title }}</span>

      <div class="ds5-hud-row" aria-live="polite">
        <div class="ds5-hud-state">
          <span class="ds5-status-dot" aria-hidden="true"></span>
          <strong class="ds5-hud-label">{{ stateLabel }}</strong>
          <span v-if="overallVersion" class="ds5-hud-version">{{ overallVersion }}</span>
        </div>
        <div class="ds5-hud-actions">
          <el-button
            v-if="statusKnown && !status.installed"
            text
            type="primary"
            class="ds5-action"
            :loading="operation === 'install'"
            :disabled="status.in_use || controlsBusy"
            @click="install()"
          >{{ t.dualSense.install }}</el-button>
          <el-button
            v-else-if="statusKnown && !status.verified"
            text
            type="warning"
            class="ds5-action"
            :loading="operation === 'install'"
            :disabled="status.in_use || controlsBusy"
            @click="install()"
          >{{ t.dualSense.repair }}</el-button>
          <el-button
            v-else-if="statusKnown && status.update_available"
            text
            type="warning"
            class="ds5-action"
            :loading="operation === 'install'"
            :disabled="status.in_use || controlsBusy"
            @click="install()"
          >{{ t.dualSense.update }}</el-button>
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

      <div class="ds5-headline">
        <p>{{ t.dualSense.intro }}</p>
        <el-checkbox
          v-model="enabled"
          class="ds5-enable-control"
          :disabled="!status.verified || status.in_use || componentControlsBusy"
          @change="saveSettings"
        >{{ t.dualSense.enableShort }}</el-checkbox>
      </div>
    </article>

    <el-alert
      v-if="showNotice"
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

    <section v-if="status.verified" class="ds5-section" :aria-label="t.dualSense.profileTitle">
      <div class="ds5-section-head">
        <span class="ds5-section-label">◈ {{ t.dualSense.profileTitle }}</span>
        <span class="ds5-section-rule"></span>
      </div>
      <div class="ds5-mode-list" role="group">
        <div class="ds5-mode-option is-selected is-static">
          <span class="ds5-mode-cursor" aria-hidden="true">▶</span>
          <span>
            <strong>{{ t.dualSense.standardModeShort }}</strong>
            <small>{{ t.dualSense.standardModeTip }}</small>
          </span>
          <span class="ds5-kbd">{{ t.dualSense.included }}</span>
        </div>
        <button
          type="button"
          class="ds5-mode-option"
          :class="{ 'is-selected': audioHaptics }"
          role="checkbox"
          :aria-checked="audioHaptics"
          :disabled="status.in_use || componentControlsBusy || (!status.usbip_available && !audioHaptics)"
          @click="setAudioHaptics(!audioHaptics)"
        >
          <span class="ds5-mode-cursor" aria-hidden="true">▶</span>
          <span>
            <strong>{{ t.dualSense.nativeModeShort }}</strong>
            <small>{{ status.usbip_available ? t.dualSense.nativeModeTip : t.dualSense.nativeUnavailable }}</small>
          </span>
          <span class="ds5-kbd">{{ audioHaptics ? t.dualSense.enabledLabel : t.dualSense.disabledLabel }}</span>
        </button>
      </div>
    </section>

    <section v-if="status.verified" class="ds5-section" :aria-label="t.dualSense.tuningTitle">
      <div class="ds5-section-head">
        <span class="ds5-section-label">◈ {{ t.dualSense.tuningTitle }}</span>
        <span class="ds5-section-rule"></span>
      </div>
      <p class="ds5-tuning-hint">{{ t.dualSense.tuningHint }}</p>

      <span class="ds5-mini-label">{{ t.dualSense.tuningPresetTitle }}</span>
      <div class="ds5-preset-row">
        <button type="button" class="ds5-preset" :disabled="componentControlsBusy" @click="applyDefaultPreset">
          <strong>{{ t.dualSense.tuningPresetDefault }}</strong>
          <small>{{ t.dualSense.tuningPresetDefaultTip }}</small>
        </button>
        <button type="button" class="ds5-preset" :disabled="componentControlsBusy" @click="applyErmPreset">
          <strong>{{ t.dualSense.tuningPresetErm }}</strong>
          <small>{{ t.dualSense.tuningPresetErmTip }}</small>
        </button>
      </div>

      <div class="ds5-tuning-fields">
        <label class="ds5-tuning-field">
          <span>{{ t.dualSense.tuningStrength }}</span>
          <el-input-number v-model="legacyStrength" :min="0.1" :max="4" :step="0.05" size="small" :disabled="componentControlsBusy" />
        </label>
        <label class="ds5-tuning-field">
          <span>{{ t.dualSense.tuningCurve }}</span>
          <el-input-number v-model="legacyCurve" :min="0.3" :max="2" :step="0.05" size="small" :disabled="componentControlsBusy" />
        </label>
        <label class="ds5-tuning-field">
          <span>{{ t.dualSense.tuningGate }}</span>
          <el-input-number
            v-model="legacyNoiseGate" :min="0.002" :max="0.06"
            :step="0.002" :precision="3" size="small" :disabled="componentControlsBusy"
          />
        </label>
      </div>

      <div class="ds5-save-row">
        <el-button
          type="primary"
          class="ds5-save-button"
          :loading="tuningSaving"
          :disabled="controlsBusy || !tuningDirty"
          @click="saveTuning"
        >{{ t.dualSense.tuningSave }}</el-button>
        <el-tag v-if="tuningDirty" class="ds5-unsaved-tag" type="danger" effect="plain">
          {{ t.dualSense.tuningUnsaved }}
        </el-tag>
      </div>
    </section>

    <section v-if="status.verified" class="ds5-section" :aria-label="t.dualSense.gameCompatibility">
      <div class="ds5-section-head">
        <span class="ds5-section-label">◈ {{ t.dualSense.gameCompatibility }}</span>
        <span class="ds5-section-rule"></span>
      </div>
      <div class="ds5-compat-row">
        <div>
          <strong>{{ t.dualSense.genshinMode }}</strong>
          <p class="ds5-tuning-hint" style="margin-bottom: 0">
            {{ status.genshin_compatibility_available ? t.dualSense.genshinModeTip : t.dualSense.genshinModeUnavailable }}
          </p>
        </div>
        <el-checkbox
          v-model="genshinCompatibility"
          :disabled="!status.genshin_compatibility_available || !status.usbip_available || !enabled || !audioHaptics || status.in_use || componentControlsBusy"
          @change="setGenshinCompatibility"
        >{{ genshinCompatibility ? t.dualSense.enabledLabel : t.dualSense.disabledLabel }}</el-checkbox>
      </div>
      <p v-if="genshinCompatibility" class="ds5-compat-notice">
        <span aria-hidden="true">!</span>{{ t.dualSense.genshinModeActive }}
      </p>
    </section>

    <section v-if="status.verified" class="ds5-section">
      <div class="ds5-section-head">
        <span class="ds5-section-label">◈ {{ t.dualSense.validateMode }}</span>
        <span class="ds5-section-rule"></span>
      </div>
      <p class="ds5-tuning-hint">{{ testCompleted ? t.dualSense.validationTip : t.dualSense.validateModeTip }}</p>
      <div class="ds5-test-actions">
        <el-button
          type="primary"
          class="ds5-test-button"
          :loading="operation === 'standard'"
          :disabled="status.in_use || controlsBusy || !status.standard_profile"
          @click="test('standard')"
        >{{ t.dualSense.testStandard }}</el-button>
        <el-button
          type="primary"
          class="ds5-test-button"
          :loading="operation === 'composite'"
          :disabled="status.in_use || controlsBusy || !canTestAudioHaptics"
          @click="test('composite')"
        >{{ t.dualSense.testComposite }}</el-button>
        <el-button text type="success" class="ds5-action" @click="emit('open-controller-meta')">
          {{ t.dualSense.openControllerMeta }}
        </el-button>
      </div>
    </section>

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
import { Refresh } from '@element-plus/icons-vue'
import { useI18n } from '../desktop/i18n/index.js'
import { useDualSenseSettings } from '../composables/useDualSenseSettings.js'

const { t } = useI18n()
const emit = defineEmits(['open-controller-meta'])

const {
  status, statusKnown, saving, refreshing,
  operation, operationProgress, operationStage, operationError,
  enabled, audioHaptics, genshinCompatibility,
  legacyStrength, legacyCurve, legacyNoiseGate,
  tuningSaving, tuningDirty, testCompleted, expandedSections,
  controlsBusy, componentControlsBusy,
  stateLabel, nextAction, overallVersion, canTestAudioHaptics,
  showNotice, healthRows, safeStatusDetail,
  install, installFromPackage, refresh,
  saveSettings, setAudioHaptics, setGenshinCompatibility,
  applyDefaultPreset, applyErmPreset, saveTuning,
  test, uninstall,
} = useDualSenseSettings()
</script>

<style scoped lang="less">
@import '../styles/DualSenseSettings.less';
</style>
