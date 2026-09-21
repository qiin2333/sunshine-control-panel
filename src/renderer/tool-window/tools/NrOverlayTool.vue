<template>
  <section ref="surface" class="nr-overlay" :class="{ expanded, settled }" :style="{ '--panel-alpha': (expanded ? opacity : Math.max(35, opacity - 12)) / 100 }">
    <header class="nr-head">
      <span class="drag-handle" data-tauri-drag-region @pointerdown="touchDrag.start" @pointermove="touchDrag.move" @pointerup="touchDrag.end" @pointercancel="touchDrag.cancel" @lostpointercapture="touchDrag.cancel" :title="text.drag"><svg viewBox="0 0 16 24" aria-hidden="true"><circle v-for="(point, i) in [[5,6],[11,6],[5,12],[11,12],[5,18],[11,18]]" :key="i" :cx="point[0]" :cy="point[1]" r="1.5" /></svg></span>
      <button class="pill" :aria-expanded="expanded" :aria-label="text.states[state] + (state === 'active' ? ' · ' + (pipeline?.nr_scale_percent ?? 100) + '%' : '')" @click="expanded = !expanded">
        <span class="dot" :class="state" />
        <span>{{ expanded ? 'DLSS NR' : compactLabel }}</span>
        <ArrowDown class="chevron" :class="{ rotated: expanded }" aria-hidden="true" />
      </button>
      <button v-if="expanded" class="close" :aria-label="text.close" :title="text.close" @click="$emit('close')"><Close aria-hidden="true" /></button>
    </header>
    <div class="nr-reveal" :inert="!expanded" :aria-hidden="!expanded">
      <div class="nr-clip">
        <div class="nr-body">
          <select v-if="pipelines.length > 1 || (selectionExpired && pipelines.length)" v-model="selectedId" :aria-label="text.session" :disabled="busy">
            <option :value="null" disabled>{{ text.choose }}</option>
            <option v-for="item in pipelines" :key="item.id" :value="item.id">{{ text.session }} #{{ item.id }} · {{ nrOutputLabel(item) }}</option>
          </select>
          <div class="switch-row">
            <span>{{ text.enhancement }}</span>
            <button class="switch" role="switch" :aria-checked="effectActive" :aria-label="text.enhancement" :class="{ on: effectActive }" :disabled="!canToggle" @click="toggle"><span /></button>
          </div>
          <div class="live-state" aria-live="polite"><span class="dot" :class="state" />{{ text.states[state] }}</div>
          <p v-if="state === 'blocked'" class="notice">{{ pipeline?.backend !== 'none' ? text.hdrBusy : text.unsupported }}</p>
          <p v-else-if="state === 'degraded'" class="notice">{{ text.fallback }}<br><small>{{ pipeline?.nr_reason }}</small></p>
          <p v-else-if="['warming_up', 'stopping', 'scaling'].includes(state)" class="notice">{{ text.wait }}</p>
          <p v-if="error" class="notice error">{{ error }} <button @click="refresh">{{ text.retry }}</button></p>
          <div v-if="supportsControls" class="scale-control">
            <label class="parameter-row"><span>{{ text.scale }}</span><output>{{ draftScale }}%</output>
              <input type="range" min="20" max="100" step="5" :value="draftScale" :disabled="!canToggle"
                :aria-label="text.scale" @input="editSlider('scale', $event)" @change="commitSlider('scale')" @pointercancel="cancelEdit">
            </label>
            <div class="scale-size">{{ editing === 'scale' || !pipeline.nr_requested_enabled ? text.previewSize : text.processing }} · {{ processingSize }}</div>
            <p class="hint">{{ text.scaleHint }}</p>
            <label class="parameter-row"><span>{{ text.intensity }}</span><output>{{ draftIntensity }}%</output>
              <input type="range" min="0" max="100" step="5" :value="draftIntensity" :disabled="!canToggle"
                :aria-label="text.intensity" @input="editSlider('intensity', $event)" @change="commitSlider('intensity')" @pointercancel="cancelEdit">
            </label>
            <p class="hint">{{ text.applyHint }}</p>
            <details class="advanced">
              <summary>{{ text.advanced }}</summary>
              <template v-if="pipeline.nr_live_controls_version >= 3">
                <div class="motion-label">{{ text.style }}</div>
                <div class="motion-options style-options" role="group" :aria-label="text.style">
                  <button v-for="style in [0, 1, 2, 3, 4]" :key="style" :disabled="!canToggle"
                    :aria-pressed="pipeline.nr_requested_style === style" :class="{ selected: pipeline.nr_requested_style === style }"
                    @click="setOption({ style })">{{ style }}</button>
                </div>
                <label class="parameter-row skin-control"><span>{{ text.skinStructure }}</span><output>{{ draftSkin }}%</output>
                  <input type="range" min="0" max="100" step="5" :value="draftSkin" :disabled="!canToggle"
                    :aria-label="text.skinStructure" @input="editSlider('skin', $event)" @change="commitSlider('skin')" @pointercancel="cancelEdit">
                </label>
                <label class="advanced-row"><span>{{ text.autoMask }}</span>
                  <input type="checkbox" :checked="pipeline.nr_requested_auto_mask" :disabled="!canToggle"
                    @change="setOption({ autoMask: $event.target.checked })">
                </label>
              </template>
              <label class="advanced-row"><span>{{ text.uiCorrection }}</span>
                <input type="checkbox" :checked="pipeline.nr_requested_ui_correction" :disabled="!canToggle"
                  @change="setOption({ uiCorrection: $event.target.checked })">
              </label>
              <div class="motion-label">{{ text.motionQuality }}</div>
              <div class="motion-options" role="group" :aria-label="text.motionQuality">
                <button v-for="(label, quality) in text.motionOptions" :key="quality" :disabled="!canToggle"
                  :aria-pressed="pipeline.nr_requested_motion_quality === quality"
                  :class="{ selected: pipeline.nr_requested_motion_quality === quality }"
                  @click="setOption({ motionQuality: quality })">{{ label }}</button>
              </div>
            </details>
            <p v-if="pipeline.nr_settings_failure_reason" class="notice">{{ text.settingsFailed }}</p>
          </div>
          <p v-else-if="pipeline" class="hint">{{ text.updateHost }}</p>
          <div class="signal-row"><span>{{ online && pipeline ? nrOutputLabel(pipeline) : '—' }}</span><span v-if="online && pipeline" class="badge">{{ text.preserved }}</span></div>
          <p v-if="online && pipeline?.dv_profile && pipeline.dv_state !== 'active'" class="notice">{{ pipeline.dv_state === 'waiting' ? text.dvWaiting : text.dvFallback }}</p>
          <p v-else-if="online && pipeline && pipeline.hdr_mode !== 'sdr' && !pipeline?.dv_profile" class="hint">{{ text.transfer }} · {{ pipeline?.hdr_mode.toUpperCase() }}</p>
          <p class="hint">{{ text.onlySession }}</p>
          <label class="opacity-row"><span>{{ text.opacity }}</span><output>{{ opacity }}%</output><input v-model.number="opacity" @change="saveOpacity" type="range" min="35" max="95" :aria-label="text.opacity"></label>
          <footer><kbd v-if="shortcut">{{ shortcut }}</kbd><span>{{ shortcut ? text.shortcut : text.shortcutUnavailable }}</span></footer>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ArrowDown, Close } from '@element-plus/icons-vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { currentMonitor, getCurrentWindow, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window'
