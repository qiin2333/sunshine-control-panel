import { computed, onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { rtxHdr } from '../tauri-adapter.js'
import { useRtxHdrI18n } from './rtxHdrI18n.js'

const emptyStatus = () => ({
  state: 'loading',
  installed: false,
  ready: false,
  in_use: false,
  backend_present: false,
  runtime_present: false,
  configured: false,
  managed_path: '',
  backend_sha256: '',
  runtime_sha256: '',
  detail: '',
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
  const shortHash = (value) => value ? `${value.slice(0, 12)}…` : text.value.notAvailable
  const healthRows = computed(() => [
    {
      label: text.value.backend,
      state: status.value.backend_present ? text.value.present : text.value.missing,
      detail: shortHash(status.value.backend_sha256),
      tone: status.value.backend_present ? 'ok' : 'bad',
    },
    {
      label: text.value.runtime,
      state: status.value.runtime_present ? text.value.present : text.value.missing,
      detail: shortHash(status.value.runtime_sha256),
      tone: status.value.runtime_present ? 'ok' : 'bad',
    },
    {
      label: text.value.configuration,
      state: status.value.configured ? text.value.configured : text.value.notConfigured,
      detail: status.value.managed_path,
      tone: status.value.configured ? 'ok' : 'warn',
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
    if (controlsBusy.value || status.value.in_use) return
    let backendPath
    let runtimePath
    try {
      backendPath = await selectDll(text.value.selectBackend, 'foundation_truehdr_backend.dll')
      if (!backendPath) return
      runtimePath = await selectDll(text.value.selectRuntime, 'nvngx_truehdr.dll')
      if (!runtimePath) return
      await ElMessageBox.confirm(
        text.value.installConfirm,
        status.value.installed ? text.value.repairTitle : text.value.installTitle,
        { type: 'warning', confirmButtonText: actionLabel.value },
      )
    } catch {
      return
    }

    operation.value = 'install'
    operationError.value = ''
    try {
      const result = await rtxHdr.install(backendPath, runtimePath)
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
    if (controlsBusy.value || status.value.in_use || !status.value.installed) return
    try {
      await ElMessageBox.confirm(text.value.uninstallConfirm, text.value.uninstallTitle, {
        type: 'warning',
        confirmButtonText: text.value.uninstall,
      })
    } catch {
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

  const openFolder = async () => {
    const directory = status.value.managed_path.replace(/[\\/]foundation_truehdr_backend\.dll$/i, '')
    if (!directory) return
    try {
      await invoke('open_local_path', { path: directory })
    } catch (error) {
      operationError.value = String(error?.message || error)
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
    openFolder,
  }
}
