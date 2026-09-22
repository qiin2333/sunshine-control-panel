import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { usbip } from '../tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'
import { useDeviceComponentOperation } from './useDeviceComponentOperation.js'

export function useUsbTransport() {
  const { t } = useI18n()
  const sharedOperation = useDeviceComponentOperation()
  const status = reactive({
    supported: true,
    installed: false,
    ready: false,
    version: '',
    version_valid: false,
    reboot_recommended: false,
    vhci_residual: false,
    attached_devices: [],
    detail: '',
  })
  const statusLoading = ref(false)
  const statusLoaded = ref(false)
  const statusProbeFailed = ref(false)
  const installing = ref(false)
  const cleaning = ref(false)
  let statusRefreshPromise = null

  const attachedDevices = computed(() => status.attached_devices || [])
  const needsCleanup = computed(() => Boolean(
    status.vhci_residual || (status.installed && !status.version_valid),
  ))
  const transportBusy = computed(() => Boolean(sharedOperation.busyElsewhere.value || installing.value || cleaning.value))
  const statusClass = computed(() => {
    if (status.ready) return 'state-ready'
    if (!statusLoaded.value) return ''
    if (statusProbeFailed.value) return 'state-error'
    if (!status.supported) return ''
    return status.installed ? 'state-error' : 'state-update_available'
  })
  const statusTitle = computed(() => {
    if (!statusLoaded.value) return t.value.deviceHub.usb.checkingTransport
    if (statusProbeFailed.value) return t.value.deviceHub.usb.statusUnavailable
    if (!status.supported) return t.value.deviceHub.usb.transportUnsupported
    if (status.ready) return t.value.deviceHub.usb.transportReady
    if (!status.installed) return t.value.deviceHub.usb.transportMissing
    return t.value.deviceHub.usb.transportNeedsRepair
  })

  function applyStatus(next) {
    if (next) Object.assign(status, next)
  }

  function friendlyError(error) {
    const message = String(error || '')
    const code = message.match(/^(USBIP-[A-Z]+-\d{3})/)?.[1]
    const translated = code && t.value.deviceHub.usb.errors?.[code]
    return translated || t.value.deviceHub.usb.unknownError
  }

  function applyProbeFailure(detail) {
    statusProbeFailed.value = true
    status.ready = false
    status.attached_devices = []
    status.vhci_residual = false
    status.detail = detail
  }

  async function performStatusRefresh() {
    statusLoading.value = true
    try {
      const result = await usbip.getStatus()
      if (result?.success) {
        statusProbeFailed.value = false
        applyStatus(result.data)
      } else {
        applyProbeFailure(result?.message || '')
      }
    } catch (error) {
      applyProbeFailure(String(error || ''))
    } finally {
      statusLoaded.value = true
      statusLoading.value = false
    }
  }

  async function refreshStatus(options = {}) {
    const afterCurrent = options?.afterCurrent === true
    const activeRefresh = statusRefreshPromise
    if (activeRefresh) {
      await activeRefresh
      if (!afterCurrent) return
    }

    const nextRefresh = performStatusRefresh()
    statusRefreshPromise = nextRefresh
    try {
      await nextRefresh
    } finally {
      if (statusRefreshPromise === nextRefresh) statusRefreshPromise = null
    }
  }

  const installTransport = sharedOperation.wrap(async () => {
    if (transportBusy.value) return
    installing.value = true
    try {
      await ElMessageBox.confirm(
        t.value.deviceHub.usb.installConfirm,
        t.value.deviceHub.usb.installTransport,
        { confirmButtonText: t.value.deviceHub.usb.continue, cancelButtonText: t.value.deviceHub.usb.cancel, type: 'warning' },
      )
    } catch { installing.value = false; return }
    // Do not let an in-flight status refresh apply stale data over the result
    // of the operation that is about to run.
    if (statusRefreshPromise) await statusRefreshPromise
    sharedOperation.markChanged()
    let result
    try {
      result = await usbip.installTransport()
    } catch (error) {
      await refreshStatus()
      return ElMessage.error(friendlyError(error))
    } finally {
      installing.value = false
    }
    if (!result?.success) {
      await refreshStatus()
      return ElMessage.error(friendlyError(result?.message))
    }
    applyStatus(result.data)
    if (result.data?.ready) ElMessage.success(t.value.deviceHub.usb.installSuccess)
    else ElMessage.warning(t.value.deviceHub.usb.rebootRequired)
  })

  const cleanupResidual = sharedOperation.wrap(async () => {
    if (transportBusy.value) return
    const removing = !needsCleanup.value
    cleaning.value = true
    try {
      await ElMessageBox.confirm(
        t.value.deviceHub.usb.cleanupConfirm,
        removing ? t.value.deviceHub.components.uninstallTransport : t.value.deviceHub.usb.cleanupResidual,
        { confirmButtonText: removing ? t.value.dualSense.uninstall : t.value.deviceHub.usb.continue, cancelButtonText: t.value.deviceHub.usb.cancel, type: 'warning' },
      )
    } catch { cleaning.value = false; return }
    if (statusRefreshPromise) await statusRefreshPromise
    sharedOperation.markChanged()
    let result
    try {
      result = await usbip.cleanupTransport()
    } catch (error) {
      await refreshStatus()
      return ElMessage.error(friendlyError(error))
    } finally {
      cleaning.value = false
    }
    if (!result?.success) {
      // The failed cleanup may still have removed some devnodes; refresh so the
      // residual flag reflects the actual state instead of the stale snapshot.
      await refreshStatus()
      return ElMessage.error(friendlyError(result?.message))
    }
    applyStatus(result.data)
    if (result.data?.vhci_residual) ElMessage.error(friendlyError('USBIP-CLEAN-002'))
    else ElMessage.success(removing ? t.value.deviceHub.components.transportRemoved : t.value.deviceHub.usb.cleanupSuccess)
  })

  sharedOperation.onOtherChange(() => refreshStatus({ afterCurrent: true }))
  onMounted(() => refreshStatus())
  return { status, statusLoading, statusLoaded, statusProbeFailed, installing, cleaning, attachedDevices, needsCleanup, transportBusy, statusClass, statusTitle, friendlyError, refreshStatus, installTransport, cleanupResidual }
}