import { useI18n } from '../../desktop/i18n/index.js'
import { displayShortcut, enhancementControlsText, enhancementError } from '../../composables/enhancementControls.js'
import { nrOverlayText } from '../../composables/nrOverlayMessages.js'
import { nrOverlayState, overlayOpacity, nrProcessingSize, nrPipelines, nrOutputLabel } from '../../composables/nrOverlayState.js'

import { overlayTouchDrag } from '../../composables/overlayTouchDrag.js'

const touchDrag = overlayTouchDrag(
  async () => {
    const win = getCurrentWindow()
    const [position, scale] = await Promise.all([win.outerPosition(), win.scaleFactor()])
    return { position, scale }
  },
  position => getCurrentWindow().setPosition(new PhysicalPosition(position.x, position.y))
)
defineEmits(['close'])
const { locale } = useI18n()
const text = computed(() => nrOverlayText(locale.value))
const expanded = ref(false)
const opacity = ref(62)
async function saveOpacity() {
  try { applyPreferences(await invoke('nr_overlay_save_settings', { patch: { opacity: Number(opacity.value) } })) }
  catch (error) { actionError.value = enhancementError(enhancementControlsText(locale.value), error) }
}
function applyPreferences(status) {
  if (!status?.settings || disposed) return
  opacity.value = overlayOpacity(status.settings.opacity)
  shortcut.value = status.nrRegistered ? displayShortcut(status.settings.nrShortcut) : ''
  if (status.target !== null && status.target !== undefined && status.target !== selectedId.value) { selectedId.value = status.target; selectionExpired.value = false }
}
const surface = ref(null)
const pipelines = ref([])
const selectedId = ref(null)
const selectionExpired = ref(false)
const online = ref(false)
const busy = ref(false)
const errorCode = ref('')
const actionError = ref('')
const error = computed(() => actionError.value || text.value[errorCode.value] || '')
const shortcut = ref('')
watch(selectedId, id => { if (id !== null && !disposed) void invoke('nr_overlay_select_session', { id }).catch(error => { actionError.value = enhancementError(enhancementControlsText(locale.value), error) }) })
const pipeline = computed(() => pipelines.value.find(item => item.id === selectedId.value))
const state = computed(() => nrOverlayState(pipeline.value, online.value))
const compactLabel = computed(() => state.value === 'active' ? `NR · ${pipeline.value?.nr_scale_percent ?? 100}%` : text.value.states[state.value])
const settled = ref(false)
let settleTimer
watch(state, (next, previous) => {
  clearTimeout(settleTimer)
  settled.value = ['active', 'disabled'].includes(next) && ['warming_up', 'stopping', 'scaling'].includes(previous)
  if (settled.value) settleTimer = setTimeout(() => { settled.value = false }, 1200)
})
const effectActive = computed(() => online.value && pipeline.value?.nr_state === 'active')
const canToggle = computed(() => online.value && pipeline.value?.nr_toggle_supported && !busy.value && !['warming_up', 'stopping', 'scaling'].includes(state.value))
const supportsControls = computed(() => pipeline.value?.nr_live_controls_version >= 2)
const draftScale = ref(100), draftIntensity = ref(100), draftSkin = ref(0)
const editing = ref('')
let editingSession = null
function syncDrafts() {
  if (editing.value !== 'scale') draftScale.value = pipeline.value?.nr_requested_scale_percent ?? 100
  if (editing.value !== 'skin') draftSkin.value = Math.round((pipeline.value?.nr_requested_skin_structure_strength ?? 0) * 100)
  if (editing.value !== 'intensity') draftIntensity.value = Math.round((pipeline.value?.nr_requested_intensity ?? 1) * 100)
}
watch(() => [selectedId.value, pipeline.value?.nr_requested_scale_percent, pipeline.value?.nr_requested_intensity, pipeline.value?.nr_requested_skin_structure_strength], syncDrafts)
watch(selectedId, () => { editing.value = ''; editingSession = null; syncDrafts() }, { flush: 'sync' })
watch(online, value => { if (!value) cancelEdit() })
const processingSize = computed(() => nrProcessingSize(pipeline.value,
  editing.value === 'scale' || !pipeline.value?.nr_requested_enabled ? draftScale.value : pipeline.value.nr_scale_percent))
