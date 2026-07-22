<template>
  <div class="vdd-settings-wrapper">
    <div class="vdd-content">
      <div class="vdd-page-header">
        <div class="page-title-group">
          <span class="page-title-icon" aria-hidden="true">
            <el-icon><Monitor /></el-icon>
          </span>
          <div class="page-title-copy">
            <h2>{{ t.vddSettings.title }}</h2>
            <div v-if="!isDriverCheckPending && (vddReady || hasUnsavedChanges)" class="page-header-meta">
              <el-tag v-if="vddReady" size="small" round effect="plain">{{ currentEdidModeLabel }}</el-tag>
              <el-tag v-if="hasUnsavedChanges" size="small" round type="warning" effect="light">
                {{ t.vddSettings.unsavedBadge }}
              </el-tag>
            </div>
          </div>
        </div>

        <div
          v-if="!isDriverCheckPending && vddReady && vddStatus.state === 'degraded'"
          class="degraded-driver-notice"
          role="status"
          aria-live="polite"
        >
          <span class="degraded-driver-icon" aria-hidden="true">!</span>
          <div class="degraded-driver-copy">
            <strong>{{ vddStatusLabel }}</strong>
            <span>{{ vddStatus.status_text }}</span>
          </div>
          <el-button type="warning" plain :loading="isInstallingDriver" @click="installOrRepairDriver">
            <el-icon><Download /></el-icon>
            {{ t.vddSettings.installRepairDriver }}
          </el-button>
        </div>
      </div>

      <section
        v-if="isDriverCheckPending"
        class="section-card driver-detection-card"
        role="status"
        aria-live="polite"
        aria-busy="true"
        aria-labelledby="driver-detection-title"
      >
        <div class="detection-core" aria-hidden="true">
          <span class="detection-orbit"></span>
          <span class="detection-core-icon"><el-icon><Monitor /></el-icon></span>
        </div>

        <div class="detection-copy">
          <span class="detection-kicker">{{ t.vddSettings.driverDetectionKicker }}</span>
          <h3 id="driver-detection-title">{{ driverDetectionTitle }}</h3>
          <p>{{ driverDetectionDescription }}</p>
        </div>

        <div class="detection-checks">
          <span :class="{ 'is-complete': driverCheckPhase === 'syncing' }">
            <i aria-hidden="true"></i>{{ t.vddSettings.driverDetectionPresence }}
          </span>
          <span :class="{ 'is-complete': driverCheckPhase === 'syncing' }">
            <i aria-hidden="true"></i>{{ t.vddSettings.driverDetectionHealth }}
          </span>
          <span :class="{ 'is-complete': driverCheckPhase === 'syncing' }">
            <i aria-hidden="true"></i>{{ t.vddSettings.driverDetectionVersion }}
          </span>
        </div>

        <div class="detection-lock">
          <span aria-hidden="true">◇</span>
          {{ t.vddSettings.driverDetectionLocked }}
        </div>
      </section>

      <section v-else-if="!vddReady" class="section-card vdd-prerequisite-card" aria-labelledby="vdd-prerequisite-title">
        <div class="prerequisite-intro">
          <div class="prerequisite-icon" aria-hidden="true">
            <el-icon><Monitor /></el-icon>
          </div>
          <div class="prerequisite-copy">
            <h3 id="vdd-prerequisite-title">{{ t.vddSettings.driverPrerequisiteTitle }}</h3>
            <p>{{ t.vddSettings.driverPrerequisiteDesc }}</p>
          </div>
        </div>

        <div class="prerequisite-status" role="status" aria-live="polite">
          <span class="status-dot" aria-hidden="true"></span>
          <div class="status-copy">
            <strong>{{ vddStatusLabel }}</strong>
            <span>{{ vddStatus.status_text || t.vddSettings.driverStatusUnknown }}</span>
            <div v-if="vddStatus.installed_version || vddStatus.bundled_version" class="driver-version-meta">
              <span>{{ t.vddSettings.driverVersionInstalled }}: {{ vddStatus.installed_version || '—' }}</span>
              <span>{{ t.vddSettings.driverVersionBundled }}: {{ vddStatus.bundled_version || '—' }}</span>
            </div>
          </div>
        </div>

        <div class="prerequisite-actions">
          <el-button v-if="vddCanInstall" type="primary" :loading="isInstallingDriver" @click="installOrRepairDriver">
            <el-icon><Download /></el-icon>
            {{ t.vddSettings.installRepairDriver }}
          </el-button>
          <el-button :loading="isCheckingDriver" @click="recheckDriver">
            <el-icon><Refresh /></el-icon>
            {{ t.vddSettings.recheckDriver }}
          </el-button>
        </div>
      </section>

      <template v-if="!isDriverCheckPending && vddReady">
      <section class="hero-panel">
        <div class="vdd-header">
          <div class="header-copy">
            <p class="header-subtitle">{{ t.vddSettings.subtitle }}</p>
            <div class="header-meta-line">
              <span class="meta-label">{{ t.vddSettings.overviewPath }}</span>
              <span class="meta-value path-value">{{ configFilePath || t.vddSettings.configPathUnknown }}</span>
            </div>
            <div class="cert-badges" aria-label="VDD capability badges">
              <div v-for="badge in capabilityBadges" :key="badge.key" :class="['cert-badge', badge.tone]">
                <span class="cert-text">{{ badge.text }}</span>
                <span class="cert-sub">{{ badge.sub }}</span>
              </div>
            </div>
          </div>

          <el-affix class="hero-callout-affix" target=".vdd-content" :offset="20">
            <div class="hero-callout">
              <span class="hero-callout-label">{{ t.vddSettings.quickActions }}</span>
              <div class="hero-callout-actions">
                <el-button
                  v-for="action in heroActions"
                  :key="action.key"
                  size="large"
                  class="hero-action-button"
                  :plain="action.plain"
                  :type="action.type"
                  :loading="action.loading"
                  @click="action.onClick"
                >
                  <el-icon><component :is="action.icon" /></el-icon>
                  {{ action.label }}
                </el-button>
              </div>
            </div>
          </el-affix>
        </div>

        <div class="overview-grid">
          <div v-for="card in overviewCards" :key="card.key" class="overview-card">
            <span class="overview-label">{{ card.label }}</span>
            <strong class="overview-value">{{ card.value }}</strong>
            <span v-if="card.hint" class="overview-hint">{{ card.hint }}</span>
          </div>
        </div>
      </section>

      <el-form :model="settings" label-position="top" size="default" class="vdd-form">
        <div class="form-layout">
          <div class="form-main">
            <div class="section-card">
              <div class="section-header">
                <h3>{{ t.vddSettings.displaySection }}</h3>
                <p>{{ t.vddSettings.displaySectionHint }}</p>
              </div>

              <el-form-item :label="t.vddSettings.resolutionPresets">
                <div class="setting-content">
                  <el-tag
                    v-for="res in sortedResolutionOptions"
                    :key="res"
                    closable
                    @close="removeResolution(res)"
                    class="resolution-tag"
                    type="info"
                  >
                    {{ res }}
                  </el-tag>
                  <el-input
                    v-if="showResInput"
                    ref="resInputRef"
                    v-model="newResolution"
                    class="input-new-tag"
                    @keyup.enter="addResolution"
                    @blur="handleResInputConfirm"
                    size="small"
                    :placeholder="t.vddSettings.resPlaceholder"
                    style="width: 140px"
                  />
                  <el-button v-else size="small" @click="showResolutionInput" class="add-btn">
                    <el-icon><Plus /></el-icon>
                    {{ t.vddSettings.addResolution }}
                  </el-button>
                </div>
              </el-form-item>

              <el-form-item :label="t.vddSettings.suggestedResolutions">
                <div class="preset-buttons">
                  <el-button
                    v-for="resolution in suggestedResolutions"
                    :key="resolution"
                    size="small"
                    plain
                    :disabled="resolutionOptions.has(resolution)"
                    @click="quickAddResolution(resolution)"
                  >
                    {{ resolution }}
                  </el-button>
                </div>
              </el-form-item>

              <el-form-item :label="t.vddSettings.gpuBinding">
                <div class="setting-content full-width">
                  <el-select
                    v-model="gpuFriendlyName"
                    filterable
                    allow-create
                    default-first-option
                    style="width: 100%; max-width: 420px"
                    :placeholder="t.vddSettings.gpuPlaceholder"
                    @blur="saveGpuEdit"
                    @keyup.enter="saveGpuEdit"
                  >
                    <el-option v-for="gpu in gpuOptions" :key="gpu" :label="gpu" :value="gpu" />
                  </el-select>
                </div>
              </el-form-item>

              <el-form-item :label="t.vddSettings.monitorCount">
                <div class="field-stack">
                  <el-input-number v-model="settings.monitors.count" :min="1" :max="1" disabled />
                  <span class="form-tip">{{ t.vddSettings.monitorCountTip }}</span>
                </div>
              </el-form-item>

              <el-form-item :label="t.vddSettings.refreshRatePresets">
                <div class="setting-content">
                  <el-tag
                    v-for="rate in sortedRefreshRateOptions"
                    :key="rate"
                    closable
                    @close="removeRefreshRate(rate)"
                    class="rate-tag"
                    type="success"
                  >
                    {{ rate }}Hz
                  </el-tag>
                  <el-input
                    v-if="showRateInput"
                    ref="rateInputRef"
                    v-model="newRefreshRate"
                    class="input-new-tag"
                    @keyup.enter="addRefreshRate"
                    @blur="handleRateInputConfirm"
                    size="small"
                    :placeholder="t.vddSettings.ratePlaceholder"
                    style="width: 120px"
                  />
                  <el-button v-else size="small" @click="showRefreshRateInput" class="add-btn">
                    <el-icon><Plus /></el-icon>
                    {{ t.vddSettings.addRefreshRate }}
                  </el-button>
                </div>
              </el-form-item>

              <el-form-item :label="t.vddSettings.suggestedRefreshRates">
                <div class="preset-buttons">
                  <el-button
                    v-for="rate in suggestedRefreshRates"
                    :key="rate"
                    size="small"
                    plain
                    :disabled="refreshRateOptions.has(rate)"
                    @click="quickAddRefreshRate(rate)"
                  >
                    {{ rate }}Hz
                  </el-button>
                </div>
              </el-form-item>

              <el-form-item :label="t.vddSettings.vrr">
                <div class="field-stack">
                  <div class="field-inline-control">
                    <el-switch v-model="settings.edid.Vrr" />
                  </div>
                  <span class="form-tip">{{ t.vddSettings.vrrTip }}</span>
                </div>
              </el-form-item>
            </div>

            <div class="section-card">
              <div class="section-header">
                <h3>{{ t.vddSettings.qualitySection }}</h3>
                <p>{{ t.vddSettings.qualitySectionHint }}</p>
              </div>

              <div class="two-column-layout">
                <el-form-item :label="t.vddSettings.colorDepthProfile" class="span-two">
                  <div class="field-stack">
                    <el-radio-group v-model="colorDepthProfile" class="color-depth-group">
                      <el-radio-button :label="COLOR_DEPTH_DEFAULT">
                        {{ t.vddSettings.colorDepthDefault }}
                      </el-radio-button>
                      <el-radio-button :label="COLOR_DEPTH_SDR10">
                        {{ t.vddSettings.sdr10bit }}
                      </el-radio-button>
                      <el-radio-button :label="COLOR_DEPTH_HDR12">
                        {{ t.vddSettings.hdr12bit }}
                      </el-radio-button>
                      <el-radio-button
                        v-if="isColorDepthConflict"
                        :label="COLOR_DEPTH_CONFLICT"
                        disabled
                      >
                        {{ t.vddSettings.colorDepthConflict }}
                      </el-radio-button>
                    </el-radio-group>
                    <span class="form-tip">{{ t.vddSettings.colorDepthMutualExclusiveTip }}</span>
                    <el-alert
                      v-if="isColorDepthConflict"
                      type="warning"
                      show-icon
                      :closable="false"
                      class="inline-alert"
                    >
                      {{ t.vddSettings.colorDepthConflictTip }}
                    </el-alert>
                  </div>
                </el-form-item>

                <el-form-item :label="t.vddSettings.colorMode">
                  <div class="field-stack">
                    <el-select
                      v-model="settings.colour.ColourFormat"
                      :placeholder="t.vddSettings.selectColorMode"
                      style="width: 220px"
                    >
                      <el-option label="RGB" value="RGB" />
                      <el-option label="YCbCr444" value="YCbCr444" />
                      <el-option label="YCbCr422" value="YCbCr422" />
                      <el-option label="YCbCr420" value="YCbCr420" />
                    </el-select>
                  </div>
                </el-form-item>

                <el-form-item :label="t.vddSettings.hardwareCursor">
                  <div class="field-stack">
                    <div class="field-inline-control">
                      <el-switch v-model="settings.cursor.HardwareCursor" />
                    </div>
                    <span class="form-tip">{{ t.vddSettings.hardwareCursorTip }}</span>
                  </div>
                </el-form-item>

              </div>
            </div>
          </div>

          <div class="form-side">
            <div class="section-card utility-card">
              <div class="section-header">
                <h3>{{ t.vddSettings.driverTools }}</h3>
                <p>{{ t.vddSettings.driverToolsHint }}</p>
              </div>

              <div class="action-strip">
                <el-button @click="reloadDriver" :loading="isReloadingDriver">
                  <el-icon><Refresh /></el-icon>
                  {{ t.vddSettings.reloadDriver }}
                </el-button>
              </div>

              <div v-if="edidFileExists" class="danger-zone">
                <div class="danger-zone-header">
                  <el-tag type="danger" effect="light" round>{{ t.vddSettings.dangerZone }}</el-tag>
                  <span class="danger-zone-desc">{{ t.vddSettings.dangerZoneHint }}</span>
                </div>

                <el-button
                  plain
                  type="danger"
                  class="danger-action"
                  @click="removeEdidFile"
                  :loading="isDeletingEdid"
                >
                  <el-icon><Delete /></el-icon>
                  {{ t.vddSettings.deleteEdid }}
                </el-button>
              </div>
            </div>

            <div class="section-card trace-card">
              <div class="section-header">
                <h3>{{ t.vddSettings.vddTraceTitle }}</h3>
                <p>{{ t.vddSettings.vddTraceHint }}</p>
              </div>

              <div class="trace-status">
                <el-tag :type="traceStatus.running ? 'success' : 'info'" effect="light" round>
                  {{ traceStatus.running ? t.vddSettings.vddTraceRunning : t.vddSettings.vddTraceIdle }}
                </el-tag>
                <span class="trace-path" :title="traceDisplayPath">{{ traceDisplayPath }}</span>
              </div>

              <p class="form-tip trace-admin-tip">{{ t.vddSettings.vddTraceAdminHint }}</p>

              <div class="action-strip trace-actions">
                <el-button
                  type="primary"
                  :loading="isStartingTrace"
                  :disabled="traceStatus.running || isStoppingTrace"
                  @click="startTrace"
                >
                  <el-icon><VideoPlay /></el-icon>
                  {{ t.vddSettings.vddTraceStart }}
                </el-button>
                <el-button
                  :loading="isStoppingTrace"
                  :disabled="!traceStatus.running || isStartingTrace"
                  @click="stopTrace"
                >
                  <el-icon><VideoPause /></el-icon>
                  {{ t.vddSettings.vddTraceStop }}
                </el-button>
                <el-button
                  :loading="isOpeningTraceFolder"
                  :disabled="!traceStatus.directory"
                  @click="openTraceFolder"
                >
                  <el-icon><FolderOpened /></el-icon>
                  {{ t.vddSettings.vddTraceOpenFolder }}
                </el-button>
              </div>
            </div>

            <div class="section-card">
              <div class="section-header">
                <h3>{{ t.vddSettings.edidSection }}</h3>
                <p>{{ t.vddSettings.edidSectionHint }}</p>
              </div>

              <el-form-item :label="t.vddSettings.customEdid">
                <div class="field-stack">
                  <div class="field-inline-control">
                    <el-switch v-model="settings.edid.CustomEdid" @change="handleEdidToggle" />
                  </div>
                  <span class="form-tip">{{ t.vddSettings.customEdidTip }}</span>
                </div>
              </el-form-item>

              <el-alert
                v-if="hasCustomEdidIssue"
                type="warning"
                show-icon
                :closable="false"
                class="inline-alert"
              >
                {{ t.vddSettings.customEdidMissingInline }}
              </el-alert>

              <el-form-item :label="t.vddSettings.edidFile" v-if="settings.edid.CustomEdid">
                <div class="edid-file-manager">
                  <el-alert type="warning" :closable="false" show-icon class="edid-warning">
                    <template #title>
                      <span class="warning-text">{{ t.vddSettings.edidWarning }}</span>
                    </template>
                  </el-alert>

                  <div class="edid-status">
                    <el-tag :type="edidFileExists ? 'success' : 'info'" effect="dark">
                      {{ edidFileExists ? t.vddSettings.edidUploaded : t.vddSettings.edidNotUploaded }}
                    </el-tag>
                    <span class="edid-path" v-if="edidFilePath">{{ edidFilePath }}</span>
                  </div>

                  <div class="edid-actions">
                    <el-upload
                      :auto-upload="false"
                      :show-file-list="false"
                      :on-change="handleEdidFileChange"
                      accept=".bin"
                    >
                      <el-button size="small" type="primary">
                        <el-icon><Upload /></el-icon>
                        {{ t.vddSettings.edidSelectFile }}
                      </el-button>
                    </el-upload>

                    <el-button size="small" @click="downloadEdid" :disabled="!edidFileExists">
                      <el-icon><Download /></el-icon>
                      {{ t.vddSettings.edidDownload }}
                    </el-button>
                  </div>

                  <div class="edid-info" v-if="edidInfo">
                    <el-descriptions :column="2" size="small" border>
                      <el-descriptions-item :label="t.vddSettings.edidFileSize">
                        {{ edidInfo.size }} {{ t.vddSettings.edidBytes }}
                      </el-descriptions-item>
                      <el-descriptions-item :label="t.vddSettings.edidFormat">
                        {{ getEdidFormatLabel(edidInfo.size) }}
                      </el-descriptions-item>
                      <el-descriptions-item label="Checksum" :span="2">
                        <el-tag :type="edidInfo.checksumValid ? 'success' : 'danger'" size="small">
                          {{ edidInfo.checksumValid ? t.vddSettings.edidChecksumValid : t.vddSettings.edidChecksumInvalid }}
                        </el-tag>
                      </el-descriptions-item>
                    </el-descriptions>
                  </div>
                </div>
              </el-form-item>
            </div>
          </div>
        </div>
      </el-form>
      </template>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Monitor,
  Plus,
  UploadFilled,
  Upload,
  Download,
  Refresh,
  Delete,
  VideoPlay,
  VideoPause,
  FolderOpened,
} from '@element-plus/icons-vue'
import { useEditableOptionField } from '../composables/useEditableOptionField.js'
import { useVddEdid } from '../composables/useVddEdid.js'
import { useVddStatusLabel } from '../composables/useVddStatusLabel.js'
import { installVddWithRecovery } from '../composables/vddInstallRecovery.js'
import { vdd } from '../tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

