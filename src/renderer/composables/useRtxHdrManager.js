import { computed, onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { openExternalUrl, rtxHdr, sunshine } from '../tauri-adapter.js'
import { useRtxHdrI18n } from './rtxHdrI18n.js'

const emptyStatus = () => ({
  state: 'loading',
  installed: false,
  enabled: false,
  ready: false,
  in_use: false,
  maintenance: false,
  host_supported: false,
  adapter_present: false,
  vc_runtime_present: false,
  runtime_present: false,
  configured: false,
  managed_path: '',
  runtime_sha256: '',
})

export function useRtxHdrManager({ api = rtxHdr, messages, runtimeName = 'nvngx_truehdr.dll' } = {}) {
  const text = messages || useRtxHdrI18n()
  const status = ref(emptyStatus())
  const statusKnown = ref(false)
  const refreshing = ref(false)
  const operation = ref('')
  const operationError = ref('')

  const controlsBusy = computed(() => refreshing.value || Boolean(operation.value))
  const stateLabel = computed(() => text.value.states[status.value.state] || status.value.state)
  const actionLabel = computed(() => status.value.installed ? text.value.repair : text.value.install)
  const healthRows = computed(() => [
    {
      label: text.value.bridge,
      state: status.value.adapter_present ? text.value.present : text.value.missing,
      tone: status.value.adapter_present ? 'ok' : 'bad',
    },
    {
      label: text.value.vcRuntime,
      state: status.value.vc_runtime_present ? text.value.present : text.value.missing,
      tone: status.value.vc_runtime_present ? 'ok' : 'bad',
    },
    {
      label: text.value.runtime,
      state: status.value.runtime_present ? text.value.present : text.value.missing,
      tone: status.value.runtime_present ? 'ok' : 'bad',
    },
  ].map(row => statusKnown.value ? row : { ...row, state: text.value.unknown, tone: 'unknown' }))

  const refresh = async (quiet = false) => {
    if (controlsBusy.value && !quiet) return
    if (!quiet) refreshing.value = true
    if (!statusKnown.value) status.value.state = 'loading'
    try {
      const result = await api.getStatus()
      if (!result.success) throw new Error(result.message)
      status.value = { ...emptyStatus(), ...result.data }
      statusKnown.value = true
      if (!quiet) operationError.value = ''
    } catch (error) {
      if (!statusKnown.value) status.value.state = 'unavailable'
      if (!quiet) operationError.value = String(error?.message || error)
    } finally {
      if (!quiet) refreshing.value = false
    }
  }

  const selectDll = (title, expectedName) => open({
    multiple: false,
    directory: false,
    title,
    filters: [{ name: expectedName, extensions: ['dll'] }],
  })

  const install = async () => {
    if (controlsBusy.value || status.value.in_use || status.value.maintenance) return
    operation.value = 'selecting'
    let runtimePath
    try {
      runtimePath = await selectDll(text.value.selectRuntime, runtimeName)
      if (!runtimePath) {
        operation.value = ''
        return
      }
      await ElMessageBox.confirm(
        text.value.installConfirm,
        status.value.installed ? text.value.repairTitle : text.value.installTitle,
        { type: 'warning', confirmButtonText: actionLabel.value },
      )
    } catch {
      operation.value = ''
      return
    }

    operation.value = 'install'
    operationError.value = ''
    try {
      const result = await api.install(runtimePath)
      if (!result.success) throw new Error(result.message)
      status.value = { ...emptyStatus(), ...result.data }
      statusKnown.value = true
      ElMessage.success(text.value.installSuccess)
    } catch (error) {
      operationError.value = String(error?.message || error)
      await refresh(true)
      ElMessage.error(text.value.installFailed)
    } finally {
      operation.value = ''
    }
  }

  const uninstall = async () => {
    if (controlsBusy.value || status.value.in_use || status.value.maintenance || !status.value.installed) return
    operation.value = 'confirming'
    try {
      await ElMessageBox.confirm(text.value.uninstallConfirm, text.value.uninstallTitle, {
        type: 'warning',
        confirmButtonText: text.value.uninstall,
      })
    } catch {
      operation.value = ''
      return
    }
    operation.value = 'uninstall'
    operationError.value = ''
    try {
      const result = await api.uninstall()
      if (!result.success) throw new Error(result.message)
      status.value = { ...emptyStatus(), ...result.data }
      statusKnown.value = true
      ElMessage.success(text.value.uninstallSuccess)
    } catch (error) {
      operationError.value = String(error?.message || error)
      await refresh(true)
      ElMessage.error(text.value.uninstallFailed)
    } finally {
      operation.value = ''
    }
  }

  const setEnabled = async (enabled) => {
    if (controlsBusy.value || !statusKnown.value || status.value.maintenance) return
    operation.value = 'saving'
    operationError.value = ''
    try {
      const result = await api.setEnabled(enabled)
      if (!result.success) throw new Error(result.message)
      status.value = { ...emptyStatus(), ...result.data }
      statusKnown.value = true
      ElMessage.success(text.value.saveSuccess)
    } catch (error) {
      operationError.value = String(error?.message || error)
      await refresh(true)
    } finally {
      operation.value = ''
    }
  }

  const showAcquisition = () => ElMessageBox.alert(text.value.acquisitionDescription, text.value.acquisitionTitle).catch(() => {})

  const openVcRuntimeDownload = () => openExternalUrl('https://aka.ms/vs/17/release/vc_redist.x64.exe')

  const openApplicationSettings = async () => {
    operationError.value = ''
    try {
      const base = await sunshine.getUrl()
      const opened = await openExternalUrl(new URL('/apps', base).href)
      if (!opened) throw new Error(text.value.appSettingsFailed || 'Could not open the Sunshine application settings')
    } catch (error) {
      operationError.value = String(error?.message || error)
    }
  }

  const recover = async () => {
    if (controlsBusy.value) return
    operation.value = 'recovering'
    operationError.value = ''
    try {
      const result = await api.recover()
      if (!result.success) throw new Error(result.message)
      status.value = { ...emptyStatus(), ...result.data }
      statusKnown.value = true
    } catch (error) {
      operationError.value = String(error?.message || error)
    } finally {
      operation.value = ''
    }
  }

  onMounted(() => refresh())

  return {
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
    openApplicationSettings,
    recover,
  }
}