function editSlider(kind, event) {
  if (!canToggle.value) return
  editing.value = kind
  editingSession = selectedId.value
  if (kind === 'scale') draftScale.value = Number(event.target.value)
  else if (kind === 'skin') draftSkin.value = Number(event.target.value)
  else draftIntensity.value = Number(event.target.value)
}
function cancelEdit() { editing.value = ''; editingSession = null; syncDrafts() }
async function commitSlider(kind) {
  if (!canToggle.value || editingSession !== selectedId.value || editing.value !== kind) { cancelEdit(); return }
  const patch = kind === 'scale' ? { scalePercent: draftScale.value } : kind === 'skin' ? { skinStructureStrength: draftSkin.value / 100 } : { intensity: draftIntensity.value / 100 }
  await setOption(patch)
  cancelEdit()
}
async function setOption(patch) {
  if (!canToggle.value || !supportsControls.value) return
  await sendRequest(pipeline.value.nr_requested_enabled, patch)
}
let disposed = false, refreshing = false, timer, observer, unlisten = []
async function refresh() {
  if (refreshing || disposed) return
  refreshing = true
  try {
    const result = await invoke('nr_live_status')
    if (disposed) return
    const next = nrPipelines(result?.pipelines)
    if (selectedId.value !== null && !next.some(item => item.id === selectedId.value)) {
      selectedId.value = null
      selectionExpired.value = true
    }
    pipelines.value = next
    // Multiple streams require explicit selection; do not target another user silently.
    if (!selectionExpired.value && selectedId.value === null && next.length === 1) selectedId.value = next[0].id
    online.value = true
    if (errorCode.value === 'disconnected') errorCode.value = ''
  } catch {
    if (!disposed) { online.value = false; errorCode.value = 'disconnected' }
  } finally { refreshing = false }
}
async function poll() { await refresh(); if (!disposed) timer = setTimeout(poll, 500) }
async function toggle() {
  if (!canToggle.value) return
  await sendRequest(!pipeline.value.nr_requested_enabled)
}
async function sendRequest(enabled, patch = {}) {
  const id = pipeline.value.id
  busy.value = true
  errorCode.value = ''; actionError.value = ''
  try {
    await invoke('nr_live_set_enabled', { id, enabled, ...patch })
    await refresh()
  } catch (reason) {
    if (!disposed) errorCode.value = String(reason).includes('nr_session_ended') ? 'ended' : 'failed'
  } finally { busy.value = false }
}
let resizing = false, resizeAgain = false
async function fitWindow() {
  if (!surface.value || disposed || touchDrag.active) return
  if (resizing) { resizeAgain = true; return }
  resizing = true
  try {
    const win = getCurrentWindow()
    const [position, old, scale, monitor] = await Promise.all([win.outerPosition(), win.outerSize(), win.scaleFactor(), currentMonitor()])
    if (disposed || touchDrag.active) return
    if (monitor) surface.value.style.setProperty('--panel-max-height', `${Math.max(120, Math.min(640, monitor.size.height / scale - 48))}px`)
    const width = Math.ceil(surface.value.getBoundingClientRect().width) + 8
    const height = Math.ceil(surface.value.getBoundingClientRect().height) + 8
    const x = position.x + old.width - Math.round(width * scale)
    const bounds = monitor ? { x: monitor.position.x, y: monitor.position.y, right: monitor.position.x + monitor.size.width, bottom: monitor.position.y + monitor.size.height } : null
    await win.setSize(new LogicalSize(width, height))
    await win.setPosition(new PhysicalPosition(Math.round(bounds ? Math.max(bounds.x, Math.min(x, bounds.right - width * scale)) : x), Math.round(bounds ? Math.max(bounds.y, Math.min(position.y, bounds.bottom - height * scale)) : position.y)))
  } catch (reason) { console.warn('NR overlay resize failed', reason) }
  finally { resizing = false; if (resizeAgain) { resizeAgain = false; void fitWindow() } }
}
onMounted(async () => {
  observer = new ResizeObserver(() => void fitWindow())
  observer.observe(surface.value)
  for (const [name, handler] of [
    ['nr-settings-changed', e => applyPreferences(e.payload)],
    ['nr-action-error', e => { actionError.value = enhancementError(enhancementControlsText(locale.value), e.payload) }],
  ]) {
    try { const stop = await listen(name, handler); if (disposed) stop(); else unlisten.push(stop) } catch {}
  }
  try { applyPreferences(await invoke('nr_overlay_settings')) } catch {}
  if (!disposed) void poll()
})
onUnmounted(() => { touchDrag.dispose(); disposed = true; clearTimeout(timer); clearTimeout(settleTimer); observer?.disconnect(); unlisten.forEach(stop => stop()) })

