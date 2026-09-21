<template>
  <section ref="surface" class="nr-overlay" :class="{ expanded, settled }" :style="{ '--panel-alpha': (expanded ? opacity : Math.max(35, opacity - 12)) / 100 }">
    <header class="nr-head">
      <span class="drag-handle" data-tauri-drag-region :title="text.drag"><svg viewBox="0 0 16 24" aria-hidden="true"><circle v-for="(point, i) in [[5,6],[11,6],[5,12],[11,12],[5,18],[11,18]]" :key="i" :cx="point[0]" :cy="point[1]" r="1.5" /></svg></span>
      <button class="pill" :aria-expanded="expanded" :aria-label="text.states[state] + (state === 'active' ? ' · ' + (pipeline?.nr_scale_percent ?? 100) + '%' : '')" @click="expanded = !expanded">
        <span class="dot" :class="state" />
        <span>{{ expanded ? 'DLSS NR' : compactLabel }}</span>
        <ArrowDown class="chevron" :class="{ rotated: expanded }" aria-hidden="true" />
      </button>
    </header>
    <div v-if="expanded" class="nr-body">
      <div class="session-row">
        <span>{{ text.session }}</span>
        <button class="close" :aria-label="text.close" @click="$emit('close')"><Close aria-hidden="true" /></button>
      </div>
      <select v-if="pipelines.length > 1 || (selectionExpired && pipelines.length)" v-model="selectedId" :aria-label="text.session" :disabled="busy">
        <option :value="null" disabled>{{ text.choose }}</option>
        <option v-for="item in pipelines" :key="item.id" :value="item.id">{{ text.session }} #{{ item.id }} · {{ item.hdr_mode.toUpperCase() }}</option>
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
      <div v-if="pipeline?.nr_requested_scale_percent !== undefined" class="scale-control">
        <div class="scale-label">{{ text.scale }}</div>
        <div class="scale-options" role="group" :aria-label="text.scale">
          <button v-for="percent in [100, 75, 67, 50]" :key="percent" :disabled="!canToggle"
            :aria-pressed="selectedScale === percent" :class="{ selected: selectedScale === percent }"
            @click="setScale(percent)">{{ percent }}%</button>
        </div>
        <div v-if="online && processingSize" class="scale-size">{{ pipeline.nr_requested_enabled ? text.processing : text.targetSize }} · {{ processingSize }}</div>
        <p class="hint">{{ text.scaleHint }}</p>
        <p v-if="pipeline.nr_scale_failure_reason" class="notice">{{ text.scaleFailed }}</p>
      </div>
      <div class="signal-row"><span>{{ online && pipeline ? (pipeline.hdr_mode === 'sdr' ? 'SDR' : 'HDR · ' + pipeline.hdr_mode.toUpperCase()) : '—' }}</span><span v-if="online && pipeline" class="badge">{{ text.preserved }}</span></div>
      <p class="hint">{{ text.onlySession }}</p>
      <label class="opacity-row"><span>{{ text.opacity }}</span><output>{{ opacity }}%</output><input v-model.number="opacity" type="range" min="35" max="95" :aria-label="text.opacity"></label>
      <footer><kbd v-if="shortcut">Ctrl Alt N</kbd><span>{{ shortcut ? text.shortcut : text.shortcutUnavailable }}</span></footer>
    </div>
  </section>
</template>

<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { ArrowDown, Close } from '@element-plus/icons-vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { currentMonitor, getCurrentWindow, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window'
import { useI18n } from '../../desktop/i18n/index.js'
import { nrOverlayState, overlayOpacity, nrProcessingSize } from '../../composables/nrOverlayState.js'