const DEFAULT_REFRESH_RATES = ['60', '120', '240']
const suggestedResolutions = ['1280x720', '1920x1080', '2560x1440', '3840x2160']
const suggestedRefreshRates = ['60', '120', '144', '240']
const RESOLUTION_PATTERN = /^\d+x\d+$/
const REFRESH_RATE_PATTERN = /^\d+(\.\d+)?$/
const CHINESE_PATTERN = /[\u4e00-\u9fa5]/
const capabilityBadges = [
  { key: 'hdr', tone: 'hdr', text: 'HDR', sub: 'dynamic range' },
  { key: 'resolution', tone: 'resolution', text: '4K', sub: 'ultra hd' },
  { key: 'refresh', tone: 'refresh', text: '240Hz', sub: 'high refresh' },
  { key: 'sync', tone: 'sync', text: 'VRR', sub: 'adaptive sync' },
]
const COLOR_DEPTH_DEFAULT = 'default'
const COLOR_DEPTH_SDR10 = 'sdr10'
const COLOR_DEPTH_HDR12 = 'hdr12'
const COLOR_DEPTH_CONFLICT = 'conflict'

const createInitialSettings = () => ({
  monitors: { count: 1 },
  gpu: { friendlyname: '' },
  global: {
    g_refresh_rate: [...DEFAULT_REFRESH_RATES],
  },
  resolutions: { resolution: [] },
  colour: {
    SDR10bit: false,
    HDRPlus: false,
    ColourFormat: 'RGB',
  },
  cursor: {
    HardwareCursor: false,
    CursorMaxY: 128,
    CursorMaxX: 128,
    AlphaCursorSupport: true,
    XorCursorSupportLevel: 2,
  },
  edid: {
    CustomEdid: false,
    PreventSpoof: false,
    EdidCeaOverride: false,
    Vrr: false,
  },
})

