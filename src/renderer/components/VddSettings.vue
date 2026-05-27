<template>
  <div class="vdd-settings-wrapper">
    <div class="vdd-content">
      <div class="vdd-page-header">
        <h2>
          <el-icon class="header-icon"><Monitor /></el-icon>
          {{ t.vddSettings.title }}
        </h2>
        <div class="page-header-meta">
          <el-tag size="small" round effect="plain">{{ currentEdidModeLabel }}</el-tag>
          <el-tag v-if="hasUnsavedChanges" size="small" round type="warning" effect="light">
            {{ t.vddSettings.unsavedBadge }}
          </el-tag>
        </div>
      </div>

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
                <el-form-item
                  v-for="item in colourSwitchFields"
                  :key="item.key"
                  :label="t.vddSettings[item.labelKey]"
                >
                  <div class="field-stack">
                    <div class="field-inline-control">
                      <el-switch v-model="settings[item.groupKey][item.valueKey]" />
                    </div>
                    <span class="form-tip">{{ t.vddSettings[item.tipKey] }}</span>
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

                <el-form-item
                  v-for="item in loggingSwitchFields"
                  :key="item.key"
                  :label="t.vddSettings[item.labelKey]"
                >
                  <div class="field-stack">
                    <div class="field-inline-control">
                      <el-switch v-model="settings[item.groupKey][item.valueKey]" />
                    </div>
                    <span class="form-tip">{{ t.vddSettings[item.tipKey] }}</span>
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
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Monitor, Plus, UploadFilled, Upload, Download, Refresh, Delete } from '@element-plus/icons-vue'
import { useEditableOptionField } from '../composables/useEditableOptionField.js'
import { useVddEdid } from '../composables/useVddEdid.js'
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
const colourSwitchFields = [
  { key: 'sdr10bit', groupKey: 'colour', valueKey: 'SDR10bit', labelKey: 'sdr10bit', tipKey: 'sdr10bitTip' },
  { key: 'hdr12bit', groupKey: 'colour', valueKey: 'HDRPlus', labelKey: 'hdr12bit', tipKey: 'hdr12bitTip' },
]

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
  logging: {
    logging: false,
    debuglogging: false,
  },
  cursor: {
    HardwareCursor: true,
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

const isLoading = ref(false)
const isSaving = ref(false)
const isReloadingDriver = ref(false)
const isDeletingEdid = ref(false)
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
const hasCustomEdidIssue = computed(() => settings.edid.CustomEdid && !edidFileExists.value)
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
const loggingSwitchFields = computed(() => {
  const fields = [
    { key: 'logging', groupKey: 'logging', valueKey: 'logging', labelKey: 'loggingLabel', tipKey: 'loggingTip' },
  ]

  if (settings.logging.logging) {
    fields.push({
      key: 'debugLogging',
      groupKey: 'logging',
      valueKey: 'debuglogging',
      labelKey: 'debugLogging',
      tipKey: 'debugLoggingTip',
    })
  }

  return fields
})

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
    logging: data?.logging || initialSettings.logging,
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

const syncVddState = async ({ silentLoad = false } = {}) => Promise.all([
  loadSettings({ silent: silentLoad }),
  loadGPUs(),
  checkEdidFile(),
  loadSettingsPath(),
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

const removeEdidFile = async () => {
  isDeletingEdid.value = true
  try {
    await removeEdidFileAction()
  } finally {
    isDeletingEdid.value = false
  }
}

onMounted(async () => {
  await syncVddState({ silentLoad: true })
})
</script>

<style lang="less" scoped>
@import '../styles/VddSettings.less';
</style>
