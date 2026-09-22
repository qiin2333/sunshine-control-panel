import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { dualsense } from '../tauri-adapter.js'
import { dualSenseErrorCode } from './dualsenseErrors.js'
import {
  createLatestIntentQueue,
  dualSenseConfigAfterInstall,
  dualSenseConfigMatches,
  dualSenseConfigUiState,
  mergeDualSenseStatus,
} from './dualsenseConfigSync.js'
import { useDualSenseComponent } from './useDualSenseComponent.js'
import { useI18n } from '../desktop/i18n/index.js'

const CONFIG_SAVE_DEBOUNCE_MS = 300

export function useDualSenseSettings() {
  const { t } = useI18n()
  const saving = ref(false)
  const audioHaptics = ref(false)
  const genshinCompatibility = ref(false)
  const legacyStrength = ref(1)
  const legacyCurve = ref(0.5)
  const legacyNoiseGate = ref(0.02)
  const tuningSaving = ref(false)
  const testCompleted = ref(false)
  let tuningSavePromise = null
  const component = useDualSenseComponent({
    busy: computed(() => saving.value || tuningSaving.value),
    onStatus(data, quiet) {
      audioHaptics.value = data.audio_haptics
      genshinCompatibility.value = data.genshin_compatibility ?? false
      if (!quiet || !tuningDirty.value) {
        legacyStrength.value = data.legacy_strength
        legacyCurve.value = data.legacy_curve
        legacyNoiseGate.value = data.legacy_noise_gate
      }
    },
    async afterInstall(data, wasInstalled) {
      const config = dualSenseConfigAfterInstall(data, wasInstalled)
      return !config || await applyComponentConfig(config)
    },
  })
  const {
    status, statusKnown, operation, operationError, controlsBusy,
    sharedOperation, invalidateStatusRefresh, waitForStatusRefresh, showError, refresh,
  } = component
  const tuningDirty = computed(() =>
    legacyStrength.value !== status.value.legacy_strength
    || legacyCurve.value !== status.value.legacy_curve
    || legacyNoiseGate.value !== status.value.legacy_noise_gate)
  const tuningStrengthFeel = computed(() => {
    if (legacyStrength.value < 0.75) return t.value.dualSense.tuningStrengthSoft
    if (legacyStrength.value > 1.5) return t.value.dualSense.tuningStrengthStrong
    return t.value.dualSense.tuningBalanced
  })
  const tuningCurveFeel = computed(() => {
    if (legacyCurve.value < 0.45) return t.value.dualSense.tuningCurveDetailed
    if (legacyCurve.value > 1) return t.value.dualSense.tuningCurvePunchy
    return t.value.dualSense.tuningBalanced
  })
  const tuningGateFeel = computed(() => {
    if (legacyNoiseGate.value < 0.012) return t.value.dualSense.tuningGateSensitive
    if (legacyNoiseGate.value > 0.03) return t.value.dualSense.tuningGateClean
    return t.value.dualSense.tuningBalanced
  })
  const waitForTuningSave = async () => {
    const pending = tuningSavePromise
    if (pending) await pending
  }

  const applyConfigControls = (requested) => {
    audioHaptics.value = requested.audioHaptics
    genshinCompatibility.value = requested.genshinCompatibility
  }

  const restoreConfirmedConfigControls = () => {
    audioHaptics.value = status.value.audio_haptics
    genshinCompatibility.value = status.value.genshin_compatibility ?? false
  }

  const synchronizeConfirmedConfig = (preserveTuning) => {
    const uiState = dualSenseConfigUiState(status.value, preserveTuning || tuningDirty.value)
    audioHaptics.value = uiState.audioHaptics
    genshinCompatibility.value = uiState.genshinCompatibility
    if (uiState.tuning) {
      legacyStrength.value = uiState.tuning.strength
      legacyCurve.value = uiState.tuning.curve
      legacyNoiseGate.value = uiState.tuning.noiseGate
    }
  }

  const applyComponentConfig = async (requestedConfig) => {
    const queue = {
      hasPending: () => false,
      peekPending: () => undefined,
    }
    invalidateStatusRefresh()
    saving.value = true
    applyConfigControls(requestedConfig)
    let outcome
    try {
      outcome = await persistConfigIntent(requestedConfig, queue)
    } catch (error) {
      outcome = { success: false, preserveTuning: tuningDirty.value, message: error }
    }
    saving.value = false
    if (!outcome.success) {
      restoreConfirmedConfigControls()
      showError(outcome.message, 'config')
      return false
    }
    synchronizeConfirmedConfig(outcome.preserveTuning)
    return true
  }

  const persistConfigIntent = async (requestedConfig, queue) => {
    const requestedAudioHaptics = requestedConfig.audioHaptics
    const requestedGenshinCompatibility = requestedConfig.genshinCompatibility
    const preserveTuning = tuningDirty.value
    operationError.value = ''
    await waitForStatusRefresh()
    await waitForTuningSave()
    if (queue.hasPending()) return { success: false, preserveTuning }
    let result = await dualsense.setConfig(
      requestedAudioHaptics,
      requestedGenshinCompatibility,
    )
    let refreshedAfterFailure = false
    if (!result.success && queue.hasPending()) {
      return { success: false, preserveTuning, message: result.message }
    }
    const firstErrorCode = result.success ? '' : dualSenseErrorCode(result.message)
    if (!result.success && ['DS5-CFG-001', 'DS5-CFG-003'].includes(firstErrorCode)) {
      refreshedAfterFailure = await refresh(true, true)
      applyConfigControls(queue.peekPending() ?? requestedConfig)
      if (refreshedAfterFailure && dualSenseConfigMatches(status.value, requestedConfig)) {
        return { success: true, preserveTuning }
      }
      const canRetry = refreshedAfterFailure
        && !queue.hasPending()
        && !status.value.in_use
        && (!requestedAudioHaptics || status.value.verified)
        && (!requestedAudioHaptics || status.value.usbip_available)
        && (!requestedGenshinCompatibility || status.value.genshin_compatibility_available)
      if (canRetry) {
        result = await dualsense.setConfig(
          requestedAudioHaptics,
          requestedGenshinCompatibility,
        )
        refreshedAfterFailure = false
      }
    }
    if (!result.success) {
      if (!refreshedAfterFailure && !queue.hasPending()) {
        refreshedAfterFailure = await refresh(true, true)
        applyConfigControls(queue.peekPending() ?? requestedConfig)
      }
      if (refreshedAfterFailure && dualSenseConfigMatches(status.value, requestedConfig)) {
        return { success: true, preserveTuning }
      }
      return { success: false, preserveTuning, message: result.message }
    }
    status.value = mergeDualSenseStatus(status.value, result.data)
    statusKnown.value = true
    return { success: true, preserveTuning }
  }

  const configSaveQueue = createLatestIntentQueue(async (requestedConfig, queue) => {
    saving.value = true
    applyConfigControls(requestedConfig)
    let outcome
    try {
      outcome = await persistConfigIntent(requestedConfig, queue)
    } catch (error) {
      outcome = { success: false, preserveTuning: tuningDirty.value, message: error }
    }
    const pending = queue.peekPending()
    if (pending) {
      applyConfigControls(pending)
      return
    }

    saving.value = false
    if (!outcome.success) {
      restoreConfirmedConfigControls()
      showError(outcome.message, 'config')
      return
    }
    synchronizeConfirmedConfig(outcome.preserveTuning)
    ElMessage.success(t.value.dualSense.configSuccess)
  }, { debounceMs: CONFIG_SAVE_DEBOUNCE_MS })

  const saveSettings = async () => {
    if (operation.value) {
      audioHaptics.value = status.value.audio_haptics
      genshinCompatibility.value = status.value.genshin_compatibility ?? false
      return
    }
    const requestedAudioHaptics = audioHaptics.value
    const requestedGenshinCompatibility = requestedAudioHaptics && genshinCompatibility.value
    const requestedConfig = {
      audioHaptics: requestedAudioHaptics,
      genshinCompatibility: requestedGenshinCompatibility,
    }
    invalidateStatusRefresh()
    operationError.value = ''
    saving.value = true
    applyConfigControls(requestedConfig)
    await configSaveQueue.submit(requestedConfig)
  }

  const setAudioHaptics = async (next) => {
    if (operation.value || (audioHaptics.value === next && !saving.value)) return
    audioHaptics.value = next
    if (!next) genshinCompatibility.value = false
    testCompleted.value = false
    await saveSettings()
  }

  const applyErmPreset = () => {
    legacyStrength.value = 1
    legacyCurve.value = 0.5
    legacyNoiseGate.value = 0.006
  }

  const applyDefaultPreset = () => {
    legacyStrength.value = 1
    legacyCurve.value = 0.5
    legacyNoiseGate.value = 0.02
  }

  const saveTuning = async () => {
    if (controlsBusy.value) return
    const requestedTuning = {
      strength: legacyStrength.value,
      curve: legacyCurve.value,
      noiseGate: legacyNoiseGate.value,
    }
    invalidateStatusRefresh()
    operationError.value = ''
    tuningSaving.value = true
    const saveOperation = (async () => {
      await waitForStatusRefresh()
      const result = await dualsense.setHapticsTuning(
        requestedTuning.strength, requestedTuning.curve, requestedTuning.noiseGate)
      if (!result.success) return showError(result.message, 'config')
      const tuningChangedWhileSaving = legacyStrength.value !== requestedTuning.strength
        || legacyCurve.value !== requestedTuning.curve
        || legacyNoiseGate.value !== requestedTuning.noiseGate
      if (!tuningChangedWhileSaving) {
        legacyStrength.value = result.data.legacy_strength
        legacyCurve.value = result.data.legacy_curve
        legacyNoiseGate.value = result.data.legacy_noise_gate
      }
      status.value.legacy_strength = result.data.legacy_strength
      status.value.legacy_curve = result.data.legacy_curve
      status.value.legacy_noise_gate = result.data.legacy_noise_gate
      status.value.config_revision = result.data.revision
      if (!tuningChangedWhileSaving) ElMessage.success(t.value.dualSense.tuningSaved)
    })()
    tuningSavePromise = saveOperation
    try {
      await saveOperation
    } finally {
      if (tuningSavePromise === saveOperation) tuningSavePromise = null
      tuningSaving.value = false
    }
  }

  const setGenshinCompatibility = async (value) => {
    if (operation.value) {
      genshinCompatibility.value = status.value.genshin_compatibility ?? false
      return
    }
    genshinCompatibility.value = Boolean(value)
    await saveSettings()
  }

  const test = sharedOperation.wrap(async (profile) => {
    if (controlsBusy.value) return
    invalidateStatusRefresh()
    operationError.value = ''
    operation.value = profile
    await waitForStatusRefresh()
    let result
    try {
      result = await dualsense.selfTest(profile)
    } catch (error) {
      operation.value = ''
      return showError(error, 'test')
    }
    operation.value = ''
    if (!result.success) return showError(result.message, 'test')
    testCompleted.value = true
    ElMessage.success(t.value.dualSense.testSuccess)
    await refresh()
  })

  onMounted(() => dualsense.logPanelOpened().catch(() => {}))

  return {
    component,
    audioHaptics, genshinCompatibility,
    legacyStrength, legacyCurve, legacyNoiseGate,
    tuningStrengthFeel, tuningCurveFeel, tuningGateFeel,
    tuningSaving, tuningDirty, testCompleted,
    setAudioHaptics, setGenshinCompatibility,
    applyDefaultPreset, applyErmPreset, saveTuning, test,
  }
}
