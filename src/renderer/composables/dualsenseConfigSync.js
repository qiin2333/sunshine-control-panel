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

export const createLatestIntentQueue = (run, { debounceMs = 0 } = {}) => {
  let active = null
  let pending
  let intentVersion = 0
  const state = {
    hasPending: () => pending !== undefined,
    peekPending: () => pending,
  }

  const waitForSettledIntent = async () => {
    if (debounceMs <= 0) return
    let observedVersion
    do {
      observedVersion = intentVersion
      await new Promise((resolve) => setTimeout(resolve, debounceMs))
    } while (observedVersion !== intentVersion)
  }

  const submit = (intent) => {
    pending = intent
    intentVersion += 1
    if (!active) {
      active = (async () => {
        while (pending !== undefined) {
          if (debounceMs > 0) await waitForSettledIntent()
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