</script>

<style scoped>
.nr-overlay { --green: #76b900; width: 182px; margin: 4px; color: #f4f5ef; background: rgb(14 16 13 / var(--panel-alpha)); border: 1px solid #b2c29a66; border-radius: 0; font: 13px/1.5 'Segoe UI', 'Microsoft YaHei', sans-serif; box-shadow: 0 1px 3px #0005; box-sizing: border-box; overflow: hidden; transition: width .22s cubic-bezier(.2,.8,.2,1), background-color .22s ease, border-color .22s ease; }
.nr-overlay.expanded { width: 292px; }
button, select, input { font: inherit; }
button { cursor: pointer; color: inherit; }
button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid #fff; outline-offset: -3px; }
button:disabled { cursor: default; opacity: .5; }
.nr-head { height: 36px; display: flex; align-items: center; padding: 0 8px; gap: 5px; background: transparent; color: #d4dace; transition: height .22s ease, padding .22s ease; }
.drag-handle { display: grid; place-items: center; width: 22px; height: 30px; flex: 0 0 22px; cursor: grab; touch-action: none; user-select: none; }
.drag-handle:active { cursor: grabbing; }
.drag-handle svg { width: 16px; height: 24px; fill: currentColor; pointer-events: none; }
.pill { display: flex; align-items: center; gap: 7px; flex: 1; min-width: 0; background: none; border: 0; padding: 4px 0; text-align: left; white-space: nowrap; font-weight: 800; letter-spacing: .2px; }
.chevron { width: 14px; height: 14px; flex: 0 0 14px; margin-left: auto; transition: transform .18s ease; }
.chevron.rotated { transform: rotate(180deg); }
.expanded .nr-head { height: 36px; padding: 0 12px; border-bottom: 1px solid #ffffff18; }
.expanded .pill { font-size: 14px; font-weight: 650; letter-spacing: .2px; }
.nr-reveal { display: grid; grid-template-rows: 0fr; opacity: 0; transform: translateY(-4px); transition: grid-template-rows .22s cubic-bezier(.2,.8,.2,1), opacity .16s ease, transform .22s ease; }
.expanded .nr-reveal { grid-template-rows: 1fr; opacity: 1; transform: translateY(0); }
.nr-clip { min-height: 0; overflow: hidden; }
.expanded .nr-clip { max-height: var(--panel-max-height, 640px); overflow-y: auto; scrollbar-width: thin; scrollbar-color: #83916f transparent; }
.nr-body { padding: 0 16px 15px; }
.close { flex: 0 0 24px; margin-left: 5px; display: grid; place-items: center; width: 24px; height: 24px; padding: 0; box-sizing: border-box; line-height: 1; border: 1px solid transparent; background: transparent; }
.close svg { display: block; width: 14px; height: 14px; }
.close:hover { background: #f4f5ef; color: #111; }
.switch-row { display: flex; align-items: center; justify-content: space-between; margin: 17px 0 12px; font-size: 18px; font-weight: 800; }
.switch { width: 50px; height: 28px; padding: 3px; border-radius: 0; border: 2px solid #070906; background: #59604f; box-shadow: none; }
.switch > span { display: block; width: 18px; height: 18px; background: #f4f5ef; transition: transform .15s; }
.switch.on { background: var(--green); }
.switch.on > span { transform: translateX(22px); background: #101508; }
.live-state { display: flex; align-items: center; gap: 8px; margin-bottom: 17px; font-size: 12px; }
.dot { display: inline-block; flex: 0 0 8px; width: 8px; height: 8px; border-radius: 0; background: #a1aa93; }
.dot.active { background: var(--green); }
.expanded .nr-head .dot { width: 6px; height: 6px; flex-basis: 6px; }
.dot.degraded, .dot.blocked { background: #f3c74c; }
.dot.warming_up, .dot.stopping, .dot.scaling { background: transparent; border: 2px solid #768366; border-top-color: currentColor; animation: spin 1s linear infinite; }
.scale-control { border-top: 1px solid #ffffff38; padding-top: 13px; margin-bottom: 14px; }
.parameter-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; color: #e3e8dc; font-size: 12px; font-weight: 700; }
.parameter-row output { font: 700 12px Consolas, monospace; color: #d1d8c7; }
.parameter-row input { width: 100%; margin: 5px 0; accent-color: var(--green); cursor: pointer; }
.parameter-row input:disabled { cursor: default; opacity: .5; }
.advanced { border-top: 1px solid #ffffff20; padding-top: 10px; color: #b9c1b0; font-size: 12px; }
.advanced summary { cursor: pointer; }
.advanced-row { display: flex; align-items: center; justify-content: space-between; margin-top: 12px; }
.advanced-row input { appearance: none; width: 14px; height: 14px; border: 1px solid #83916f; background: #0004; cursor: pointer; }
.advanced-row input:checked { background: var(--green); box-shadow: inset 0 0 0 2px #101508; }
.motion-label { margin: 12px 0 8px; }
.motion-options { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; }
.style-options { grid-template-columns: repeat(5, 1fr); }
.skin-control { margin-top: 14px; }
.motion-options button { padding: 6px 0; border: 1px solid #8a927b; border-radius: 0; background: #0004; color: #d1d8c7; font-size: 11px; font-weight: 700; }
.motion-options button.selected { background: var(--green); color: #101508; border-color: #a8e03e; }
.motion-options button:hover:not(:disabled) { border-color: #fff; }
.scale-size { margin-top: 11px; color: #e0e7d7; font: 12px/1.5 Consolas, 'Microsoft YaHei', monospace; }
.scale-control .hint { margin: 7px 0 14px; }
.signal-row { border-top: 1px solid #ffffff38; padding-top: 13px; display: flex; justify-content: space-between; align-items: center; font-weight: 700; }
.badge { color: #a5dc43; font-size: 10px; border: 1px solid var(--green); padding: 2px 6px; border-radius: 0; }
.hint { color: #b7c0ab; font-size: 11px; margin: 9px 0 16px; }
.opacity-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; color: #d0d8c5; font-size: 12px; font-weight: 600; }
.parameter-row input[type="range"], .opacity-row input { appearance: none; width: 100%; margin: 3px 0; background: #4e5845; border: 1px solid #83916f; height: 6px; border-radius: 0; }
.parameter-row input[type="range"]::-webkit-slider-thumb, .opacity-row input::-webkit-slider-thumb { appearance: none; width: 12px; height: 18px; background: var(--green); border: 2px solid #0a1003; cursor: ew-resize; }
footer { display: flex; gap: 9px; align-items: center; border-top: 1px solid #ffffff38; margin-top: 17px; padding-top: 12px; color: #b7c0ab; font-size: 11px; }
kbd { border: 1px solid #d2dbc6; background: #d2dbc6; color: #101508; padding: 2px 5px; border-radius: 0; font: 700 11px/1.5 Consolas, monospace; }
.notice { font-size: 12px; color: #f3c74c; margin: 8px 0 12px; overflow-wrap: anywhere; }
.notice button { border: 0; background: none; text-decoration: underline; }
select { width: 100%; margin-top: 10px; background: #151b10; color: #f4f5ef; border: 1px solid #83916f; border-radius: 0; }
.nr-overlay:not(.expanded) { border-color: #ffffff1c; box-shadow: none; }
.nr-overlay:not(.expanded) .nr-head { height: 26px; background: transparent; color: #d4dace; padding: 0 7px; }
.nr-overlay:not(.expanded) .pill { font-size: 11px; font-weight: 600; letter-spacing: .15px; }
.nr-overlay:not(.expanded) .dot { width: 6px; height: 6px; flex-basis: 6px; }
.nr-overlay:not(.expanded) .drag-handle { width: 18px; height: 24px; flex-basis: 18px; }
.nr-overlay:not(.expanded) .drag-handle svg { width: 12px; height: 18px; }
.nr-overlay:not(.expanded) .drag-handle, .nr-overlay:not(.expanded) .chevron { opacity: 0; }
.nr-overlay:not(.expanded):hover .drag-handle, .nr-overlay:not(.expanded):hover .chevron,
.nr-overlay:not(.expanded):focus-within .drag-handle, .nr-overlay:not(.expanded):focus-within .chevron { opacity: .75; }
.nr-overlay:not(.expanded).settled { border-color: #76b90088; }
@media (pointer: coarse) { .drag-handle, .nr-overlay:not(.expanded) .drag-handle { width: 36px; height: 40px; flex-basis: 36px; } }
@media (hover: none) { .nr-overlay:not(.expanded) .drag-handle, .nr-overlay:not(.expanded) .chevron { opacity: .65; } }
@keyframes spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .dot { animation: none !important; } .nr-overlay, .nr-head, .nr-reveal, .switch > span, .chevron { transition: none; } }
</style>
