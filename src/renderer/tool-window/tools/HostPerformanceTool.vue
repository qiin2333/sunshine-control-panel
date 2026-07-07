<template>
  <div ref="containerRef" class="tool-container" :class="{ embedded, expanded: isExpanded }">
    <div class="monitor-head" v-bind="dragRegionAttrs">
      <div class="session-block" v-bind="dragRegionAttrs">
        <div class="status-line">
          <span class="status-dot" :class="statusClass"></span>
          <span class="status-text">{{ statusText }}</span>
        </div>
        <div class="session-title">{{ sessionTitle }}</div>
        <div class="session-meta">{{ sessionMeta }}</div>
      </div>

      <div class="head-actions">
        <button class="icon-btn" :disabled="refreshing" :title="t.performanceTool.refresh" @click="loadSnapshot(true)">
          <RefreshRight :class="{ spinning: refreshing }" />
        </button>
        <button v-if="!embedded" class="icon-btn" :title="expandTitle" @click="isExpanded = !isExpanded">
          <ArrowUp v-if="isExpanded" />
          <ArrowDown v-else />
        </button>
        <button v-if="!embedded" class="icon-btn close-btn" :title="t.performanceTool.close" @click="$emit('close')">
          <Close />
        </button>
      </div>
    </div>

    <div v-if="loading" class="state-panel compact-state">
      <Monitor class="state-icon" />
      <span>{{ t.performanceTool.loading }}</span>
    </div>

    <div v-else-if="error" class="state-panel compact-state error">
      <Warning class="state-icon" />
      <span>{{ t.performanceTool.loadFailed.replace('{error}', error) }}</span>
    </div>

    <div v-else-if="!currentSession" class="state-panel compact-state">
      <Monitor class="state-icon" />
      <span>{{ t.performanceTool.noStream }}</span>
    </div>

    <template v-else>
      <div class="hud-row">
        <div class="hud-metric primary">
          <span>{{ t.performanceTool.p95 }}</span>
          <strong>{{ formatMs(hostLatency.p95_ms) }}</strong>
        </div>
        <div class="hud-metric">
          <span>{{ t.performanceTool.recentFps }}</span>
          <strong>{{ formatNumber(hostLatency.recent_fps) }}</strong>
        </div>
        <div class="hud-metric">
          <span>{{ t.performanceTool.budgetUsage }}</span>
          <strong>{{ formatPercent(budgetUsage) }}</strong>
        </div>
      </div>

      <div class="chart-panel" :class="{ compact: !isExpanded && !embedded }">
        <div class="chart-header">
          <span>{{ t.performanceTool.hostLatency }}</span>
          <span>{{ t.performanceTool.samples.replace('{count}', hostLatency.samples ?? 0) }}</span>
        </div>
        <svg class="sparkline" viewBox="0 0 320 74" preserveAspectRatio="none">
          <line x1="0" y1="56" x2="320" y2="56" class="grid-line" />
          <polyline v-if="sparklinePoints" :points="sparklinePoints" class="sparkline-line" />
        </svg>
      </div>

      <div v-if="isExpanded || embedded" class="details">
        <div class="section-caption">{{ t.performanceTool.windowStats }}</div>
        <div class="detail-grid">
          <div v-for="item in detailStats" :key="item.label" class="detail-item">
            <span>{{ item.label }}</span>
            <strong>{{ item.value }}</strong>
          </div>
        </div>

        <template v-if="pipelineSegments.length">
          <div class="section-caption">{{ t.performanceTool.pipelineStats }}</div>
          <div class="pipeline-list">
            <div v-for="segment in pipelineSegments" :key="segment.key" class="pipeline-item">
              <span>{{ segment.label }}</span>
              <strong>{{ formatMs(segment.stats.p95_ms) }}</strong>
              <small>
                {{ t.performanceTool.avgShort }} {{ formatMs(segment.stats.avg_ms) }}
                / {{ t.performanceTool.lastShort }} {{ formatMs(segment.stats.last_ms) }}
              </small>
            </div>
          </div>
        </template>

        <div class="section-caption">{{ t.performanceTool.sessionStats }}</div>
        <div class="session-grid">
          <div>
            <span>{{ t.performanceTool.encoderCapture }}</span>
            <strong>{{ currentSession.encoder || 'auto' }} / {{ currentSession.capture || 'auto' }}</strong>
          </div>
          <div>
            <span>{{ t.performanceTool.bitrate }}</span>
            <strong>{{ formatBitrate(currentSession.bitrate_kbps) }}</strong>
          </div>
          <div>
            <span>{{ t.performanceTool.uptime }}</span>
            <strong>{{ formatUptime(currentSession.uptime_ms) }}</strong>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ArrowDown, ArrowUp, Close, Monitor, RefreshRight, Warning } from '@element-plus/icons-vue'
