<template>
  <section ref="surface" class="nr-overlay" :class="{ expanded }" :style="{ '--panel-alpha': (expanded ? opacity : Math.max(35, opacity - 12)) / 100 }">
    <header class="nr-head">
      <span class="drag-handle" data-tauri-drag-region :title="text.drag">⠿</span>
      <button class="pill" :aria-expanded="expanded" @click="expanded = !expanded">
        <span class="dot" :class="state" />
        <span>{{ expanded ? 'DLSS NR' : text.states[state] }}</span>
        <ArrowDown class="chevron" :class="{ rotated: expanded }" aria-hidden="true" />
      </button>
    </header>
    <div v-if="expanded" class="nr-body">
      <div class="session-row">
        <span>{{ text.session }}</span>
        <button class="close" :aria-label="text.close" @click="$emit('close')">×</button>
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
import { ArrowDown } from '@element-plus/icons-vue'
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
const opacity = ref(72)
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
onUnmounted(() => { disposed = true; clearTimeout(timer); observer?.disconnect(); unlisten?.() })
</script>

<style scoped>
.nr-overlay { width: 182px; margin: 4px; color: #eef3f1; background: rgb(19 24 26 / var(--panel-alpha)); border: 1px solid #ffffff24; border-radius: 15px; font: 13px/1.5 'Segoe UI', 'Microsoft YaHei', sans-serif; box-shadow: 0 2px 5px #0003; box-sizing: border-box; overflow: hidden; }
.nr-overlay.expanded { width: 292px; border-radius: 18px; }
button, select, input { font: inherit; }
button { cursor: pointer; color: inherit; }
button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid #79dcb0; outline-offset: 2px; }
button:disabled { cursor: default; opacity: .5; }
.nr-head { height: 38px; display: flex; align-items: center; padding: 0 9px; gap: 5px; }
.drag-handle { cursor: grab; color: #acb8b2; font-size: 21px; user-select: none; }
.pill { display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0; background: none; border: 0; padding: 4px 0; text-align: left; white-space: nowrap; }
.chevron { width: 14px; height: 14px; flex: 0 0 14px; margin-left: auto; color: #ced8d3; transition: transform .18s ease; }
.chevron.rotated { transform: rotate(180deg); }
.expanded .nr-head { height: 49px; padding: 0 18px; }
.expanded .pill { font-size: 17px; font-weight: 600; }
.expanded .pill > .dot { display: none; }
.nr-body { padding: 0 20px 15px; }
.session-row { display: flex; justify-content: space-between; align-items: center; color: #b1beb7; margin-top: -4px; padding-bottom: 12px; border-bottom: 1px solid #ffffff18; }
.close { border: 0; background: none; font-size: 22px; line-height: 20px; }
.switch-row { display: flex; align-items: center; justify-content: space-between; margin: 19px 0 12px; font-size: 17px; }
.switch { width: 46px; height: 26px; padding: 3px; border-radius: 20px; border: 1px solid #ffffff24; background: #68726c; }
.switch > span { display: block; width: 18px; height: 18px; border-radius: 50%; background: #f6faf7; transition: transform .15s; }
.switch.on { background: #72cca4; }
.switch.on > span { transform: translateX(18px); }
.live-state { display: flex; align-items: center; gap: 8px; margin-bottom: 17px; }
.dot { display: inline-block; flex: 0 0 9px; width: 9px; height: 9px; border-radius: 50%; background: #9ba49f; }
.dot.active { background: #7de3b4; box-shadow: 0 0 8px #7de3b433; }
.dot.degraded, .dot.blocked { background: #efce75; }
.dot.warming_up, .dot.stopping, .dot.scaling { background: transparent; border: 2px solid #9ba49f; border-top-color: #e6f0eb; animation: spin 1s linear infinite; }
.scale-control { border-top: 1px solid #ffffff18; padding-top: 13px; margin-bottom: 14px; }
.scale-label { color: #c3cec7; font-size: 12px; margin-bottom: 9px; }
.scale-options { display: grid; grid-template-columns: repeat(4, 1fr); padding: 3px; gap: 3px; border: 1px solid #ffffff18; border-radius: 9px; background: #0002; }
.scale-options button { padding: 5px 0; border: 0; border-radius: 6px; background: transparent; color: #b5c3bb; font-size: 12px; }
.scale-options button.selected { background: #7de3b425; color: #9debc5; box-shadow: inset 0 0 0 1px #7de3b44a; }
.scale-options button:hover:not(:disabled) { background: #ffffff12; }
.scale-size { margin-top: 9px; color: #d1dcd5; font-size: 11px; font-variant-numeric: tabular-nums; }
.scale-control .hint { margin: 5px 0 0; }
.signal-row { border-top: 1px solid #ffffff18; padding-top: 14px; display: flex; justify-content: space-between; align-items: center; }
.badge { color: #ccd5cf; font-size: 11px; border: 1px solid #ffffff20; padding: 2px 6px; border-radius: 6px; }
.hint { color: #b0bab4; font-size: 11px; margin: 9px 0 16px; }
.opacity-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 7px; color: #c3cec7; font-size: 12px; }
.opacity-row input { width: 100%; accent-color: #7de3b4; height: 16px; }
footer { display: flex; gap: 8px; align-items: center; border-top: 1px solid #ffffff18; margin-top: 14px; padding-top: 12px; color: #b4c0b8; font-size: 11px; }
kbd { border: 1px solid #ffffff30; padding: 2px 4px; border-radius: 4px; font: inherit; }
.notice { font-size: 12px; color: #efce75; margin: 8px 0 12px; overflow-wrap: anywhere; }
.notice button { border: 0; background: none; text-decoration: underline; }
select { width: 100%; margin-top: 10px; background: #202925; color: #eef3f1; border: 1px solid #ffffff30; border-radius: 5px; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .dot { animation: none !important; } .switch > span, .chevron { transition: none; } }
</style>
