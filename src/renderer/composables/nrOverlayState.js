export function nrOverlayState(pipeline, online) {
  if (!online) return 'unavailable'
  if (!pipeline) return 'idle'
  if (!pipeline.nr_toggle_supported) return 'blocked'
  if (pipeline.nr_requested_enabled) {
    if (pipeline.nr_state === 'active') {
      const changingScale = pipeline.nr_requested_scale_percent !== undefined &&
        pipeline.nr_requested_scale_percent !== pipeline.nr_scale_percent
      return changingScale ? 'scaling' : 'active'
    }
    return pipeline.nr_state === 'degraded' ? 'degraded' : 'warming_up'
  }
  return ['active', 'warming_up'].includes(pipeline.nr_state) ? 'stopping' : 'disabled'
}

export function overlayOpacity(value) {
  const number = Number(value)
  return Number.isFinite(number) && value !== null ? Math.max(35, Math.min(95, number)) : 62
}

export function nrProcessingSize(pipeline, percent) {
  const width = pipeline?.nr_source_width, height = pipeline?.nr_source_height
  if (!width || !height) return ''
  return `${Math.max(1, Math.round(width * percent / 100))} × ${Math.max(1, Math.round(height * percent / 100))}`
}

// Validate the host response before reactive templates dereference session fields.
export function nrPipelines(value) {
  if (!Array.isArray(value)) return []
  return value.filter(item => item !== null && typeof item === 'object' &&
    Number.isSafeInteger(item.id) && item.id > 0 && typeof item.hdr_mode === 'string')
}
