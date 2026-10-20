export function dualSenseComponentAction(status = {}) {
  if (!status.installed) return 'install'
  if (!status.verified) return 'repair'
  if (status.update_available) return 'update'
  return ''
}

export function dualSenseComponentOperational(status = {}) {
  return Boolean(status.verified)
}

/** Requires explicit shared cleanup for residual devices, unsupported versions or ABI conflicts. */
export function usbipTransportNeedsCleanup(status = {}) {
  return Boolean(
    status.vhci_residual
    // error_code 是后端下发的结构化码（Rust status::detail_error_code），
    // 不得回退到解析 detail 文本。
    || (status.installed && (!status.version_valid || status.error_code === 'USBIP-EXEC-006')),
  )
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

/** Selects the maintenance step without allowing component installation to replace shared drivers. */
export function usbTransportPlan(status = {}) {
  if (status.ready) return 'reuse'
  if (usbipTransportNeedsCleanup(status)) return 'cleanup'
  if (status.installed) return 'repair'
  return 'install'
}
