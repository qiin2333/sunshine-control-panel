import { computed, onMounted, onUnmounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { dualsense } from '../tauri-adapter.js'
import {
  dualSenseErrorCode,
  friendlyDualSenseError,
  safeDualSenseTechnicalError,
} from './dualsenseErrors.js'
import {
  createLatestIntentQueue,
  dualSenseConfigAfterInstall,
  dualSenseConfigAfterUninstall,
  dualSenseConfigMatches,
  dualSenseConfigReadable,
  dualSenseConfigUiState,
  mergeDualSenseStatus,
} from './dualsenseConfigSync.js'
import { installSelectedDualSensePackage } from './dualsenseInstallFlow.js'
import { useI18n } from '../desktop/i18n/index.js'

const STATUS_REFRESH_WAIT_TIMEOUT_MS = 5000
const CONFIG_SAVE_DEBOUNCE_MS = 300

/**
 * DualSense 控制器页的全部状态与操作。
 * 组件层只负责布局与事件绑定。
 */
export function useDualSenseSettings() {
  const { t } = useI18n()
  const statusKnown = ref(false)
  const saving = ref(false)
  const refreshing = ref(false)
  const operation = ref('')
  const operationProgress = ref(0)
  const operationStageKey = ref('preparing')
  const operationError = ref('')
  const enabled = ref(false)
  const audioHaptics = ref(false)
  const genshinCompatibility = ref(false)
  const legacyStrength = ref(1)
  const legacyCurve = ref(0.5)
  const legacyNoiseGate = ref(0.02)
  const tuningSaving = ref(false)
  const testCompleted = ref(false)
  const expandedSections = ref([])
  const rebootRecommended = ref(false)
  const status = ref({
    state: 'loading', installed: false, verified: false, enabled: false,
    audio_haptics: false, genshin_compatibility: false,
    genshin_compatibility_available: false,
    component_version: '', available_component_version: '', update_available: false,
    driver_installed: false, usbip_available: false, usbip_version: '',
    usbip_version_valid: false, reboot_recommended: false,
    standard_profile: false, composite_profile: false, in_use: false,
    legacy_strength: 1, legacy_curve: 0.5, legacy_noise_gate: 0.02,
    config_revision: 0, config_readable: false,
    error_code: '', detail: '',
  })
  let pollTimer
  let unlistenProgress
  let statusRefreshGeneration = 0
  let statusRefreshPromise = null
  let tuningSavePromise = null

  const controlsBusy = computed(() => saving.value || tuningSaving.value || Boolean(operation.value))
  const componentControlsBusy = computed(() => Boolean(operation.value))
  const tuningDirty = computed(() =>
    legacyStrength.value !== status.value.legacy_strength
    || legacyCurve.value !== status.value.legacy_curve
    || legacyNoiseGate.value !== status.value.legacy_noise_gate)
  const stateLabel = computed(() => t.value.dualSense.states[status.value.state] || status.value.state)
  const safeStatusDetail = computed(() => status.value.detail
    ? safeDualSenseTechnicalError(status.value.detail)
    : '')
  const operationStage = computed(() => t.value.dualSense.stages[operationStageKey.value] || operationStageKey.value)
  const nextAction = computed(() => {
    if (rebootRecommended.value) {
      return status.value.usbip_available
        ? t.value.dualSense.restartSuggestedAvailable
        : t.value.dualSense.restartSuggestedUnavailable
    }
    return {
      not_installed: t.value.dualSense.nextNotInstalled,
      repair_required: t.value.dualSense.nextRepair,
      update_available: t.value.dualSense.nextUpdate,
      transport_missing: t.value.dualSense.nextTransport,
      in_use: t.value.dualSense.nextInUse,
    }[status.value.state] || t.value.dualSense.nextRepair
  })

  const overallVersion = computed(() => {
    if (status.value.update_available) {
      const installedVersion = status.value.component_version || t.value.dualSense.unknownVersion
      return `${installedVersion} → ${status.value.available_component_version}`
    }
    return status.value.component_version || status.value.runtime_version || status.value.error_code
  })
  const canTestAudioHaptics = computed(() => status.value.composite_profile && status.value.usbip_available)
  const usbTransportDetail = computed(() => {
    if (!status.value.usbip_version_valid) return t.value.dualSense.pinnedTransportRequired
    if (status.value.usbip_available) return `USB/IP ${status.value.usbip_version}`
    if (!status.value.installed) return t.value.dualSense.transportCheckPending
    return t.value.dualSense.transportProbeFailed
  })
  const showNotice = computed(() => rebootRecommended.value || ['not_installed', 'repair_required', 'update_available', 'transport_missing', 'in_use'].includes(status.value.state))
  const healthRows = computed(() => [
    {
      label: t.value.dualSense.component,
      state: status.value.update_available
        ? t.value.dualSense.updateAvailable
        : status.value.verified ? t.value.dualSense.available : t.value.dualSense.unavailable,
      detail: status.value.update_available
        ? `${status.value.component_version || t.value.dualSense.unknownVersion} → ${status.value.available_component_version}`
        : status.value.component_version || status.value.error_code,
      tone: status.value.update_available ? 'warn' : status.value.verified ? 'ok' : status.value.installed ? 'bad' : '',
    },
    {
      label: t.value.dualSense.runtime,
      state: status.value.runtime_version ? t.value.dualSense.available : t.value.dualSense.unavailable,
      detail: status.value.runtime_version,
      tone: status.value.runtime_version ? 'ok' : '',
    },
    {
      label: t.value.dualSense.standard,
      state: status.value.standard_profile ? t.value.dualSense.available : t.value.dualSense.unavailable,
      detail: status.value.in_use ? t.value.dualSense.deviceActive : t.value.dualSense.createdOnDemand,
      tone: status.value.in_use ? 'busy' : status.value.standard_profile ? 'ok' : '',
    },
    {
      label: t.value.dualSense.usbTransport,
      state: status.value.usbip_available ? t.value.dualSense.available : t.value.dualSense.unavailable,
      detail: usbTransportDetail.value,
      tone: status.value.usbip_available ? 'ok' : 'warn',
    },
    {
      label: t.value.dualSense.composite,
      state: status.value.composite_profile && status.value.usbip_available ? t.value.dualSense.available : t.value.dualSense.unavailable,
      detail: status.value.composite_profile ? '4 ch · 48 kHz' : '',
      tone: status.value.composite_profile && status.value.usbip_available ? 'ok' : 'warn',
    },
  ])

  const showError = (message, context = 'generic') => {
    operationError.value = safeDualSenseTechnicalError(message)
    ElMessage.error(friendlyDualSenseError(message, t.value.dualSense.errors, context))
  }

  const invalidateStatusRefresh = () => {
    statusRefreshGeneration += 1
  }

  const waitForStatusRefresh = async () => {
    const pending = statusRefreshPromise
    if (!pending) return true

    let timeoutId
    try {
      return await Promise.race([
        pending.then(() => true),
        new Promise((resolve) => {
          timeoutId = window.setTimeout(() => resolve(false), STATUS_REFRESH_WAIT_TIMEOUT_MS)
        }),
      ])
    } finally {
      if (timeoutId !== undefined) window.clearTimeout(timeoutId)
    }
  }

  const waitForTuningSave = async () => {
    const pending = tuningSavePromise
    if (pending) await pending
  }

  const refresh = async (quiet = false, allowBusy = false) => {
    if ((!allowBusy && controlsBusy.value) || refreshing.value || (quiet && statusRefreshPromise)) return false
    if (!quiet && statusRefreshPromise) {
      await waitForStatusRefresh()
      if (controlsBusy.value || refreshing.value || statusRefreshPromise) return false
    }
    const refreshGeneration = ++statusRefreshGeneration
    if (!quiet) operationError.value = ''
    const preserveTuning = quiet && tuningDirty.value
    if (!quiet) refreshing.value = true
    let refreshed = false
    const request = dualsense.getStatus()
    statusRefreshPromise = request
    try {
      const result = await request
      if (refreshGeneration !== statusRefreshGeneration) return false
      if (result.success) {
        const configReadable = dualSenseConfigReadable(result.data)
        status.value = mergeDualSenseStatus(status.value, result.data)
        statusKnown.value = true
        if (configReadable) {
          enabled.value = result.data.enabled
          audioHaptics.value = result.data.audio_haptics
          genshinCompatibility.value = result.data.genshin_compatibility ?? false
          if (!preserveTuning) {
            legacyStrength.value = result.data.legacy_strength
            legacyCurve.value = result.data.legacy_curve
            legacyNoiseGate.value = result.data.legacy_noise_gate
          }
          refreshed = true
        } else if (!quiet) {
          showError(result.data.detail || result.data.error_code, 'status')
        }
      } else if (!quiet) {
        showError(result.message, 'status')
      }
    } finally {
      if (statusRefreshPromise === request) statusRefreshPromise = null
      if (!quiet) refreshing.value = false
    }
    return refreshed
  }

  const install = async (packagePath = null) => {
    if (controlsBusy.value) return
    packagePath = typeof packagePath === 'string' ? packagePath : null
    const componentWasInstalled = status.value.installed
    const upgrading = Boolean(status.value.installed && status.value.update_available)
    const localPackage = Boolean(packagePath)
    operation.value = 'confirm-install'
    try {
      await ElMessageBox.confirm(
        localPackage
          ? t.value.dualSense.localInstallConfirm
          : upgrading ? t.value.dualSense.updateConfirm : t.value.dualSense.installConfirm,
        localPackage
          ? t.value.dualSense.localInstallTitle
          : upgrading ? t.value.dualSense.updateTitle : t.value.dualSense.installTitle,
        {
          type: 'warning',
          confirmButtonText: upgrading ? t.value.dualSense.update : t.value.dualSense.install,
        },
      )
    } catch {
      operation.value = ''
      return
    }
    invalidateStatusRefresh()
    operation.value = 'install'
    operationError.value = ''
    operationProgress.value = 0
    await waitForStatusRefresh()
    const result = await dualsense.install(packagePath)
    if (!result.success) {
      operation.value = ''
      await refresh(true)
      return showError(result.message, 'install')
    }
    operation.value = ''
    rebootRecommended.value = result.data.reboot_recommended
    const configReadable = dualSenseConfigReadable(result.data)
    status.value = mergeDualSenseStatus(status.value, result.data)
    statusKnown.value = true
    if (configReadable) {
      enabled.value = result.data.enabled
      audioHaptics.value = result.data.audio_haptics
      genshinCompatibility.value = result.data.genshin_compatibility ?? false
      legacyStrength.value = result.data.legacy_strength
      legacyCurve.value = result.data.legacy_curve
      legacyNoiseGate.value = result.data.legacy_noise_gate
    }
    const installedConfig = dualSenseConfigAfterInstall(result.data, componentWasInstalled)
    if (installedConfig && !(await applyComponentConfig(installedConfig))) {
      if (rebootRecommended.value) {
        ElMessage.warning(result.data.usbip_available
          ? t.value.dualSense.restartSuggestedAvailable
          : t.value.dualSense.restartSuggestedUnavailable)
      }
      return
    }
    operationError.value = ''
    ElMessage.success(upgrading ? t.value.dualSense.updateSuccess : t.value.dualSense.installSuccess)
    if (result.data.reboot_recommended) {
      ElMessage.warning(status.value.usbip_available
        ? t.value.dualSense.restartSuggestedAvailable
        : t.value.dualSense.restartSuggestedUnavailable)
    }
  }

  const installFromPackage = async () => {
    if (controlsBusy.value) return
    let selected
    try {
      selected = await open({
        multiple: false,
        directory: false,
        title: t.value.dualSense.selectLocalPackage,
        filters: [{ name: t.value.dualSense.componentPackage, extensions: ['zip'] }],
      })
    } catch (error) {
      showError(error, 'packagePicker')
      return
    }
    if (typeof selected === 'string' && selected) {
      await installSelectedDualSensePackage({ packagePath: selected, installPackage: install })
    }
  }

  const applyConfigControls = (requested) => {
    enabled.value = requested.enabled
    audioHaptics.value = requested.audioHaptics
    genshinCompatibility.value = requested.genshinCompatibility
  }

  const restoreConfirmedConfigControls = () => {
    enabled.value = status.value.enabled
    audioHaptics.value = status.value.audio_haptics
    genshinCompatibility.value = status.value.genshin_compatibility ?? false
  }

  const synchronizeConfirmedConfig = (preserveTuning) => {
    const uiState = dualSenseConfigUiState(status.value, preserveTuning || tuningDirty.value)
    enabled.value = uiState.enabled
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
    const requestedEnabled = requestedConfig.enabled
    const requestedAudioHaptics = requestedConfig.audioHaptics
    const requestedGenshinCompatibility = requestedConfig.genshinCompatibility
    const preserveTuning = tuningDirty.value
    operationError.value = ''
    await waitForStatusRefresh()
    await waitForTuningSave()
    if (queue.hasPending()) return { success: false, preserveTuning }
    let result = await dualsense.setConfig(
      requestedEnabled,
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
        && (!requestedEnabled || status.value.verified)
        && (!requestedEnabled || !requestedAudioHaptics || status.value.usbip_available)
        && (!requestedGenshinCompatibility || status.value.genshin_compatibility_available)
      if (canRetry) {
        result = await dualsense.setConfig(
          requestedEnabled,
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
    status.value = result.data
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
      enabled.value = status.value.enabled
      audioHaptics.value = status.value.audio_haptics
      genshinCompatibility.value = status.value.genshin_compatibility ?? false
      return
    }
    const requestedEnabled = enabled.value
    const requestedAudioHaptics = audioHaptics.value
    const requestedGenshinCompatibility = requestedEnabled && requestedAudioHaptics && genshinCompatibility.value
    const requestedConfig = {
      enabled: requestedEnabled,
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

  const test = async (profile) => {
    if (controlsBusy.value) return
    invalidateStatusRefresh()
    operationError.value = ''
    operation.value = profile
    await waitForStatusRefresh()
    const result = await dualsense.selfTest(profile)
    operation.value = ''
    if (!result.success) return showError(result.message, 'test')
    testCompleted.value = true
    ElMessage.success(t.value.dualSense.testSuccess)
    await refresh()
  }

  const uninstall = async () => {
    if (controlsBusy.value) return
    operation.value = 'confirm-uninstall'
    try {
      await ElMessageBox.confirm(t.value.dualSense.uninstallConfirm, t.value.dualSense.uninstallTitle, {
        type: 'warning', confirmButtonText: t.value.dualSense.uninstall,
      })
    } catch {
      operation.value = ''
      return
    }
    invalidateStatusRefresh()
    operation.value = 'uninstall'
    operationError.value = ''
    await waitForStatusRefresh()
    const result = await dualsense.uninstall()
    operation.value = ''
    if (!result.success) return showError(result.message, 'uninstall')
    rebootRecommended.value = false
    status.value = mergeDualSenseStatus(status.value, result.data)
    statusKnown.value = true
    if (!(await applyComponentConfig(dualSenseConfigAfterUninstall()))) return
    ElMessage.success(t.value.dualSense.uninstallSuccess)
  }

  onMounted(async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event')
      unlistenProgress = await listen('dualsense-operation-progress', ({ payload }) => {
        operationStageKey.value = payload?.stage || 'preparing'
        operationProgress.value = Number(payload?.progress || 0)
      })
    } catch {
      // Browser-only renderer previews do not provide the Tauri event bridge.
    }
    void dualsense.logPanelOpened()
    await refresh()
    pollTimer = window.setInterval(() => {
      refresh(true)
    }, 30000)
  })
  onUnmounted(() => {
    invalidateStatusRefresh()
    window.clearInterval(pollTimer)
    unlistenProgress?.()
  })

  return {
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
  }
}
