export function deviceRuntimeReady(dualsenseStatus = {}, microphoneStatus = {}) {
  return Boolean(dualsenseStatus.verified || microphoneStatus.component_available)
}

export function dualSenseComponentAction(status = {}) {
  if (!status.installed) return 'install'
  if (!status.verified) return 'repair'
  if (status.update_available) return 'update'
  return ''
}

export function dualSenseComponentOperational(status = {}) {
  return Boolean(status.verified)
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