const getErrorMessage = (error, fallback = '') => {
  if (error instanceof Error && error.message) {
    return error.message
  }

  if (typeof error === 'string' && error.trim()) {
    return error
  }

  return fallback
}

const fillPlaceholders = (message, replacements = {}) => Object.entries(replacements).reduce(
  (result, [key, value]) => result.replace(`{${key}}`, String(value)),
  message || ''
)

const getVddText = (key, replacements = {}) => fillPlaceholders(t.value.vddSettings[key], replacements)

const syncGpuOptions = (options = []) => {
  const uniqueOptions = [...new Set(options.filter(Boolean))]

  if (gpuFriendlyName.value && !uniqueOptions.includes(gpuFriendlyName.value)) {
    uniqueOptions.unshift(gpuFriendlyName.value)
  }

  gpuOptions.value = uniqueOptions
}

const settings = reactive(createInitialSettings())
const gpuFriendlyName = ref('')
const gpuOptions = ref([])
const configFilePath = ref('')
const traceStatus = ref({
  running: false,
  directory: '',
  latest_file: '',
})
const vddStatus = reactive({
  state: 'unknown',
  installed: false,
  running: false,
  control_available: false,
  installed_version: '',
  bundled_version: '',
  version_match: false,
  monitor_active: false,
  status_text: '',
})