import { sunshine } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'
import { useAdaptiveWindowSize } from '../../composables/useAdaptiveWindowSize.js'

const { t } = useI18n()

const props = defineProps({
  embedded: {
    type: Boolean,
    default: false,
  },
})

defineEmits(['close'])

const POLL_INTERVAL_MS = 1000
const SPARKLINE_SAMPLE_LIMIT = 80
const SPARKLINE_SMOOTHING_ALPHA = 0.42
const SPARKLINE_CEILING_RISE_WEIGHT = 0.65
const SPARKLINE_CEILING_FALL_WEIGHT = 0.08

const snapshot = ref(null)
const loading = ref(true)
const refreshing = ref(false)
const error = ref('')
const isExpanded = ref(props.embedded)
const containerRef = ref(null)
const sparklineCeilingMs = ref(16)

let pollTimer = null
let snapshotInFlight = false
let queuedManualRefresh = false
let isDisposed = false

const activeSessions = computed(() => snapshot.value?.sessions?.filter((session) => session.active) ?? [])

const currentSession = computed(() => {
  const sessions = activeSessions.value
  if (sessions.length === 0) return null

  const latestSessionId = snapshot.value?.latest_session_id
  return sessions.find((session) => session.session_id === latestSessionId) ?? sessions[0]
})

const hostLatency = computed(() => currentSession.value?.host_latency ?? {})
const pipeline = computed(() => currentSession.value?.pipeline ?? {})

const frameBudgetMs = computed(() => {
  const fps = currentSession.value?.fps || 60
  return 1000 / fps
})

const budgetUsage = computed(() => {
  const p95 = hostLatency.value.p95_ms
  if (p95 == null) return null
  return (p95 / frameBudgetMs.value) * 100
})

const statusPressure = computed(() => {
  if (budgetUsage.value == null) return null

  const average = hostLatency.value.avg_ms
  const averageUsage = average == null ? 0 : (average / frameBudgetMs.value) * 100
  return Math.max(averageUsage, budgetUsage.value * 0.7)
})

const statusClass = computed(() => {
  if (error.value) return 'error'
  if (!currentSession.value) return 'idle'
  if (!hostLatency.value.samples) return 'waiting'
  if (statusPressure.value == null) return 'waiting'
  if (statusPressure.value <= 90) return 'good'
  if (statusPressure.value <= 170) return 'warning'
  return 'error'
})

const statusText = computed(() => {
  if (error.value) return t.value.performanceTool.status.error
  if (!currentSession.value) return t.value.performanceTool.status.idle
  if (!hostLatency.value.samples) return t.value.performanceTool.status.waiting
  if (statusClass.value === 'good') return t.value.performanceTool.status.good
  if (statusClass.value === 'warning') return t.value.performanceTool.status.warning
  return t.value.performanceTool.status.slow
})

const dragRegionAttrs = computed(() => props.embedded ? {} : { 'data-tauri-drag-region': '' })
const expandTitle = computed(() => isExpanded.value ? t.value.performanceTool.collapse : t.value.performanceTool.expand)
const sessionTitle = computed(() => currentSession.value?.client_name || t.value.performanceTool.noClient)

const sessionMeta = computed(() => {
  const session = currentSession.value
  if (!session) return t.value.performanceTool.noSessionMeta
  return `${session.width}x${session.height}@${session.fps} FPS`
})

