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
  const state = microphoneOverviewState(status)
  if (state === 'faulted') return 'state-error'
  if (['active', 'capturing', 'idle'].includes(state)) return 'state-ready'
  return ''
}

export function canTestMicrophone(status = {}, loading = false) {
  return Boolean(
    !loading
    && status.state !== 'unsupported'
    && ['vb_cable', 'auto', 'usbip_experimental'].includes(status.configured_backend)
    && (status.configured_backend !== 'usbip_experimental' || status.component_available),
  )
}

export function microphoneOverviewState(status = {}) {
  if (status.state === 'unsupported') return 'unsupported'
  if (!status.configured_backend) return 'unknown'
  if (status.configured_backend === 'disabled') return 'disabled'
  // USB endpoint fields do not describe VB-Cable's capture state.
  if (status.active_backend === 'vb_cable') return 'active'
  if (status.configured_backend === 'vb_cable') return 'waiting'
  if (status.error_code || ['faulted', 'device_faulted'].includes(status.state)) return 'faulted'
  if (status.configured_backend === 'auto' && !status.active_backend) return 'waiting'
  if (!status.component_available) return 'missing'
  if (status.host_streaming) return 'capturing'
  if (status.device_created) return 'idle'
  return 'waiting'
}

export function microphoneUsesUsbip(status = {}) {
  return status.active_backend
    ? status.active_backend === 'usbip_experimental'
    : status.configured_backend === 'usbip_experimental'
}

export function usbTransportPlan(status = {}) {
  if (status.ready) return 'reuse'
  if (status.vhci_residual || (status.installed && !status.version_valid)) return 'cleanup'
  if (status.installed) return 'repair'
  return 'install'
}