const isLoading = ref(false)
const isCheckingDriver = ref(false)
const hasCompletedDriverCheck = ref(false)
const driverCheckPhase = ref('detecting')
const isInstallingDriver = ref(false)
const isSaving = ref(false)
const isReloadingDriver = ref(false)
const isDeletingEdid = ref(false)
const isStartingTrace = ref(false)
const isStoppingTrace = ref(false)
const isOpeningTraceFolder = ref(false)
const hasLoadedSnapshot = ref(false)
const lastSavedSnapshot = ref('')

const {
  edidFileExists,
  edidFilePath,
  edidInfo,
  getEdidFormatLabel,
  checkEdidFile,
  handleEdidToggle,
  handleEdidFileChange,
  downloadEdid,
  removeEdidFile: removeEdidFileAction,
} = useVddEdid({ t, settings })

const compareResolutions = (a, b) => {
  const [aWidth, aHeight] = a.split('x').map(Number)
  const [bWidth, bHeight] = b.split('x').map(Number)
  return aWidth * aHeight - bWidth * bHeight || aWidth - bWidth || aHeight - bHeight
}

const compareRefreshRates = (a, b) => parseFloat(a) - parseFloat(b)

const {
  options: resolutionOptions,
  draft: newResolution,
  visible: showResInput,
  inputRef: resInputRef,
  sortedOptions: sortedResolutionOptions,
  setValues: setResolutionOptions,
  showInput: showResolutionInput,
  addDraft: addResolution,
  addValue: quickAddResolution,
  confirmInput: handleResInputConfirm,
  removeValue: removeResolution,
} = useEditableOptionField({
  compare: compareResolutions,
  validate: (value) => {
    if (!RESOLUTION_PATTERN.test(value)) {
      return false
    }

    const [width, height] = value.split('x').map(Number)
    return width > 0 && height > 0
  },
  messages: {
    invalid: { type: 'warning', text: () => t.value.vddSettings.resolutionFormatError },
    exists: { type: 'info', text: () => t.value.vddSettings.resolutionExists },
    added: { type: 'success', text: (value) => getVddText('resolutionAdded', { value }) },
    removed: { type: 'info', text: (value) => getVddText('resolutionRemoved', { value }) },
    minOne: { type: 'error', text: () => t.value.vddSettings.resolutionMinOne },
  },
})