defineEmits(['close'])
const { locale } = useI18n()
const text = computed(() => locale.value.startsWith('zh') ? {
  states: { disabled: 'NR 已关闭', active: 'NR 已开启', warming_up: 'NR 启动中', stopping: 'NR 关闭中', scaling: '切换处理比例', degraded: 'NR 已降级', idle: '等待串流', blocked: 'NR 不可用', unavailable: '主机未连接' },
  scale: 'NR 处理比例', processing: '处理尺寸', targetSize: '预设尺寸', scaleHint: '降低比例可减少开销；原画尺寸保持不变', scaleFailed: '该比例未能生效，已请求恢复上一档', choose: '请选择会话', drag: '拖动浮层', session: '当前串流', close: '关闭浮层', enhancement: '画面增强', preserved: '保持输出', onlySession: '仅影响当前会话，不修改应用默认设置', opacity: '背景不透明度', shortcut: '切换画面增强', shortcutUnavailable: '快捷键不可用，可点击开关', hdrBusy: 'RTX HDR 正在占用增强位置', unsupported: '当前主机或捕获路径不支持实时切换', fallback: '增强未生效，正在使用原画；关闭后可重新尝试', wait: '等待主机处理下一帧；首次开启需要初始化', retry: '重试', disconnected: '无法读取主机状态，请确认 Sunshine 正在运行', ended: '该串流已结束', failed: '切换失败，请重试',
} : {
  states: { disabled: 'NR off', active: 'NR on', warming_up: 'NR starting', stopping: 'NR stopping', scaling: 'Changing NR scale', degraded: 'NR bypassed', idle: 'Waiting for stream', blocked: 'NR unavailable', unavailable: 'Host disconnected' },
  scale: 'NR processing scale', processing: 'Processing size', targetSize: 'Planned size', scaleHint: 'Lower scales reduce cost. Original dimensions stay unchanged.', scaleFailed: 'Scale failed; restoration of the previous scale was requested.', choose: 'Select a stream', drag: 'Drag overlay', session: 'Current stream', close: 'Close overlay', enhancement: 'Enhancement', preserved: 'Output preserved', onlySession: 'This session only. App defaults stay unchanged.', opacity: 'Background opacity', shortcut: 'Toggle enhancement', shortcutUnavailable: 'Shortcut unavailable; use the switch', hdrBusy: 'RTX HDR is using the enhancement slot', unsupported: 'This host or capture path does not support live switching', fallback: 'Using the original image. Switch off and on to retry.', wait: 'Waiting for the next frame. First use needs initialization.', retry: 'Retry', disconnected: 'Cannot read host status. Check that Sunshine is running.', ended: 'This stream has ended', failed: 'Could not switch. Try again.',
})
const expanded = ref(false)
const opacity = ref(62)
try { opacity.value = overlayOpacity(localStorage.getItem('nr-overlay-opacity')) } catch {}
watch(opacity, value => { try { localStorage.setItem('nr-overlay-opacity', String(overlayOpacity(value))) } catch {} })
const surface = ref(null)
const pipelines = ref([])
const selectedId = ref(null)
const selectionExpired = ref(false)
const online = ref(false)
const busy = ref(false)
const error = ref('')
const shortcut = ref(false)
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
const selectedScale = computed(() => pipeline.value?.nr_requested_scale_percent ?? 100)
const processingSize = computed(() => nrProcessingSize(pipeline.value,
  pipeline.value?.nr_requested_enabled ? pipeline.value.nr_scale_percent : selectedScale.value))
