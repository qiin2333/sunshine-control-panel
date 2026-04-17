<template>
  <SectionPanel :title="t.rtssTool.monitoring">
    <template #icon><DataAnalysis /></template>
    <template #actions>
      <ToggleSwitch :modelValue="active" @update:modelValue="toggleMonitoring" />
    </template>

    <!-- 指标选择 -->
    <div class="metric-groups">
      <div v-for="group in metricGroups" :key="group.id" class="metric-group">
        <div class="metric-group-label">{{ group.label }}</div>
        <div class="metric-chips">
          <button
            v-for="m in group.metrics"
            :key="m.id"
            class="metric-chip"
            :class="{ active: config.metrics.includes(m.id) }"
            @click="toggleMetric(m.id)"
          >
            {{ m.label }}
          </button>
        </div>
      </div>
    </div>

    <!-- 样式配置 -->
    <div class="monitor-style">
      <div class="style-row">
        <label>{{ t.rtssTool.headerText }}</label>
        <input v-model="config.header_text" class="style-input" placeholder="☀  Foundation Sunshine" />
      </div>
      <div class="style-row">
        <label>{{ t.rtssTool.updateInterval }}</label>
        <select v-model.number="config.interval_ms" class="fmt-select wide">
          <option :value="500">0.5s</option>
          <option :value="1000">1s</option>
          <option :value="2000">2s</option>
          <option :value="5000">5s</option>
        </select>
      </div>
      <div class="style-row colors-row">
        <div class="color-field">
          <label>{{ t.rtssTool.titleColor }}</label>
          <input type="color" :value="'#' + config.title_color" @input="e => config.title_color = e.target.value.slice(1)" />
        </div>
        <div class="color-field">
          <label>{{ t.rtssTool.labelColor }}</label>
          <input type="color" :value="'#' + config.label_color" @input="e => config.label_color = e.target.value.slice(1)" />
        </div>
        <div class="color-field">
          <label>{{ t.rtssTool.valueColor }}</label>
          <input type="color" :value="'#' + config.value_color" @input="e => config.value_color = e.target.value.slice(1)" />
        </div>
      </div>
    </div>

    <!-- OSD 预览 -->
    <div v-if="active && snapshot.osd_text" class="osd-preview">
      <div class="osd-preview-label">{{ t.rtssTool.osdPreview }}</div>
      <pre class="osd-preview-text">{{ snapshot.osd_text }}</pre>
    </div>
  </SectionPanel>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted, watch } from 'vue'
import { DataAnalysis } from '@element-plus/icons-vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../../../desktop/i18n/index.js'
import SectionPanel from '../../components/SectionPanel.vue'
import ToggleSwitch from '../../components/ToggleSwitch.vue'

const { t, locale } = useI18n()
const emit = defineEmits(['message'])

const STORAGE_KEY = 'rtss-monitoring-config'

const DEFAULT_CONFIG = {
  interval_ms: 1000,
  metrics: ['session_state', 'stream_fps', 'stream_bitrate'],
  title_color: 'FFD700',
  label_color: 'AAAAAA',
  value_color: '00FF00',
  font_size: 0,
  header_text: '☀ Foundation Sunshine',
}

function loadPersistedConfig() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw)
      return { ...DEFAULT_CONFIG, ...parsed }
    }
  } catch {}
  return { ...DEFAULT_CONFIG }
}

const active = ref(false)
const snapshot = reactive({ active: false, osd_text: '', metrics: {} })
const config = reactive(loadPersistedConfig())

const availableMetrics = ref([])
let pollTimer = null

// 持久化 config 到 localStorage
watch(config, (v) => {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(v)) } catch {}
}, { deep: true })

const metricGroups = computed(() => {
  const isZh = locale.value === 'zh'
  const groups = [
    { id: 'session', label: isZh ? '串流会话' : 'Streaming', metrics: [] },
    { id: 'process', label: isZh ? '进程性能' : 'Process', metrics: [] },
  ]
  for (const m of availableMetrics.value) {
    const grp = groups.find(g => g.id === m.group)
    if (grp) grp.metrics.push({ id: m.id, label: isZh ? m.label_zh : m.label_en })
  }
  return groups.filter(g => g.metrics.length > 0)
})