const {
  options: refreshRateOptions,
  draft: newRefreshRate,
  visible: showRateInput,
  inputRef: rateInputRef,
  sortedOptions: sortedRefreshRateOptions,
  setValues: setRefreshRateOptions,
  showInput: showRefreshRateInput,
  addDraft: addRefreshRate,
  addValue: quickAddRefreshRate,
  confirmInput: handleRateInputConfirm,
  removeValue: removeRefreshRate,
} = useEditableOptionField({
  initialValues: DEFAULT_REFRESH_RATES,
  compare: compareRefreshRates,
  validate: (value) => {
    if (!REFRESH_RATE_PATTERN.test(value)) {
      return false
    }

    const rate = parseFloat(value)
    return rate >= 1 && rate <= 480
  },
  messages: {
    invalid: { type: 'warning', text: () => t.value.vddSettings.refreshRateInvalidExt },
    exists: { type: 'warning', text: () => t.value.vddSettings.refreshRateExists },
    added: { type: 'success', text: (value) => getVddText('refreshRateAdded', { value }) },
    removed: { type: 'info', text: (value) => getVddText('refreshRateRemoved', { value }) },
    minOne: { type: 'error', text: () => t.value.vddSettings.refreshRateMinOne },
  },
})

const gpuSummary = computed(() => gpuFriendlyName.value || t.value.vddSettings.unboundGpu)
const presetSummary = computed(() => getVddText('presetsSummary', {
  resolutions: sortedResolutionOptions.value.length,
  rates: sortedRefreshRateOptions.value.length,
}))
const traceDisplayPath = computed(() => traceStatus.value.latest_file || t.value.vddSettings.vddTraceNoFile)
const hasCustomEdidIssue = computed(() => settings.edid.CustomEdid && !edidFileExists.value)
const isColorDepthConflict = computed(() => settings.colour.SDR10bit && settings.colour.HDRPlus)
const colorDepthProfile = computed({
  get() {
    if (settings.colour.SDR10bit && settings.colour.HDRPlus) {
      return COLOR_DEPTH_CONFLICT
    }

    if (settings.colour.SDR10bit) {
      return COLOR_DEPTH_SDR10
    }

    if (settings.colour.HDRPlus) {
      return COLOR_DEPTH_HDR12
    }

    return COLOR_DEPTH_DEFAULT
  },
  set(value) {
    settings.colour.SDR10bit = value === COLOR_DEPTH_SDR10
    settings.colour.HDRPlus = value === COLOR_DEPTH_HDR12
  },
})
const currentEdidModeLabel = computed(() => {
  if (!settings.edid.CustomEdid) {
    return t.value.vddSettings.builtInMode
  }

  return edidFileExists.value
    ? t.value.vddSettings.customModeReady
    : t.value.vddSettings.customModeMissing
})
const currentEdidModeHint = computed(() => {
  if (!settings.edid.CustomEdid) {
    return t.value.vddSettings.builtInModeDesc
  }

  return t.value.vddSettings.customModeDesc
})
const overviewCards = computed(() => [
  {
    key: 'mode',
    label: t.value.vddSettings.overviewMode,
    value: currentEdidModeLabel.value,
    hint: currentEdidModeHint.value,
  },
  {
    key: 'presets',
    label: t.value.vddSettings.overviewPresets,
    value: presetSummary.value,
  },
  {
    key: 'gpu',
    label: t.value.vddSettings.overviewGpu,
    value: gpuSummary.value,
  },
])
const heroActions = computed(() => [
  {
    key: 'reload',
    label: t.value.vddSettings.reload,
    icon: Refresh,
    loading: isLoading.value,
    plain: true,
    type: undefined,
    onClick: reloadSettings,
  },
  {
    key: 'save',
    label: t.value.vddSettings.saveAndApply,
    icon: UploadFilled,
    loading: isSaving.value,
    plain: false,
    type: 'primary',
    onClick: saveSettings,
  },
])
const buildSettingsPayload = () => ({
  ...settings,
  gpu: {
    friendlyname: gpuFriendlyName.value.trim(),
  },
  global: {
    g_refresh_rate: [...sortedRefreshRateOptions.value],
  },
  resolutions: {
    resolution: sortedResolutionOptions.value.map((res) => {
      const [width, height] = res.split('x').map(Number)
      return { width, height }
    }),
  },
})

