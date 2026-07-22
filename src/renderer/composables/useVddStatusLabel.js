import { computed, unref } from 'vue'

const VDD_STATUS_LABEL_KEYS = Object.freeze({
  ready: 'driverStateReady',
  degraded: 'driverStateDegraded',
  not_installed: 'driverStateNotInstalled',
  unhealthy: 'driverStateUnhealthy',
  reboot_required: 'driverStateRebootRequired',
  payload_missing: 'driverStatePayloadMissing',
  unsupported: 'driverStateUnsupported',
  unknown: 'driverStateUnknown',
})

export function useVddStatusLabel(t, status) {
  return computed(() => {
    const state = unref(status)?.state
    const hasState = Object.prototype.hasOwnProperty.call(VDD_STATUS_LABEL_KEYS, state)
    const key = hasState ? VDD_STATUS_LABEL_KEYS[state] : VDD_STATUS_LABEL_KEYS.unknown
    return unref(t).vddSettings[key]
  })
}