// ─── 方法 ───
function toggleMetric(id) {
  const idx = config.metrics.indexOf(id)
  if (idx >= 0) config.metrics.splice(idx, 1)
  else config.metrics.push(id)
  if (active.value) startMonitoring()
}

async function toggleMonitoring(val) {
  if (val) await startMonitoring()
  else await stopMonitoring()
}

async function startMonitoring() {
  try {
    await invoke('rtss_start_monitoring', { config: { ...config } })
    active.value = true
    startPoll()
    emit('message', t.value.rtssTool.monitoringStarted, 'success')
  } catch (e) {
    emit('message', String(e), 'error')
  }
}

async function stopMonitoring() {
  try {
    await invoke('rtss_stop_monitoring')
    active.value = false
    stopPoll()
    snapshot.active = false
    snapshot.osd_text = ''
    emit('message', t.value.rtssTool.monitoringStopped, 'success')
  } catch (e) {
    emit('message', String(e), 'error')
  }
}

function startPoll() {
  stopPoll()
  pollTimer = setInterval(async () => {
    try {
      const snap = await invoke('rtss_get_monitoring_status')
      Object.assign(snapshot, snap)
      if (!snap.active) { active.value = false; stopPoll() }
    } catch (e) { /* ignore */ }
  }, 2000)
}

function stopPoll() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null }
}

async function loadState() {
  try {
    availableMetrics.value = await invoke('rtss_get_available_metrics')
  } catch (e) { /* ignore */ }
  try {
    const snap = await invoke('rtss_get_monitoring_status')
    if (snap.active) {
      active.value = true
      Object.assign(snapshot, snap)
      // 同步后端运行中的 config（若存在）
      if (snap.config) {
        Object.assign(config, snap.config)
      }
      startPoll()
    }
  } catch (e) { /* ignore */ }
}

onMounted(() => loadState())
onUnmounted(() => stopPoll())

defineExpose({ loadState })
</script>

<style lang="less" scoped>
.metric-groups {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.metric-group-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.55);
  margin-bottom: 3px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-weight: 600;
}

.metric-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.metric-chip {
  padding: 3px 10px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.75);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { background: rgba(255, 255, 255, 0.14); color: white; }
  &.active { background: rgba(147, 197, 253, 0.2); border-color: rgba(147, 197, 253, 0.5); color: #93c5fd; }
}

.monitor-style {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 4px;
}

.style-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;

  > label {
    color: rgba(255, 255, 255, 0.8);
    min-width: 65px;
    flex-shrink: 0;
  }
}

.style-input {
  flex: 1;
  padding: 4px 8px;
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  color: white;
  font-size: 12px;
  outline: none;

  &:focus { border-color: rgba(96, 165, 250, 0.6); box-shadow: 0 0 0 2px rgba(96, 165, 250, 0.15); }
  &::placeholder { color: rgba(255, 255, 255, 0.3); }
}

.colors-row {
  flex-wrap: wrap;
  gap: 10px;
}

.color-field {
  display: flex;
  align-items: center;
  gap: 5px;

  label {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.7);
    min-width: unset;
  }

  input[type="color"] {
    width: 24px;
    height: 24px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 6px;
    background: none;
    cursor: pointer;
    padding: 0;

    &::-webkit-color-swatch-wrapper { padding: 2px; }
    &::-webkit-color-swatch { border-radius: 4px; border: none; }
  }
}

.fmt-select {
  padding: 4px 8px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  cursor: pointer;
  outline: none;

  &.wide { min-width: 80px; }
  option { background: #1e1e2e; color: white; }
}

.osd-preview {
  background: rgba(0, 0, 0, 0.45);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 10px;
  padding: 8px 12px;
}

.osd-preview-label {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.5);
  margin-bottom: 4px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-weight: 600;
}

.osd-preview-text {
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 11px;
  color: #4ade80;
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
  line-height: 1.6;
}
</style>