const takeSnapshot = () => JSON.stringify(buildSettingsPayload())

const hasUnsavedChanges = computed(() => hasLoadedSnapshot.value && takeSnapshot() !== lastSavedSnapshot.value)
const vddReady = computed(() => vddStatus.running && ['ready', 'degraded'].includes(vddStatus.state))
const isDriverCheckPending = computed(() => !hasCompletedDriverCheck.value)
const driverDetectionTitle = computed(() => driverCheckPhase.value === 'syncing'
  ? t.value.vddSettings.driverSyncTitle
  : t.value.vddSettings.driverDetectionTitle)
const driverDetectionDescription = computed(() => driverCheckPhase.value === 'syncing'
  ? t.value.vddSettings.driverSyncDesc
  : t.value.vddSettings.driverDetectionDesc)
const vddCanInstall = computed(() => !['unsupported', 'payload_missing'].includes(vddStatus.state))
const vddStatusLabel = useVddStatusLabel(t, vddStatus)

const markSettingsClean = () => {
  lastSavedSnapshot.value = takeSnapshot()
  hasLoadedSnapshot.value = true
}

const applyLoadedSettings = (data) => {
  const initialSettings = createInitialSettings()
  const mergedData = {
    ...initialSettings,
    ...data,
    monitors: data?.monitors || initialSettings.monitors,
    gpu: data?.gpu || initialSettings.gpu,
    global: data?.global || initialSettings.global,
    resolutions: data?.resolutions || initialSettings.resolutions,
    colour: data?.colour || initialSettings.colour,
    cursor: data?.cursor || initialSettings.cursor,
    edid: data?.edid || initialSettings.edid,
  }

  Object.assign(settings, mergedData)

  gpuFriendlyName.value = mergedData.gpu?.friendlyname || ''
  settings.gpu.friendlyname = gpuFriendlyName.value

  const loadedResolutions = []
  for (const resolution of mergedData.resolutions?.resolution || []) {
    if (resolution.width && resolution.height) {
      loadedResolutions.push(`${resolution.width}x${resolution.height}`)
    }
  }

  const loadedRates = mergedData.global?.g_refresh_rate?.length
    ? mergedData.global.g_refresh_rate.map((rate) => String(rate))
    : initialSettings.global.g_refresh_rate
  setResolutionOptions(loadedResolutions)
  setRefreshRateOptions(loadedRates)
}

