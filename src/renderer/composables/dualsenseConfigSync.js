export const dualSenseConfigUiState = (data, preserveTuning = false) => ({
  enabled: data.enabled,
  audioHaptics: data.audio_haptics,
  genshinCompatibility: data.genshin_compatibility ?? false,
  tuning: preserveTuning
    ? null
    : {
        strength: data.legacy_strength,
        curve: data.legacy_curve,
        noiseGate: data.legacy_noise_gate,
      },
})

export const dualSenseConfigReadable = (data) => data?.config_readable === true

export const dualSenseConfigMatches = (data, requested) =>
  dualSenseConfigReadable(data)
  && data.enabled === requested.enabled
  && data.audio_haptics === requested.audioHaptics
  && (data.genshin_compatibility ?? false) === requested.genshinCompatibility

export const createLatestIntentQueue = (run) => {
  let active = null
  let pending
  const state = {
    hasPending: () => pending !== undefined,
    peekPending: () => pending,
  }

  const submit = (intent) => {
    pending = intent
    if (!active) {
      active = (async () => {
        while (pending !== undefined) {
          const current = pending
          pending = undefined
          await run(current, state)
        }
      })().finally(() => {
        active = null
      })
    }
    return active
  }

  return { submit, ...state }
}

export const mergeDualSenseStatus = (current, incoming) => {
  if (dualSenseConfigReadable(incoming)) return incoming

  return {
    ...incoming,
    enabled: current.enabled,
    audio_haptics: current.audio_haptics,
    genshin_compatibility: current.genshin_compatibility,
    legacy_strength: current.legacy_strength,
    legacy_curve: current.legacy_curve,
    legacy_noise_gate: current.legacy_noise_gate,
    config_revision: current.config_revision,
  }
}