const detailStats = computed(() => [
  { label: t.value.performanceTool.last, value: formatMs(hostLatency.value.last_ms) },
  { label: t.value.performanceTool.min, value: formatMs(hostLatency.value.min_ms) },
  { label: t.value.performanceTool.average, value: formatMs(hostLatency.value.avg_ms) },
  { label: t.value.performanceTool.max, value: formatMs(hostLatency.value.max_ms) },
  { label: t.value.performanceTool.frameBudget, value: formatMs(frameBudgetMs.value) },
  { label: t.value.performanceTool.sampleAge, value: formatAge(hostLatency.value.last_sample_age_ms) },
  { label: t.value.performanceTool.totalFrames, value: formatInteger(hostLatency.value.frames_total) },
  { label: t.value.performanceTool.totalSamples, value: formatInteger(hostLatency.value.total_samples) },
])

const pipelineSegments = computed(() => [
  { key: 'capture_to_convert', label: t.value.performanceTool.captureWait },
  { key: 'convert', label: t.value.performanceTool.convert },
  { key: 'encode_queue', label: t.value.performanceTool.encodeQueue },
  { key: 'encode', label: t.value.performanceTool.encode },
  { key: 'packet_to_broadcast', label: t.value.performanceTool.broadcastQueue },
  { key: 'total', label: t.value.performanceTool.totalPipeline },
].map((segment) => ({
  ...segment,
  stats: pipeline.value?.[segment.key] ?? {},
})).filter((segment) => segment.stats.samples))

const smoothSparklineValues = (values) => {
  if (values.length < 2) return values

  let smoothed = values[0]
  return values.map((value, index) => {
    if (index === 0) return value
    smoothed += (value - smoothed) * SPARKLINE_SMOOTHING_ALPHA
    return smoothed
  })
}

const updateSparklineCeiling = () => {
  const series = hostLatency.value.series_ms ?? []
  const values = series.slice(-SPARKLINE_SAMPLE_LIMIT)
  const target = Math.max(
    ...values,
    hostLatency.value.p95_ms ?? 0,
    frameBudgetMs.value,
    1,
  ) * 1.18
  const current = sparklineCeilingMs.value || target
  const weight = target > current ? SPARKLINE_CEILING_RISE_WEIGHT : SPARKLINE_CEILING_FALL_WEIGHT
  sparklineCeilingMs.value = current + (target - current) * weight
}

const sparklinePoints = computed(() => {
  const series = hostLatency.value.series_ms ?? []
  if (series.length < 2) return ''

  const values = smoothSparklineValues(series.slice(-SPARKLINE_SAMPLE_LIMIT))
  const max = Math.max(sparklineCeilingMs.value, 1)

  return values.map((value, index) => {
    const x = (index / (values.length - 1)) * 320
    const normalized = Math.min(Math.max(value / max, 0), 1)
    const y = 64 - normalized * 48
    return `${x.toFixed(1)},${y.toFixed(1)}`
  }).join(' ')
})

const formatMs = (value) => value == null ? '--' : `${Number(value).toFixed(2)} ms`
const formatNumber = (value) => value == null ? '--' : Number(value).toFixed(1)
const formatPercent = (value) => value == null ? '--' : `${Math.round(value)}%`
const formatInteger = (value) => value == null ? '--' : Number(value).toLocaleString()
const formatAge = (value) => value == null ? '--' : `${Math.round(value)} ms`
const formatBitrate = (kbps) => !kbps ? '--' : `${Math.round(kbps / 1000)} Mbps`