async function setScale(percent) {
  if (!canToggle.value || percent === selectedScale.value) return
  await sendRequest(pipeline.value.nr_requested_enabled, percent)
}
let disposed = false, refreshing = false, timer, observer, unlisten
async function refresh() {
  if (refreshing || disposed) return
  refreshing = true
  try {
    const result = await invoke('nr_live_status')
    if (disposed) return
    const next = Array.isArray(result?.pipelines) ? result.pipelines : []
    if (selectedId.value !== null && !next.some(item => item.id === selectedId.value)) {
      selectedId.value = null
      selectionExpired.value = true
    }
    pipelines.value = next
    // Multiple streams require explicit selection; do not target another user silently.
    if (!selectionExpired.value && selectedId.value === null && next.length === 1) selectedId.value = next[0].id
    online.value = true
    if (error.value === text.value.disconnected) error.value = ''
  } catch {
    if (!disposed) { online.value = false; error.value = text.value.disconnected }
  } finally { refreshing = false }
}
async function poll() { await refresh(); if (!disposed) timer = setTimeout(poll, 500) }
async function toggle() {
  if (!canToggle.value) return
  await sendRequest(!pipeline.value.nr_requested_enabled)
}
async function sendRequest(enabled, scalePercent) {
  const id = pipeline.value.id
  busy.value = true
  error.value = ''
  try {
    await invoke('nr_live_set_enabled', { id, enabled, ...(scalePercent === undefined ? {} : { scalePercent }) })
    await refresh()
  } catch (reason) {
    if (!disposed) error.value = String(reason).includes('nr_session_ended') ? text.value.ended : text.value.failed
  } finally { busy.value = false }
}
let resizing = false, resizeAgain = false
async function fitWindow() {
  if (!surface.value || disposed) return
  if (resizing) { resizeAgain = true; return }
  resizing = true
  try {
    const win = getCurrentWindow()
    const width = expanded.value ? 300 : 190
    const height = Math.ceil(surface.value.getBoundingClientRect().height) + 8
    const [position, old, scale, monitor] = await Promise.all([win.outerPosition(), win.outerSize(), win.scaleFactor(), currentMonitor()])
    const x = position.x + old.width - Math.round(width * scale)
    const bounds = monitor ? { x: monitor.position.x, y: monitor.position.y, right: monitor.position.x + monitor.size.width, bottom: monitor.position.y + monitor.size.height } : null
    await win.setSize(new LogicalSize(width, height))
    await win.setPosition(new PhysicalPosition(Math.round(bounds ? Math.max(bounds.x, Math.min(x, bounds.right - width * scale)) : x), Math.round(bounds ? Math.max(bounds.y, Math.min(position.y, bounds.bottom - height * scale)) : position.y)))
  } catch (reason) { console.warn('NR overlay resize failed', reason) }
  finally { resizing = false; if (resizeAgain) { resizeAgain = false; void fitWindow() } }
}
watch(expanded, async () => { await nextTick(); void fitWindow() })
onMounted(async () => {
  void poll()
  observer = new ResizeObserver(() => void fitWindow())
  observer.observe(surface.value)
  try {
    const stop = await listen('nr-toggle', toggle)
    if (disposed) stop(); else unlisten = stop
    shortcut.value = await invoke('nr_overlay_shortcut_status')
  } catch {}
})
onUnmounted(() => { disposed = true; clearTimeout(timer); clearTimeout(settleTimer); observer?.disconnect(); unlisten?.() })
</script>