const applyTraceStatus = (data = {}) => {
  traceStatus.value = {
    running: Boolean(data.running),
    directory: data.directory || '',
    latest_file: data.latest_file || '',
  }
}

const refreshTraceStatus = async () => {
  try {
    const result = await vdd.getTraceStatus()
    if (result?.success) {
      applyTraceStatus(result.data)
    }
  } catch (error) {
    console.error('Failed to refresh VDD trace status:', error)
  }
}

const syncVddState = async ({ silentLoad = false } = {}) => Promise.all([
  loadSettings({ silent: silentLoad }),
  loadGPUs(),
  checkEdidFile(),
  loadSettingsPath(),
  refreshTraceStatus(),
])

const loadSettingsPath = async () => {
  const result = await vdd.getSettingsFilePath()
  if (result?.success) {
    configFilePath.value = result.data
  }
}

const loadSettings = async ({ silent = false } = {}) => {
  isLoading.value = true
  try {
    const result = await vdd.loadSettings()

    if (!result?.success) {
      applyLoadedSettings(createInitialSettings())
      if (!silent) {
        ElMessage.warning(t.value.vddSettings.loadDefault)
      }
      markSettingsClean()
      return false
    }

    applyLoadedSettings(result.data)
    markSettingsClean()

    if (!silent) {
      ElMessage.success(t.value.vddSettings.reloadSuccess)
    }

    return true
  } catch (error) {
    console.error('Load settings error:', error)
    if (!silent) {
      ElMessage.error(t.value.vddSettings.loadFailed)
    }
    return false
  } finally {
    isLoading.value = false
  }
}

const reloadSettings = async () => {
  await syncVddState()
}

const refreshVddStatus = async () => {
  isCheckingDriver.value = true
  try {
    const result = await vdd.getStatus()
    if (!result?.success) {
      throw new Error(result?.message || t.value.vddSettings.driverStatusCheckFailed)
    }
    Object.assign(vddStatus, result.data)
    return vddReady.value
  } catch (error) {
    vddStatus.state = 'unknown'
    vddStatus.status_text = getErrorMessage(error, t.value.vddSettings.driverStatusCheckFailed)
    return false
  } finally {
    isCheckingDriver.value = false
  }
}

const recheckDriver = async () => {
  hasCompletedDriverCheck.value = false
  driverCheckPhase.value = 'detecting'
  try {
    if (await refreshVddStatus()) {
      driverCheckPhase.value = 'syncing'
      await syncVddState({ silentLoad: true })
    }
  } finally {
    hasCompletedDriverCheck.value = true
  }
}

