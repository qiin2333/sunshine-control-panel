import { nrOverlayState } from './nrOverlayState.js'

// Validate the target again when committing an edit: a newly selected stream
// must never receive a value drafted for the previous one.
export function nrSessionRequest(pipeline, targetId, patch, online = true) {
  if (!pipeline || pipeline.id !== targetId) throw new Error('nr_session_ended')
  if (!online || !pipeline.nr_toggle_supported)
    throw new Error('nr_toggle_unsupported')
  if (
    ['warming_up', 'stopping', 'scaling'].includes(
      nrOverlayState(pipeline, online)
    )
  ) {
    throw new Error('nr_settings_pending')
  }
  const enabled = patch.enabled ?? pipeline.nr_requested_enabled
  const request = { id: targetId, enabled }
  if (patch.scalePercent !== undefined) {
    if (!(pipeline.nr_live_controls_version >= 2) || !enabled)
      throw new Error('nr_toggle_unsupported')
    if (
      !Number.isInteger(patch.scalePercent) ||
      patch.scalePercent < 20 ||
      patch.scalePercent > 100 ||
      patch.scalePercent % 5
    ) {
      throw new Error('nr_request_invalid')
    }
    request.scalePercent = patch.scalePercent
  }
  return request
}
