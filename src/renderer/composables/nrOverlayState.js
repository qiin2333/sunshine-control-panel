export function nrOverlayState(pipeline, online) {
  if (!online) return 'unavailable'
  if (!pipeline) return 'idle'
  if (!pipeline.nr_toggle_supported) return 'blocked'
  if (pipeline.nr_requested_enabled) {
    if (pipeline.nr_state === 'active') return 'active'
    return pipeline.nr_state === 'degraded' ? 'degraded' : 'warming_up'
  }
  return ['active', 'warming_up'].includes(pipeline.nr_state) ? 'stopping' : 'disabled'
}

export function overlayOpacity(value) {
  const number = Number(value)
  return Number.isFinite(number) && value !== null ? Math.max(35, Math.min(95, number)) : 72
}
