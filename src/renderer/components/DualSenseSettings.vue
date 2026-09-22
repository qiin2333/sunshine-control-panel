<template>
  <section class="ds5-page">
    <header v-if="!embedded" class="ds5-page-header">
      <p class="ds5-eyebrow">{{ t.controllers.eyebrow }}</p>
      <div class="ds5-title-row">
        <h1>{{ t.controllers.title }}</h1>
        <el-tag class="ds5-tag-exp" effect="plain">{{ t.dualSense.experimental }}</el-tag>
      </div>
      <p class="ds5-intro">{{ t.controllers.intro }}</p>
    </header>

    <DualSenseComponentPanel :component="component" :title="t.dualSense.title" />
    <el-alert v-if="status.state === 'not_installed'" class="ds5-notice" type="warning" :title="nextAction" :closable="false" show-icon />

    <div v-if="statusKnown && !status.usbip_available" class="ds5-test-actions">
      <el-button size="small" @click="emit('manage-components')">{{ t.deviceHub.components.manageTransport }}</el-button>
    </div>
    <section v-if="componentOperational" class="ds5-section" :aria-label="t.dualSense.profileTitle">
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

    <section v-if="componentOperational" class="ds5-section" :aria-label="t.dualSense.tuningTitle">
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
        <div class="ds5-tuning-field">
          <div class="ds5-tuning-field-head">
            <strong>{{ t.dualSense.tuningStrength }}</strong>
            <span class="ds5-feel-badge">{{ tuningStrengthFeel }}</span>
          </div>
          <small class="ds5-tuning-field-tip">{{ t.dualSense.tuningStrengthTip }}</small>
          <div class="ds5-eq-fader">
            <span>{{ t.dualSense.tuningStrengthHigh }}</span>
            <el-slider
              v-model="legacyStrength" vertical height="128px"
              :aria-label="t.dualSense.tuningStrength"
              :min="0.1" :max="4" :step="0.05" :show-tooltip="false" :disabled="componentControlsBusy"
            />
            <span>{{ t.dualSense.tuningStrengthLow }}</span>
          </div>
          <label class="ds5-exact-value">
            <span>{{ t.dualSense.tuningExactValue }}</span>
            <el-input-number
              v-model="legacyStrength" :min="0.1" :max="4" :step="0.05"
              :precision="2" :controls="false" size="small" :disabled="componentControlsBusy"
            />
          </label>
        </div>
        <div class="ds5-tuning-field">
          <div class="ds5-tuning-field-head">
            <strong>{{ t.dualSense.tuningCurve }}</strong>
            <span class="ds5-feel-badge">{{ tuningCurveFeel }}</span>
          </div>
          <small class="ds5-tuning-field-tip">{{ t.dualSense.tuningCurveTip }}</small>
          <div class="ds5-eq-fader">
            <span>{{ t.dualSense.tuningCurveHigh }}</span>
            <el-slider
              v-model="legacyCurve" vertical height="128px"
              :aria-label="t.dualSense.tuningCurve"
              :min="0.3" :max="2" :step="0.05" :show-tooltip="false" :disabled="componentControlsBusy"
            />
            <span>{{ t.dualSense.tuningCurveLow }}</span>
          </div>
          <label class="ds5-exact-value">
            <span>{{ t.dualSense.tuningExactValue }}</span>
            <el-input-number
              v-model="legacyCurve" :min="0.3" :max="2" :step="0.05"
              :precision="2" :controls="false" size="small" :disabled="componentControlsBusy"
            />
          </label>
        </div>
        <div class="ds5-tuning-field">
          <div class="ds5-tuning-field-head">
            <strong>{{ t.dualSense.tuningGate }}</strong>
            <span class="ds5-feel-badge">{{ tuningGateFeel }}</span>
          </div>
          <small class="ds5-tuning-field-tip">{{ t.dualSense.tuningGateTip }}</small>
          <div class="ds5-eq-fader">
            <span>{{ t.dualSense.tuningGateHigh }}</span>
            <el-slider
              v-model="legacyNoiseGate" vertical height="128px"
              :aria-label="t.dualSense.tuningGate"
              :min="0.002" :max="0.06" :step="0.002" :show-tooltip="false" :disabled="componentControlsBusy"
            />
            <span>{{ t.dualSense.tuningGateLow }}</span>
          </div>
          <label class="ds5-exact-value">
            <span>{{ t.dualSense.tuningExactValue }}</span>
            <el-input-number
              v-model="legacyNoiseGate" :min="0.002" :max="0.06"
              :step="0.002" :precision="3" :controls="false" size="small" :disabled="componentControlsBusy"
            />
          </label>
        </div>
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

    <section v-if="componentOperational" class="ds5-section" :aria-label="t.dualSense.gameCompatibility">
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
          :disabled="!status.genshin_compatibility_available || !status.usbip_available || !audioHaptics || status.in_use || componentControlsBusy"
          @change="setGenshinCompatibility"
        >{{ genshinCompatibility ? t.dualSense.enabledLabel : t.dualSense.disabledLabel }}</el-checkbox>
      </div>
      <p v-if="genshinCompatibility" class="ds5-compat-notice">
        <span aria-hidden="true">!</span>{{ t.dualSense.genshinModeActive }}
      </p>
    </section>

    <section v-if="componentOperational" class="ds5-section">
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

  </section>
</template>

<script setup>
import { useI18n } from '../desktop/i18n/index.js'
import { useDualSenseSettings } from '../composables/useDualSenseSettings.js'
import DualSenseComponentPanel from './controllersHub/DualSenseComponentPanel.vue'

const { t } = useI18n()
const emit = defineEmits(['open-controller-meta', 'manage-components'])
defineProps({ embedded: { type: Boolean, default: false } })
const {
  component,
  audioHaptics, genshinCompatibility,
  legacyStrength, legacyCurve, legacyNoiseGate,
  tuningStrengthFeel, tuningCurveFeel, tuningGateFeel,
  tuningSaving, tuningDirty, testCompleted,
  setAudioHaptics, setGenshinCompatibility,
  applyDefaultPreset, applyErmPreset, saveTuning, test,
} = useDualSenseSettings()
const {
  status, statusKnown, operation, controlsBusy, componentControlsBusy,
  componentOperational, nextAction, canTestAudioHaptics,
} = component
</script>
