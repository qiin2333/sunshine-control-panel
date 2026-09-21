<template>
  <section class="live-controls" :aria-busy="busy || pending">
    <div class="live-control-row">
      <div>
        <strong>{{ text.liveTitle }}</strong>
        <p>{{ text.liveHint }}</p>
      </div>
      <el-switch
        :model-value="pipeline.nr_requested_enabled"
        :aria-label="text.liveTitle"
        :disabled="!canChange"
        :loading="busy || pending"
        @change="(enabled) => send({ enabled }, pipeline.id)"
      />
    </div>
    <label v-if="pipeline.nr_live_controls_version >= 2" class="live-scale">
      {{ text.scale }}<strong>{{ draftScale }}%</strong>
      <input
        type="range"
        min="20"
        max="100"
        step="5"
        :value="draftScale"
        :aria-label="text.scale"
        :disabled="!canChange || !pipeline.nr_requested_enabled"
        :style="{ '--range-progress': ((draftScale - 20) / 80) * 100 + '%' }"
        @pointerdown="beginEdit"
        @keydown="beginEdit"
        @input="draftScale = Number($event.target.value)"
        @change="commitScale"
        @blur="cancelEdit"
        @pointercancel="cancelEdit"
      />
    </label>
    <p class="quality-note">{{ text.scaleHint }}</p>
    <p v-if="pending" class="quality-note" role="status">
      {{ overlayText.states[state] }}
    </p>
    <p
      v-if="pipeline.nr_settings_failure_reason"
      class="quality-warning"
      role="status"
    >
      {{ overlayText.settingsFailed }}
    </p>
  </section>
</template>
<script setup>
import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../desktop/i18n/index.js'
import { enhancementControlsText } from '../composables/enhancementControls.js'
import { nrOverlayText } from '../composables/nrOverlayMessages.js'
import { nrOverlayState } from '../composables/nrOverlayState.js'
import { nrSessionRequest } from '../composables/nrSessionControls.js'
const props = defineProps({
  pipeline: { type: Object, required: true },
  online: Boolean
})
const emit = defineEmits(['changed', 'error'])
const { locale } = useI18n()
const text = computed(() => enhancementControlsText(locale.value))
const overlayText = computed(() => nrOverlayText(locale.value))
const busy = ref(false),
  draftScale = ref(100),
  editTarget = ref(null)
const state = computed(() => nrOverlayState(props.pipeline, props.online))
const pending = computed(() =>
  ['warming_up', 'stopping', 'scaling'].includes(state.value)
)
const canChange = computed(
  () =>
    props.online &&
    props.pipeline.nr_toggle_supported &&
    !busy.value &&
    !pending.value
)
const requestedScale = () =>
  props.pipeline.nr_requested_scale_percent ??
  props.pipeline.nr_scale_percent ??
  100
watch(
  () => [props.pipeline.id, requestedScale()],
  ([id], previous) => {
    if (previous && previous[0] !== id) editTarget.value = null
    if (editTarget.value === null) draftScale.value = requestedScale()
  },
  { immediate: true }
)
function beginEdit() {
  if (canChange.value && editTarget.value === null)
    editTarget.value = props.pipeline.id
}
function cancelEdit() {
  editTarget.value = null
  draftScale.value = requestedScale()
}
async function commitScale() {
  const target = editTarget.value
  const value = draftScale.value
  if (target === null) return cancelEdit()
  await send({ scalePercent: value }, target)
  cancelEdit()
}
async function send(patch, target) {
  if (busy.value) return
  try {
    const request = nrSessionRequest(
      props.pipeline,
      target,
      patch,
      props.online
    )
    busy.value = true
    emit('error', '')
    await invoke('nr_live_set_enabled', request)
    emit('changed')
  } catch (error) {
    emit('error', error instanceof Error ? error.message : String(error))
  } finally {
    busy.value = false
  }
}
</script>
