export const dualSenseConfigUiState = (data, preserveTuning = false) => ({
  enabled: data.enabled,
  audioHaptics: data.audio_haptics,
  tuning: preserveTuning
    ? null
    : {
        strength: data.legacy_strength,
        curve: data.legacy_curve,
        noiseGate: data.legacy_noise_gate,
      },
})

export const dualSenseConfigReadable = (data) => data?.config_readable === true

export const mergeDualSenseStatus = (current, incoming) => {
  if (dualSenseConfigReadable(incoming)) return incoming

  return {
    ...incoming,
    enabled: current.enabled,
    audio_haptics: current.audio_haptics,
    legacy_strength: current.legacy_strength,
    legacy_curve: current.legacy_curve,
    legacy_noise_gate: current.legacy_noise_gate,
    config_revision: current.config_revision,
  }
}
