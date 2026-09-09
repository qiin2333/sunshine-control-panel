import { computed, onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { openExternalUrl, rtxHdr } from '../tauri-adapter.js'
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

export function useRtxHdrManager() {
  const text = useRtxHdrI18n()
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
  ])

  const refresh = async (quiet = false) => {
    if (controlsBusy.value && !quiet) return
    if (!quiet) refreshing.value = true
    try {
      const result = await rtxHdr.getStatus()
      if (!result.success) throw new Error(result.message)
      status.value = { ...emptyStatus(), ...result.data }
      statusKnown.value = true
      if (!quiet) operationError.value = ''
    } catch (error) {
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
      runtimePath = await selectDll(text.value.selectRuntime, 'nvngx_truehdr.dll')
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
      const result = await rtxHdr.install(runtimePath)
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
      const result = await rtxHdr.uninstall()
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
      const result = await rtxHdr.setEnabled(enabled)
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

  const openFolder = async () => {
    const directory = status.value.managed_path.replace(/[\\/]nvngx_truehdr\.dll$/i, '')
    if (!directory) return
    try {
      await invoke('open_local_path', { path: directory })
    } catch (error) {
      operationError.value = String(error?.message || error)
    }
  }

  const showAcquisition = () => ElMessageBox.alert(text.value.acquisitionDescription, text.value.acquisitionTitle).catch(() => {})

  const openVcRuntimeDownload = () => openExternalUrl('https://aka.ms/vs/17/release/vc_redist.x64.exe')

  const recover = async () => {
    if (controlsBusy.value) return
    operation.value = 'recovering'
    operationError.value = ''
    try {
      const result = await rtxHdr.recover()
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
    recover,
    openFolder,
  }
}