<style scoped>
.nr-overlay { --green: #76b900; width: 182px; margin: 4px; color: #f4f5ef; background: rgb(14 16 13 / var(--panel-alpha)); border: 1px solid #b2c29a66; border-radius: 0; font: 13px/1.5 'Segoe UI', 'Microsoft YaHei', sans-serif; box-shadow: 0 1px 3px #0005; box-sizing: border-box; overflow: hidden; }
.nr-overlay.expanded { width: 292px; }
button, select, input { font: inherit; }
button { cursor: pointer; color: inherit; }
button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid #fff; outline-offset: -3px; }
button:disabled { cursor: default; opacity: .5; }
.nr-head { height: 36px; display: flex; align-items: center; padding: 0 8px; gap: 5px; background: rgb(118 185 0 / .68); color: #080e02; }
.drag-handle { display: grid; place-items: center; width: 22px; height: 30px; flex: 0 0 22px; cursor: grab; touch-action: none; user-select: none; }
.drag-handle:active { cursor: grabbing; }
.drag-handle svg { width: 16px; height: 24px; fill: currentColor; pointer-events: none; }
.pill { display: flex; align-items: center; gap: 7px; flex: 1; min-width: 0; background: none; border: 0; padding: 4px 0; text-align: left; white-space: nowrap; font-weight: 800; letter-spacing: .2px; }
.chevron { width: 14px; height: 14px; flex: 0 0 14px; margin-left: auto; transition: transform .18s ease; }
.chevron.rotated { transform: rotate(180deg); }
.expanded .nr-head { height: 48px; padding: 0 14px; border-bottom: 1px solid #b2c29a55; }
.expanded .pill { font-size: 20px; font-weight: 900; letter-spacing: -.6px; }
.expanded .pill > .dot { display: none; }
.nr-body { padding: 0 16px 15px; }
.session-row { display: flex; justify-content: space-between; align-items: center; color: #b9c1b0; padding: 10px 0; border-bottom: 1px solid #ffffff38; font-size: 11px; font-weight: 600; }
.close { display: grid; place-items: center; width: 24px; height: 24px; border: 1px solid #ffffff50; background: #0003; }
.close svg { width: 14px; height: 14px; }
.close:hover { background: #f4f5ef; color: #111; }
.switch-row { display: flex; align-items: center; justify-content: space-between; margin: 17px 0 12px; font-size: 18px; font-weight: 800; }
.switch { width: 50px; height: 28px; padding: 3px; border-radius: 0; border: 2px solid #070906; background: #59604f; box-shadow: none; }
.switch > span { display: block; width: 18px; height: 18px; background: #f4f5ef; transition: transform .15s; }
.switch.on { background: var(--green); }
.switch.on > span { transform: translateX(22px); background: #101508; }
.live-state { display: flex; align-items: center; gap: 8px; margin-bottom: 17px; font-size: 12px; }
.dot { display: inline-block; flex: 0 0 8px; width: 8px; height: 8px; border-radius: 0; background: #a1aa93; }
.dot.active { background: var(--green); }
.expanded .nr-head .dot.active { background: #101508; }
.dot.degraded, .dot.blocked { background: #f3c74c; }
.dot.warming_up, .dot.stopping, .dot.scaling { background: transparent; border: 2px solid #768366; border-top-color: currentColor; animation: spin 1s linear infinite; }
.scale-control { border-top: 1px solid #ffffff38; padding-top: 13px; margin-bottom: 14px; }
.scale-label { color: #e3e8dc; font-size: 12px; font-weight: 700; margin-bottom: 9px; }
.scale-options { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; }
.scale-options button { padding: 7px 0; border: 1px solid #8a927b; border-radius: 0; background: #0004; color: #d1d8c7; font: 700 12px/1.5 Consolas, monospace; }
.scale-options button.selected { background: var(--green); color: #101508; border-color: #a8e03e; box-shadow: none; }
.scale-options button:hover:not(:disabled) { border-color: #fff; }
.scale-size { margin-top: 11px; color: #e0e7d7; font: 12px/1.5 Consolas, 'Microsoft YaHei', monospace; }
.scale-control .hint { margin: 5px 0 0; }
.signal-row { border-top: 1px solid #ffffff38; padding-top: 13px; display: flex; justify-content: space-between; align-items: center; font-weight: 700; }
.badge { color: #a5dc43; font-size: 10px; border: 1px solid var(--green); padding: 2px 6px; border-radius: 0; }
.hint { color: #b7c0ab; font-size: 11px; margin: 9px 0 16px; }
.opacity-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; color: #d0d8c5; font-size: 12px; font-weight: 600; }
.opacity-row input { appearance: none; width: 100%; margin: 3px 0; background: #4e5845; border: 1px solid #83916f; height: 6px; border-radius: 0; }
.opacity-row input::-webkit-slider-thumb { appearance: none; width: 12px; height: 18px; background: var(--green); border: 2px solid #0a1003; cursor: ew-resize; }
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
@media (hover: none) { .nr-overlay:not(.expanded) .drag-handle, .nr-overlay:not(.expanded) .chevron { opacity: .65; } }
@keyframes spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .dot { animation: none !important; } .switch > span, .chevron { transition: none; } }
</style>
