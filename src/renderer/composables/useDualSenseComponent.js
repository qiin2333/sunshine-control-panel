import { computed, onMounted, onUnmounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { dualsense, usbip } from '../tauri-adapter.js'
import { friendlyDualSenseError, safeDualSenseTechnicalError } from './dualsenseErrors.js'
import { dualSenseConfigReadable, mergeDualSenseStatus } from './dualsenseConfigSync.js'
import { installSelectedDualSensePackages } from './dualsenseInstallFlow.js'
import { dualSenseComponentAction, dualSenseComponentOperational, usbTransportPlan } from './deviceHubStatus.js'
import { useDeviceComponentOperation } from './useDeviceComponentOperation.js'
import { useI18n } from '../desktop/i18n/index.js'

const STATUS_REFRESH_WAIT_TIMEOUT_MS = 5000

// Component management can run without mounting controller configuration or tests.
export function useDualSenseComponent({ busy = ref(false), onStatus, afterInstall } = {}) {
  const { t } = useI18n()
  const sharedOperation = useDeviceComponentOperation()
  const statusKnown = ref(false)
  const refreshing = ref(false)
  const operation = ref('')
  const operationProgress = ref(0)
  const operationStageKey = ref('preparing')
  const operationError = ref('')
  const expandedSections = ref([])
  const rebootRecommended = ref(false)
  const status = ref({
    state: 'loading', installed: false, verified: false,
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
  let disposed = false
  let statusRefreshGeneration = 0
  let statusRefreshPromise = null

  const controlsBusy = computed(() => sharedOperation.busyElsewhere.value || busy.value || Boolean(operation.value))
  const componentControlsBusy = computed(() => sharedOperation.busyElsewhere.value || Boolean(operation.value))
  const componentAction = computed(() => dualSenseComponentAction(status.value))
  const componentOperational = computed(() => dualSenseComponentOperational(status.value))
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
    if (status.value.verified && status.value.update_available) {
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
      state: status.value.verified && status.value.update_available
        ? t.value.dualSense.updateAvailable
        : status.value.verified ? t.value.dualSense.available : t.value.dualSense.unavailable,
      detail: status.value.verified && status.value.update_available
        ? `${status.value.component_version || t.value.dualSense.unknownVersion} → ${status.value.available_component_version}`
        : status.value.component_version || status.value.error_code,
      tone: status.value.verified && status.value.update_available ? 'warn' : status.value.verified ? 'ok' : status.value.installed ? 'bad' : '',
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

  const refresh = async (quiet = false, allowBusy = false) => {
    if ((!allowBusy && controlsBusy.value) || refreshing.value || (quiet && statusRefreshPromise)) return false
    if (!quiet && statusRefreshPromise) {
      await waitForStatusRefresh()
      if (controlsBusy.value || refreshing.value || statusRefreshPromise) return false
    }
    const refreshGeneration = ++statusRefreshGeneration
    if (!quiet) operationError.value = ''
    if (!quiet) refreshing.value = true
    let refreshed = false
    const request = dualsense.getStatus()
    statusRefreshPromise = request
    try {
      const result = await request
      if (refreshGeneration !== statusRefreshGeneration) return false
      if (result.success) {
        const configReadable = dualSenseConfigReadable(result.data)
        if (configReadable) onStatus?.(result.data, quiet)
        status.value = mergeDualSenseStatus(status.value, result.data)
        statusKnown.value = true
        if (configReadable) {
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

  const install = sharedOperation.wrap(async (packagePaths = []) => {
    if (controlsBusy.value) return
    packagePaths = Array.isArray(packagePaths)
      ? packagePaths.filter((path) => typeof path === 'string' && path)
      : []
    const componentWasInstalled = status.value.installed
    const upgrading = componentAction.value === 'update'
    const localPackage = packagePaths.length > 0
    operation.value = 'confirm-install'
    try {
      const transport = await usbip.getStatus()
      if (!transport?.success) {
        showError(transport?.message, 'install')
        operation.value = ''
        return
      }
      const plan = usbTransportPlan(transport.data)
      const confirmation = localPackage
        ? t.value.dualSense.localInstallConfirm
        : upgrading ? t.value.dualSense.updateConfirm : t.value.dualSense.installConfirm
      await ElMessageBox.confirm(
        `${confirmation}\n\n${t.value.deviceHub.components.transportPlan[plan]}`,
        localPackage
          ? t.value.dualSense.localInstallTitle
          : upgrading ? t.value.dualSense.updateTitle : t.value.dualSense.installTitle,
        {
          type: 'warning',
          confirmButtonText: upgrading ? t.value.dualSense.update : t.value.dualSense.install,
          cancelButtonText: t.value.deviceHub.usb.cancel,
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
    let result
    try {
      sharedOperation.markChanged()
      result = await dualsense.install(packagePaths)
    } catch (error) {
      operation.value = ''
      await refresh(true)
      return showError(error, 'install')
    }
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
      onStatus?.(result.data, false)
    }
    if (afterInstall && !(await afterInstall(result.data, componentWasInstalled))) {
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
  })

  const installFromPackage = async () => {
    if (controlsBusy.value) return
    let selected
    try {
      selected = await open({
        multiple: true,
        directory: false,
        title: t.value.dualSense.selectLocalPackage,
        filters: [{ name: t.value.dualSense.componentPackage, extensions: ['zip', 'exe'] }],
      })
    } catch (error) {
      showError(error, 'packagePicker')
      return
    }
    await installSelectedDualSensePackages({ packagePaths: selected, installPackages: install })
  }

  const uninstall = sharedOperation.wrap(async () => {
    if (controlsBusy.value) return
    operation.value = 'confirm-uninstall'
    try {
      await ElMessageBox.confirm(t.value.dualSense.uninstallConfirm, t.value.dualSense.uninstallTitle, {
        type: 'warning', confirmButtonText: t.value.dualSense.uninstall, cancelButtonText: t.value.deviceHub.usb.cancel,
      })
    } catch {
      operation.value = ''
      return
    }
    invalidateStatusRefresh()
    operation.value = 'uninstall'
    operationError.value = ''
    await waitForStatusRefresh()
    let result
    try {
      sharedOperation.markChanged()
      result = await dualsense.uninstall()
    } catch (error) {
      operation.value = ''
      await refresh(true)
      return showError(error, 'uninstall')
    }
    operation.value = ''
    if (!result.success) {
      await refresh(true)
      return showError(result.message, 'uninstall')
    }
    rebootRecommended.value = false
    status.value = mergeDualSenseStatus(status.value, result.data)
    statusKnown.value = true
    ElMessage.success(t.value.dualSense.uninstallSuccess)
  })

  sharedOperation.onOtherChange(() => refresh(true))
  onMounted(async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event')
      const unlisten = await listen('dualsense-operation-progress', ({ payload }) => {
        operationStageKey.value = payload?.stage || 'preparing'
        operationProgress.value = Number(payload?.progress || 0)
      })
      if (disposed) unlisten()
      else unlistenProgress = unlisten
    } catch {
      // Browser previews do not provide the Tauri event bridge.
    }
    if (disposed) return
    await refresh()
    if (disposed) return
    pollTimer = window.setInterval(() => refresh(true), 30000)
  })
  onUnmounted(() => {
    disposed = true
    invalidateStatusRefresh()
    window.clearInterval(pollTimer)
    unlistenProgress?.()
  })

  return {
    status, statusKnown, refreshing,
    operation, operationProgress, operationStage, operationError,
    controlsBusy, componentControlsBusy, componentAction, componentOperational,
    stateLabel, nextAction, overallVersion, canTestAudioHaptics,
    showNotice, healthRows, safeStatusDetail, expandedSections,
    install, installFromPackage, refresh, uninstall,
    sharedOperation, invalidateStatusRefresh, waitForStatusRefresh, showError,
  }
}
