export function deviceRuntimeReady(dualsenseStatus = {}, microphoneStatus = {}) {
  return Boolean(dualsenseStatus.verified || microphoneStatus.component_available)
}

export function microphoneStatusTone(status = {}) {
  if (status.error_code || status.state === 'faulted') return 'state-error'
  if (status.device_created || status.online) return 'state-ready'
  return ''
}

export function canTestMicrophone(status = {}, loading = false) {
  return Boolean(
    !loading
    && status.component_available
    && status.configured_backend
    && status.configured_backend !== 'disabled',
  )
}

export function microphoneOverviewState(status = {}) {
  if (!status.component_available) return 'missing'
  if (status.host_streaming) return 'capturing'
  if (status.device_created) return 'idle'
  return 'waiting'
}