const formatUptime = (ms) => {
  if (!ms) return '--'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}m ${seconds}s`
}

const buildApiUrl = (baseUrl) => {
  const normalized = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl
  return `${normalized}/api/perf/current`
}

const fetchSnapshot = async () => {
  const proxyUrl = await sunshine.getProxyUrl()
  const response = await fetch(buildApiUrl(proxyUrl), {
    signal: AbortSignal.timeout(3000),
  })

  if (!response.ok) {
    throw new Error(`HTTP ${response.status}`)
  }

  return response.json()
}

const loadSnapshot = async (manual = false) => {
  if (snapshotInFlight) {
    if (manual) {
      queuedManualRefresh = true
      refreshing.value = true
    }
    return
  }

  snapshotInFlight = true
  if (manual) {
    refreshing.value = true
  }
  error.value = ''

  try {
    const nextSnapshot = await fetchSnapshot()
    if (!isDisposed) {
      snapshot.value = nextSnapshot
    }
  } catch (err) {
    if (!isDisposed) {
      error.value = err?.message || String(err)
    }
  } finally {
    snapshotInFlight = false
    if (!isDisposed) {
      loading.value = false
    }

    if (queuedManualRefresh && !isDisposed) {
      queuedManualRefresh = false
      loadSnapshot(true)
      return
    }

    queuedManualRefresh = false
    if (!isDisposed) {
      refreshing.value = false
    }
  }
}

const adaptiveWidth = computed(() => isExpanded.value ? 430 : 340)
const { scheduleSyncWindowSize } = useAdaptiveWindowSize(containerRef, {
  enabled: computed(() => !props.embedded),
  width: adaptiveWidth,
  minHeight: 220,
  animate: true,
})

onMounted(() => {
  loadSnapshot()
  pollTimer = window.setInterval(() => loadSnapshot(), POLL_INTERVAL_MS)
})

watch(isExpanded, scheduleSyncWindowSize)
watch(currentSession, scheduleSyncWindowSize)
watch(hostLatency, updateSparklineCeiling, { immediate: true })
watch(frameBudgetMs, updateSparklineCeiling)
watch(pipelineSegments, scheduleSyncWindowSize)

onUnmounted(() => {
  isDisposed = true
  if (pollTimer) {
    window.clearInterval(pollTimer)
    pollTimer = null
  }
})
</script>

<style lang="less" scoped>
.tool-container {
  --perf-bg: #f7fbff;
  --perf-surface: #ffffff;
  --perf-surface-soft: #f1f7fd;
  --perf-border: #dce9f5;
  --perf-border-muted: #e8f0f8;
  --perf-text: #26344d;
  --perf-text-muted: #78869b;
  --perf-accent: #55bfd0;
  --perf-accent-soft: #e4f7fa;
  --perf-accent-hover: #b8e5ec;
  --perf-accent-text: #218191;
  --perf-good: #68c7a2;
  --perf-warning: #f3bd63;
  --perf-danger: #f27f90;
  --perf-danger-soft: #fff0f3;
  --perf-danger-border: #f6c8d0;
  --perf-danger-text: #c24c61;
  --perf-grid: #deebf4;
  --perf-section: #8c98aa;

  width: 100vw;
  padding: 12px;
  box-sizing: border-box;
  color: var(--perf-text);
  border-radius: var(--fd-card-radius, 12px);
  background: var(--perf-bg);
  box-shadow: none;
  transition: width 0.18s ease, background 0.18s ease;

  &.expanded {
    width: 100vw;
    overflow: visible;
  }

  &.embedded {
    --perf-surface: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.22);
    --perf-border: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    --perf-border-muted: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.08);
    --perf-text: var(--fd-text-primary, #fff);
    --perf-text-muted: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.58);
    --perf-accent: var(--fd-accent, #00fff5);
    --perf-accent-soft: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    --perf-accent-hover: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.34);
    --perf-accent-text: var(--fd-accent, #00fff5);
    --perf-good: var(--fd-status-success, #34d399);
    --perf-warning: var(--fd-status-warning, #fbbf24);
    --perf-danger: var(--fd-status-danger, #f87171);
    --perf-danger-soft: rgba(var(--fd-status-danger-rgb, 248, 113, 113), 0.18);
    --perf-danger-border: rgba(var(--fd-status-danger-rgb, 248, 113, 113), 0.34);
    --perf-danger-text: var(--fd-status-danger, #f87171);
    --perf-grid: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
    --perf-section: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.46);

    width: 100%;
    max-height: none;
    padding: 0;
    border: none;
    background: transparent;
    box-shadow: none;
    backdrop-filter: none;
    overflow: visible;
  }
}

.monitor-head,
.status-line,
.head-actions,
.chart-header {
  display: flex;
  align-items: center;
}

.monitor-head {
  justify-content: space-between;
  gap: 10px;
}

.session-block {
  min-width: 0;
}

.tool-container:not(.embedded) {
  .monitor-head,
  .session-block {
    cursor: move;
    -webkit-app-region: drag;
  }
}

.tool-container.embedded {
  .monitor-head,
  .session-block {
    cursor: default;
    -webkit-app-region: no-drag;
  }
}

.status-line {
  gap: 6px;
  height: 18px;
  margin-bottom: 5px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;

  &.good {
    background: var(--perf-good);
  }

  &.warning,
  &.waiting {
    background: var(--perf-warning);
  }

  &.error {
    background: var(--perf-danger);
  }

  &.idle {
    background: var(--perf-accent);
  }
}

.status-text,
.session-meta,
.chart-header,
.detail-item span,
.session-grid span,
.hud-metric span {
  font-size: 11px;
  color: var(--perf-text-muted);
}

.session-title {
  font-size: 15px;
  font-weight: 700;
  line-height: 1.15;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-meta {
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.head-actions {
  gap: 6px;
  flex-shrink: 0;
  cursor: default;
  -webkit-app-region: no-drag;
}

.icon-btn {
  width: 28px;
  height: 28px;
  flex: 0 0 28px;
  border: 1px solid var(--perf-border-muted);
  border-radius: 8px;
  background: var(--perf-surface);
  color: var(--perf-text);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  -webkit-app-region: no-drag;
  transition: background 0.16s ease, border-color 0.16s ease, color 0.16s ease;

  svg {
    width: 15px;
    height: 15px;
  }

  &:hover:not(:disabled) {
    border-color: var(--perf-accent-hover);
    background: var(--perf-accent-soft);
    color: var(--perf-accent-text);
  }

  &:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
}

.close-btn:hover {
  border-color: var(--perf-danger-border);
  background: var(--perf-danger-soft);
  color: var(--perf-danger-text);
}

.spinning {
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.compact-state {
  min-height: 72px;
  margin-top: 10px;
}

.state-panel {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  text-align: center;
  border: 1px dashed var(--perf-border);
  border-radius: 10px;
  background: var(--perf-surface);
  color: var(--perf-text-muted);
  font-size: 12px;

  &.error {
    color: var(--perf-danger-text);
    border-color: var(--perf-danger-border);
  }
}

.state-icon {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

.hud-row {
  display: grid;
  grid-template-columns: 1.2fr 0.9fr 0.9fr;
  gap: 8px;
  margin-top: 12px;
}

.hud-metric {
  min-width: 0;
  padding: 9px 10px;
  border-radius: 10px;
  background: var(--perf-surface);
  border: 1px solid var(--perf-border);

  &.primary {
    border-color: var(--perf-accent-hover);
    background: var(--perf-accent-soft);
  }

  span,
  strong {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  strong {
    margin-top: 4px;
    font-size: 18px;
    line-height: 1.15;
  }
}

.chart-panel {
  margin-top: 10px;
  padding: 10px;
  border: 1px solid var(--perf-border);
  border-radius: 10px;
  background: var(--perf-surface);

  &.compact {
    padding-bottom: 6px;
  }
}

.chart-header {
  justify-content: space-between;
  gap: 10px;
}

.sparkline {
  width: 100%;
  height: 74px;
  display: block;
}

.compact .sparkline {
  height: 46px;
}

.grid-line {
  stroke: var(--perf-grid);
  stroke-width: 1;
}

.sparkline-line {
  fill: none;
  stroke: var(--perf-accent);
  stroke-width: 2.2;
  stroke-linejoin: round;
  stroke-linecap: round;
}

.details {
  margin-top: 12px;
}

.section-caption {
  margin: 12px 0 7px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--perf-section);
}

.detail-grid,
.session-grid,
.pipeline-list {
  display: grid;
  gap: 8px;
}

.detail-grid {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.session-grid {
  grid-template-columns: 1.4fr 0.8fr 0.8fr;
}

.pipeline-list {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.detail-item,
.session-grid > div,
.pipeline-item {
  min-width: 0;
  padding: 8px;
  border-radius: 8px;
  background: var(--perf-surface);
  border: 1px solid var(--perf-border-muted);

  span,
  strong,
  small {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  strong {
    margin-top: 3px;
    font-size: 13px;
  }

  small {
    margin-top: 2px;
    font-size: 10px;
    color: var(--perf-text-muted);
  }
}

.pipeline-item {
  span {
    font-size: 13px;
    font-weight: 700;
  }
}

@media (max-width: 420px) {
  .tool-container,
  .tool-container.expanded {
    width: 100vw;
  }

  .detail-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .pipeline-list {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