const installOrRepairDriver = async () => {
  if (hasUnsavedChanges.value) {
    ElMessage.warning(t.value.vddSettings.unsavedDesc)
    return
  }

  try {
    await ElMessageBox.confirm(
      t.value.vddSettings.installRepairConfirm,
      t.value.vddSettings.installRepairTitle,
      {
        confirmButtonText: t.value.vddSettings.installRepairDriver,
        cancelButtonText: t.value.vddSettings.cancel,
        type: 'warning',
      }
    )
    isInstallingDriver.value = true
    await installVddWithRecovery({
      install: () => vdd.install(),
      getStatus: () => vdd.getStatus(),
      onStatus: (status) => Object.assign(vddStatus, status),
      verificationError: t.value.vddSettings.installVerificationFailed,
    })
    await syncVddState({ silentLoad: true })
    ElMessage.success(t.value.vddSettings.installRepairSuccess)
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(getErrorMessage(error, t.value.vddSettings.installRepairFailed))
    }
  } finally {
    isInstallingDriver.value = false
  }
}

const loadGPUs = async () => {
  try {
    const result = await vdd.getGPUs()
    if (result?.success) {
      syncGpuOptions(result.data)
    }
  } catch (error) {
    console.error('Failed to get GPU list:', error)
  }
}

const saveSettings = async () => {
  if (CHINESE_PATTERN.test(gpuFriendlyName.value)) {
    ElMessage.error(t.value.vddSettings.saveGpuError)
    return
  }

  if (settings.edid.CustomEdid && !edidFileExists.value) {
    ElMessage.error(t.value.vddSettings.customEdidMissingSave)
    return
  }

  isSaving.value = true
  try {
    const payload = buildSettingsPayload()
    const result = await vdd.saveSettings(payload)

    if (!result?.success) {
      throw new Error(result?.message || t.value.vddSettings.unknownError)
    }

    await syncVddState({ silentLoad: true })
    markSettingsClean()
    ElMessage.success(t.value.vddSettings.saveSuccessDetail)
  } catch (error) {
    console.error('Save settings error:', error)
    ElMessage.error(t.value.vddSettings.saveFailed.replace('{error}', getErrorMessage(error, t.value.vddSettings.unknownError)))
  } finally {
    isSaving.value = false
  }
}

const saveGpuEdit = () => {
  const trimmedValue = gpuFriendlyName.value.trim()
  if (CHINESE_PATTERN.test(trimmedValue)) {
    ElMessage.error(t.value.vddSettings.gpuNameNoChinese)
    gpuFriendlyName.value = settings.gpu.friendlyname || ''
    return
  }

  gpuFriendlyName.value = trimmedValue

  if (trimmedValue) {
    syncGpuOptions([...gpuOptions.value, trimmedValue])
  }

  settings.gpu.friendlyname = trimmedValue
}

const reloadDriver = async () => {
  isReloadingDriver.value = true
  try {
    const success = await vdd.execPipeCmd('RELOAD_DRIVER')
    if (!success) {
      throw new Error(t.value.vddSettings.reloadDriverFailed)
    }

    ElMessage.success(t.value.vddSettings.reloadDriverSuccess)
  } catch (error) {
    ElMessage.error(getErrorMessage(error, t.value.vddSettings.reloadDriverFailed))
  } finally {
    isReloadingDriver.value = false
  }
}

const startTrace = async () => {
  isStartingTrace.value = true
  try {
    const result = await vdd.startTrace()
    if (!result?.success) {
      throw new Error(result?.message || t.value.vddSettings.vddTraceStartFailed)
    }

    applyTraceStatus(result.data)
    ElMessage.success(t.value.vddSettings.vddTraceStartSuccess)
  } catch (error) {
    ElMessage.error(getErrorMessage(error, t.value.vddSettings.vddTraceStartFailed))
  } finally {
    isStartingTrace.value = false
    await refreshTraceStatus()
  }
}

const stopTrace = async () => {
  isStoppingTrace.value = true
  try {
    const result = await vdd.stopTrace()
    if (!result?.success) {
      throw new Error(result?.message || t.value.vddSettings.vddTraceStopFailed)
    }

    applyTraceStatus(result.data)
    ElMessage.success(t.value.vddSettings.vddTraceStopSuccess)
  } catch (error) {
    ElMessage.error(getErrorMessage(error, t.value.vddSettings.vddTraceStopFailed))
  } finally {
    isStoppingTrace.value = false
    await refreshTraceStatus()
  }
}

const openTraceFolder = async () => {
  isOpeningTraceFolder.value = true
  try {
    const result = await vdd.openTraceFolder()
    if (!result?.success) {
      throw new Error(result?.message || t.value.vddSettings.vddTraceOpenFolderFailed)
    }
  } catch (error) {
    ElMessage.error(getErrorMessage(error, t.value.vddSettings.vddTraceOpenFolderFailed))
  } finally {
    isOpeningTraceFolder.value = false
  }
}

const removeEdidFile = async () => {
  isDeletingEdid.value = true
  try {
    await removeEdidFileAction()
  } finally {
    isDeletingEdid.value = false
  }
}

onMounted(async () => {
  await recheckDriver()
})
</script>

<style lang="less" scoped>
@import '../styles/VddSettings.less';
</style>
