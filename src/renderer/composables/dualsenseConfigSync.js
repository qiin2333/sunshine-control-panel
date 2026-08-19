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
