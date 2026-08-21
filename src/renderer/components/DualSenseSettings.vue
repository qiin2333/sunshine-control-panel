<template>
  <section class="ds5-page">
    <header class="page-header">
      <div class="page-title-group">
        <p class="eyebrow">{{ t.controllers.eyebrow }}</p>
        <h1>{{ t.controllers.title }}</h1>
        <p>{{ t.controllers.intro }}</p>
      </div>
      <el-tag class="pixel-tag" effect="plain">{{ t.dualSense.experimental }}</el-tag>
    </header>

    <article class="component-panel" :class="`state-${status.state}`">
      <section class="title-row">
        <div>
          <h2>{{ t.dualSense.title }}</h2>
          <p>{{ t.dualSense.intro }}</p>
        </div>
        <el-checkbox
          v-model="enabled"
          class="enable-control"
          :disabled="!status.verified || status.in_use || controlsBusy"
          @change="saveSettings"
        >{{ saving ? t.dualSense.saving : t.dualSense.enableShort }}</el-checkbox>
      </section>

      <section class="status-row" aria-live="polite">
        <div class="status-heading">
          <span class="status-dot" aria-hidden="true"></span>
          <strong>{{ stateLabel }}</strong>
          <span v-if="overallVersion" class="status-version">{{ overallVersion }}</span>
        </div>
        <div class="status-actions">
          <el-button
            v-if="statusKnown && !status.installed"
            text
            class="menu-action menu-action-primary"
            :loading="operation === 'install'"
            :disabled="loading || status.in_use || controlsBusy"
            @click="install"
          >
            <span class="action-bracket" aria-hidden="true">[</span>
            <span>{{ t.dualSense.install }}</span>
            <span class="action-bracket" aria-hidden="true">]</span>
          </el-button>
          <el-button
            v-else-if="statusKnown && !status.verified"
            text
            class="menu-action menu-action-warning"
            :loading="operation === 'install'"
            :disabled="loading || status.in_use || controlsBusy"
            @click="install"
          >
            <span class="action-bracket" aria-hidden="true">[</span>
            <span>{{ t.dualSense.repair }}</span>
            <span class="action-bracket" aria-hidden="true">]</span>
          </el-button>
          <el-button
            text
            class="menu-action menu-action-secondary"
            :loading="loading"
            :disabled="controlsBusy"
            @click="refresh()"
          >
            <el-icon><Refresh /></el-icon>{{ t.dualSense.refresh }}
          </el-button>
        </div>
      </section>

      <article v-if="operation === 'install'" class="operation-card" aria-live="polite">
        <div><strong>{{ operationStage }}</strong><span>{{ operationProgress }}%</span></div>
        <el-progress :percentage="operationProgress" :show-text="false" />
      </article>

      <el-alert
        v-if="showNotice"
        class="notice"
        type="warning"
        :title="nextAction"
        :closable="false"
        show-icon
      />

      <section v-if="status.verified" class="profile-section" :aria-label="t.dualSense.profileTitle">
        <span class="section-label">{{ t.dualSense.profileTitle }}</span>
        <div class="profile-options" role="group">
          <div class="profile-option is-selected is-static">
            <span class="selection-cursor" aria-hidden="true">▶</span>
            <span class="option-copy">
              <strong>{{ t.dualSense.standardModeShort }}</strong>
              <small>{{ t.dualSense.standardModeTip }}</small>
            </span>
            <kbd>{{ t.dualSense.included }}</kbd>
          </div>
          <button
            type="button"
            class="profile-option"
            :class="{ 'is-selected': audioHaptics }"
            role="checkbox"
            :aria-checked="audioHaptics"
            :disabled="status.in_use || controlsBusy || (!status.usbip_available && !audioHaptics)"
            @click="setAudioHaptics(!audioHaptics)"
          >
            <span class="selection-cursor" aria-hidden="true">▶</span>
            <span class="option-copy">
              <strong>{{ t.dualSense.nativeModeShort }}</strong>
              <small>{{ status.usbip_available ? t.dualSense.nativeModeTip : t.dualSense.nativeUnavailable }}</small>
            </span>
            <kbd>{{ saving ? t.dualSense.saving : (audioHaptics ? t.dualSense.enabledLabel : t.dualSense.disabledLabel) }}</kbd>
          </button>
        </div>
      </section>

      <section v-if="status.verified" class="compatibility-section" :aria-label="t.dualSense.gameCompatibility">
        <span class="section-label">{{ t.dualSense.gameCompatibility }}</span>
        <div class="compatibility-option">
          <span class="option-copy">
            <strong>{{ t.dualSense.genshinMode }}</strong>
            <small>{{ status.genshin_compatibility_available ? t.dualSense.genshinModeTip : t.dualSense.genshinModeUnavailable }}</small>
          </span>
          <el-checkbox
            v-model="genshinCompatibility"
            :disabled="!status.genshin_compatibility_available || !enabled || !audioHaptics || status.in_use || controlsBusy"
            @change="setGenshinCompatibility"
          >{{ genshinCompatibility ? t.dualSense.enabledLabel : t.dualSense.disabledLabel }}</el-checkbox>
        </div>
        <p v-if="genshinCompatibility" class="compatibility-notice">
          <span aria-hidden="true">!</span>{{ t.dualSense.genshinModeActive }}
        </p>
      </section>

      <section v-if="status.verified" class="test-section">
        <div>
          <strong>{{ t.dualSense.validateMode }}</strong>
          <p>{{ testCompleted ? t.dualSense.validationTip : t.dualSense.validateModeTip }}</p>
        </div>
        <div class="test-actions">
          <el-button
            text
            :loading="operation === 'standard'"
            :disabled="status.in_use || controlsBusy || !status.standard_profile"
            @click="test('standard')"
          >[ {{ t.dualSense.testStandard }} ]</el-button>
          <el-button
            text
            :loading="operation === 'composite'"
            :disabled="status.in_use || controlsBusy || !canTestAudioHaptics"
            @click="test('composite')"
          >[ {{ t.dualSense.testComposite }} ]</el-button>
          <div v-if="testCompleted" class="controller-meta-action">
            <el-button text type="success" @click="emit('open-controller-meta')">
              [ {{ t.dualSense.openControllerMeta }} ]
            </el-button>
          </div>
        </div>
      </section>

      <el-collapse v-model="expandedSections" class="details-collapse">
        <el-collapse-item name="health" :title="t.dualSense.componentHealth">
          <div class="health-list">
            <div v-for="item in healthRows" :key="item.label" class="health-row">
              <span>{{ item.label }}</span>
              <strong class="health-state"><i :class="item.tone" aria-hidden="true"></i>{{ item.state }}</strong>
              <span class="health-detail">{{ item.detail }}</span>
            </div>
          </div>
          <footer class="panel-footer">
            <details v-if="status.detail">
              <summary>{{ status.error_code || t.dualSense.technicalDetails }}</summary>
              <pre>{{ status.detail }}</pre>
            </details>
            <el-button
              v-if="status.installed"
              link
              type="danger"
              :loading="operation === 'uninstall'"
              :disabled="status.in_use || controlsBusy"
              @click="uninstall"
            >{{ t.dualSense.uninstall }}</el-button>
          </footer>
        </el-collapse-item>
      </el-collapse>
    </article>
  </section>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { dualsense } from '../tauri-adapter.js'
import { dualSenseErrorCode, friendlyDualSenseError } from '../composables/dualsenseErrors.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()
const emit = defineEmits(['open-controller-meta'])
const loading = ref(true)
const statusKnown = ref(false)
const saving = ref(false)
const refreshing = ref(false)
const operation = ref('')
const operationProgress = ref(0)
const operationStageKey = ref('preparing')
const enabled = ref(false)
const audioHaptics = ref(false)
const genshinCompatibility = ref(false)
const testCompleted = ref(false)
const expandedSections = ref([])
const rebootRecommended = ref(false)
const status = ref({
  state: 'loading', installed: false, verified: false, enabled: false,
  audio_haptics: false, genshin_compatibility: false,
  genshin_compatibility_available: false,
  driver_installed: false, usbip_available: false, usbip_version: '',
  usbip_version_valid: false, reboot_recommended: false,
  standard_profile: false, composite_profile: false, in_use: false,
  error_code: '', detail: '',
})
let pollTimer
let unlistenProgress

const controlsBusy = computed(() => refreshing.value || saving.value || Boolean(operation.value))
const stateLabel = computed(() => t.value.dualSense.states[status.value.state] || status.value.state)
const operationStage = computed(() => t.value.dualSense.stages[operationStageKey.value] || operationStageKey.value)
const nextAction = computed(() => {
  if (rebootRecommended.value) {
    return status.value.usbip_available
      ? t.value.dualSense.restartSuggestedAvailable
      : t.value.dualSense.restartSuggestedUnavailable
  }
  return {
    not_installed: t.value.dualSense.nextNotInstalled,
    repair_required: t.value.dualSense.nextRepair,
    transport_missing: t.value.dualSense.nextTransport,
    in_use: t.value.dualSense.nextInUse,
  }[status.value.state] || t.value.dualSense.nextRepair
})

const overallVersion = computed(() => status.value.component_version || status.value.runtime_version || status.value.error_code)
const canTestAudioHaptics = computed(() => status.value.composite_profile && status.value.usbip_available)
const usbTransportDetail = computed(() => {
  if (!status.value.usbip_version_valid) return t.value.dualSense.pinnedTransportRequired
  if (status.value.usbip_available) return `USB/IP ${status.value.usbip_version}`
  if (!status.value.installed) return t.value.dualSense.transportCheckPending
  return t.value.dualSense.transportProbeFailed
})
const showNotice = computed(() => rebootRecommended.value || ['not_installed', 'repair_required', 'transport_missing', 'in_use'].includes(status.value.state))
const healthRows = computed(() => [
  {
    label: t.value.dualSense.component,
    state: status.value.verified ? t.value.dualSense.available : t.value.dualSense.unavailable,
    detail: status.value.component_version || status.value.error_code,
    tone: status.value.verified ? 'ok' : status.value.installed ? 'bad' : '',
  },
  {
    label: t.value.dualSense.runtime,
    state: status.value.runtime_version ? t.value.dualSense.available : t.value.dualSense.unavailable,
    detail: status.value.runtime_version,
    tone: status.value.runtime_version ? 'ok' : '',
  },
  {
    label: t.value.dualSense.standard,
    state: status.value.standard_profile ? t.value.dualSense.available : t.value.dualSense.unavailable,
    detail: status.value.in_use ? t.value.dualSense.deviceActive : t.value.dualSense.createdOnDemand,
    tone: status.value.in_use ? 'busy' : status.value.standard_profile ? 'ok' : '',
  },
  {
    label: t.value.dualSense.usbTransport,
    state: status.value.usbip_available ? t.value.dualSense.available : t.value.dualSense.unavailable,
    detail: usbTransportDetail.value,
    tone: status.value.usbip_available ? 'ok' : 'warn',
  },
  {
    label: t.value.dualSense.composite,
    state: status.value.composite_profile && status.value.usbip_available ? t.value.dualSense.available : t.value.dualSense.unavailable,
    detail: status.value.composite_profile ? '4 ch · 48 kHz' : '',
    tone: status.value.composite_profile && status.value.usbip_available ? 'ok' : 'warn',
  },
])

const showError = (message, context = 'generic') => ElMessage.error(
  friendlyDualSenseError(message, t.value.dualSense.errors, context),
)

const refresh = async (quiet = false) => {
  if (controlsBusy.value) return false
  refreshing.value = true
  if (!quiet) loading.value = true
  let refreshed = false
  try {
    const result = await dualsense.getStatus()
    if (result.success) {
      status.value = result.data
      statusKnown.value = true
      enabled.value = result.data.enabled
      audioHaptics.value = result.data.audio_haptics
      genshinCompatibility.value = result.data.genshin_compatibility ?? false
      refreshed = true
    } else if (!quiet) {
      showError(result.message, 'status')
    }
  } finally {
    loading.value = false
    refreshing.value = false
  }
  return refreshed
}

const install = async () => {
  if (controlsBusy.value) return
  operation.value = 'confirm-install'
  try {
    await ElMessageBox.confirm(t.value.dualSense.installConfirm, t.value.dualSense.installTitle, {
      type: 'warning', confirmButtonText: t.value.dualSense.install,
    })
  } catch {
    operation.value = ''
    return
  }
  operation.value = 'install'
  operationProgress.value = 0
  const result = await dualsense.install()
  if (!result.success) {
    operation.value = ''
    await refresh(true)
    return showError(result.message, 'install')
  }
  operation.value = ''
  status.value = result.data
  statusKnown.value = true
  enabled.value = result.data.enabled
  audioHaptics.value = result.data.audio_haptics
  genshinCompatibility.value = result.data.genshin_compatibility ?? false
  rebootRecommended.value = result.data.reboot_recommended
  ElMessage.success(t.value.dualSense.installSuccess)
  if (result.data.reboot_recommended) {
    ElMessage.warning(status.value.usbip_available
      ? t.value.dualSense.restartSuggestedAvailable
      : t.value.dualSense.restartSuggestedUnavailable)
  }
}

const saveSettings = async () => {
  if (refreshing.value || operation.value || saving.value) {
    enabled.value = status.value.enabled
    audioHaptics.value = status.value.audio_haptics
    genshinCompatibility.value = status.value.genshin_compatibility ?? false
    return
  }
  const requestedEnabled = enabled.value
  const requestedAudioHaptics = audioHaptics.value
  const requestedGenshinCompatibility = requestedEnabled && requestedAudioHaptics && genshinCompatibility.value
  genshinCompatibility.value = requestedGenshinCompatibility
  saving.value = true
  let result = await dualsense.setConfig(
    requestedEnabled,
    requestedAudioHaptics,
    requestedGenshinCompatibility,
  )
  let refreshedAfterFailure = false
  if (!result.success && dualSenseErrorCode(result.message) === 'DS5-CFG-003') {
    saving.value = false
    refreshedAfterFailure = await refresh(true)
    const canRetry = refreshedAfterFailure
      && !status.value.in_use
      && (!requestedEnabled || status.value.verified)
      && (!requestedEnabled || !requestedAudioHaptics || status.value.usbip_available)
    if (canRetry) {
      enabled.value = requestedEnabled
      audioHaptics.value = requestedAudioHaptics
      genshinCompatibility.value = requestedGenshinCompatibility
      saving.value = true
      result = await dualsense.setConfig(
        requestedEnabled,
        requestedAudioHaptics,
        requestedGenshinCompatibility,
      )
      refreshedAfterFailure = false
    }
  }
  if (!result.success) {
    saving.value = false
    if (!refreshedAfterFailure) {
      refreshedAfterFailure = await refresh(true)
    }
    if (!refreshedAfterFailure) {
      enabled.value = status.value.enabled
      audioHaptics.value = status.value.audio_haptics
      genshinCompatibility.value = status.value.genshin_compatibility ?? false
    }
    return showError(result.message, 'config')
  }
  saving.value = false
  status.value = result.data
  enabled.value = result.data.enabled
  audioHaptics.value = result.data.audio_haptics
  genshinCompatibility.value = result.data.genshin_compatibility ?? false
  ElMessage.success(t.value.dualSense.configSuccess)
}

const setAudioHaptics = async (enabled) => {
  if (controlsBusy.value || audioHaptics.value === enabled) return
  audioHaptics.value = enabled
  if (!enabled) genshinCompatibility.value = false
  testCompleted.value = false
  await saveSettings()
}

const setGenshinCompatibility = async (value) => {
  if (controlsBusy.value) return
  genshinCompatibility.value = Boolean(value)
  await saveSettings()
}

const test = async (profile) => {
  if (controlsBusy.value) return
  operation.value = profile
  const result = await dualsense.selfTest(profile)
  operation.value = ''
  if (!result.success) return showError(result.message, 'test')
  testCompleted.value = true
  ElMessage.success(t.value.dualSense.testSuccess)
  await refresh()
}

const uninstall = async () => {
  if (controlsBusy.value) return
  operation.value = 'confirm-uninstall'
  try {
    await ElMessageBox.confirm(t.value.dualSense.uninstallConfirm, t.value.dualSense.uninstallTitle, {
      type: 'warning', confirmButtonText: t.value.dualSense.uninstall,
    })
  } catch {
    operation.value = ''
    return
  }
  operation.value = 'uninstall'
  const result = await dualsense.uninstall()
  operation.value = ''
  if (!result.success) return showError(result.message, 'uninstall')
  rebootRecommended.value = false
  ElMessage.success(t.value.dualSense.uninstallSuccess)
  await refresh()
}

onMounted(async () => {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    unlistenProgress = await listen('dualsense-operation-progress', ({ payload }) => {
      operationStageKey.value = payload?.stage || 'preparing'
      operationProgress.value = Number(payload?.progress || 0)
    })
  } catch {
    // Browser-only renderer previews do not provide the Tauri event bridge.
  }
  await refresh()
  pollTimer = window.setInterval(() => {
    if (controlsBusy.value) return
    refresh(true)
  }, 30000)
})
onUnmounted(() => {
  window.clearInterval(pollTimer)
  unlistenProgress?.()
})
</script>

<style scoped lang="less">
.ds5-page {
  max-width: 960px;
  margin: 0 auto;
  padding: 26px 28px 42px;
  color: var(--el-text-color-primary);
  font-family: 'Microsoft YaHei UI', 'Noto Sans SC', sans-serif;
  font-size: 14px;
  line-height: 1.5;
}
.page-header { display: flex; justify-content: space-between; gap: 24px; align-items: flex-start; margin-bottom: 34px; }
.page-title-group {
  h1 { margin: 0 0 7px; font-size: 24px; font-weight: 500; }
  > p:last-child { margin: 0; color: var(--el-text-color-secondary); font-size: 15px; line-height: 1.6; }
}
.eyebrow { font-family: 'PixelMplus12', 'Courier New', monospace; }
.eyebrow, .section-label { color: var(--el-text-color-secondary); letter-spacing: .1em; }
.eyebrow { font-size: 12px; }
.section-label { font-size: 13px; }
.eyebrow { margin: 0 0 7px; }
.pixel-tag { border: 0; border-radius: 0; color: var(--el-color-primary); background: transparent; font-size: 13px; letter-spacing: .05em; }
.component-panel { max-width: 820px; margin: 0 auto; padding: 8px; }
.title-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 32px; margin-bottom: 24px; }
.title-row {
  h2 { margin: 0 0 7px; font-size: 22px; font-weight: 500; }
  p { margin: 0; max-width: 560px; color: var(--el-text-color-secondary); line-height: 1.5; }
}
.enable-control { font-size: 14px; font-weight: 500; }
.status-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 32px; margin-bottom: 30px; }
.status-heading, .status-actions, .test-actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
.status-actions { gap: 18px; }
.status-actions :deep(.el-button.menu-action) {
  min-height: 30px;
  margin: 0;
  padding: 3px 2px;
  border: 0;
  border-radius: 0 !important;
  color: var(--el-text-color-regular);
  background: transparent;
  font: inherit;
  font-weight: 500;
  box-shadow: none;
  transition: color .12s ease, transform .12s ease;
}
.status-actions :deep(.el-button.menu-action:hover),
.status-actions :deep(.el-button.menu-action:focus-visible) {
  color: var(--el-color-primary);
  background: transparent;
  transform: translateY(-1px);
}
.status-actions :deep(.el-button.menu-action:focus-visible) { outline: 1px dashed currentColor; outline-offset: 3px; }
.status-actions :deep(.el-button.menu-action.is-disabled) { background: transparent; transform: none; }
.status-actions :deep(.el-button.menu-action-primary) { color: var(--el-color-primary); }
.status-actions :deep(.el-button.menu-action-warning) { color: var(--el-color-warning); }
.status-actions :deep(.el-button.menu-action-secondary) { color: var(--el-text-color-secondary); font-weight: 400; }
.action-bracket { font-family: 'PixelMplus12', 'Courier New', monospace; opacity: .8; }
.status-version { color: var(--el-text-color-secondary); font-size: 13px; }
.status-dot { width: 8px; height: 8px; flex: 0 0 auto; background: var(--el-text-color-placeholder); }
.state-ready .status-dot { background: var(--el-color-success); box-shadow: 0 0 0 3px var(--el-color-success-light-8); }
.state-transport_missing .status-dot, .state-in_use .status-dot { background: var(--el-color-warning); box-shadow: 0 0 0 3px var(--el-color-warning-light-8); }
.state-repair_required .status-dot { background: var(--el-color-danger); box-shadow: 0 0 0 3px var(--el-color-danger-light-8); }
.operation-card { padding: 0 0 20px; div { display: flex; justify-content: space-between; margin-bottom: 7px; color: var(--el-text-color-secondary); font-size: 13px; } }
.notice { margin: 0 0 20px; padding-inline: 0; border-radius: 0; background: transparent; }
.profile-section { margin-top: 4px; }
.section-label { display: block; margin-bottom: 10px; }
.profile-options { display: grid; gap: 4px; }
.profile-option {
  display: grid;
  grid-template-columns: 20px minmax(0, 1fr) auto;
  gap: 10px;
  align-items: center;
  width: 100%;
  min-height: 58px;
  padding: 8px 12px;
  border: 0;
  color: var(--el-text-color-secondary);
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
  &.is-static { cursor: default; }
  &:disabled { cursor: not-allowed; opacity: .55; }
  &.is-selected { color: var(--el-text-color-primary); background: var(--el-color-primary-light-9); box-shadow: inset 4px 0 0 var(--el-color-primary); }
  kbd { min-width: 48px; padding: 3px 6px; border: 1px solid var(--el-border-color); color: var(--el-text-color-secondary); background: transparent; font: inherit; font-size: 13px; text-align: center; }
}
.selection-cursor { visibility: hidden; color: var(--el-color-primary); font-family: 'PixelMplus12', 'Courier New', monospace; }
.profile-option.is-selected .selection-cursor { visibility: visible; }
.option-copy {
  min-width: 0;
  strong, small { display: block; }
  strong { font-weight: 500; }
  small { margin-top: 4px; color: var(--el-text-color-secondary); font-size: 13px; line-height: 1.55; }
}
.compatibility-section { margin-top: 26px; }
.compatibility-option {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 24px;
  align-items: center;
  padding: 4px 12px;
}
.compatibility-notice {
  display: flex;
  gap: 9px;
  margin: 10px 12px 0;
  color: var(--el-color-warning-dark-2);
  font-size: 13px;
  line-height: 1.55;
  span { flex: 0 0 auto; font-family: 'PixelMplus12', 'Courier New', monospace; font-weight: 700; }
}
.test-section { display: grid; grid-template-columns: minmax(0, 1fr); gap: 14px; margin-top: 26px; p { margin: 5px 0 0; max-width: 680px; color: var(--el-text-color-secondary); font-size: 13px; line-height: 1.55; } }
.test-actions { justify-content: flex-start; }
.controller-meta-action { flex: 0 0 100%; }
.controller-meta-action :deep(.el-button) { margin-left: 0; }
.details-collapse { margin-top: 24px; border: 0; :deep(.el-collapse-item__header) { border: 0; color: var(--el-text-color-secondary); background: transparent; font-family: inherit; font-size: 14px; } :deep(.el-collapse-item__wrap) { border: 0; background: transparent; } :deep(.el-collapse-item__content) { padding-bottom: 0; color: inherit; font-family: inherit; } }
.health-list { display: grid; padding: 0 8px 10px 20px; }
.health-row { display: grid; grid-template-columns: minmax(140px, 1fr) minmax(110px, auto) minmax(0, 1fr); gap: 16px; align-items: center; min-height: 44px; }
.health-state { display: flex; align-items: center; gap: 7px; font-weight: 500; i { width: 7px; height: 7px; background: var(--el-text-color-placeholder); } i.ok { background: var(--el-color-success); } i.busy { background: var(--el-color-primary); } i.warn { background: var(--el-color-warning); } i.bad { background: var(--el-color-danger); } }
.health-detail { min-width: 0; color: var(--el-text-color-secondary); font-size: 13px; text-align: right; overflow-wrap: anywhere; }
.panel-footer { display: grid; gap: 8px; padding: 14px 8px 14px 20px; color: var(--el-text-color-secondary); font-size: 13px; overflow-wrap: anywhere; details summary { cursor: pointer; } pre { white-space: pre-wrap; overflow-wrap: anywhere; } }
.ds5-page :deep(.el-button),
.ds5-page :deep(.el-checkbox__inner),
.ds5-page :deep(.el-progress-bar__outer),
.ds5-page :deep(.el-progress-bar__inner) { border-radius: 0; }
.ds5-page :deep(.el-button.is-text) { padding-inline: 4px; }
:global([data-bs-theme='dark']) .ds5-page {
  --el-bg-color-overlay: rgba(61, 50, 53, .72);
  --el-fill-color-blank: rgba(255, 255, 255, .055);
  --el-fill-color-light: rgba(255, 255, 255, .055);
  --el-fill-color-lighter: rgba(255, 255, 255, .035);
  --el-fill-color-extra-light: rgba(255, 255, 255, .025);
  --el-text-color-primary: #f0dfc3;
  --el-text-color-regular: #d6c3a7;
  --el-text-color-secondary: #b9aca8;
  --el-text-color-placeholder: #9d9494;
  --el-border-color: rgba(230, 213, 184, .22);
  --el-border-color-lighter: rgba(230, 213, 184, .11);
  --el-border-color-extra-light: rgba(230, 213, 184, .08);
  --el-color-primary-light-9: rgba(212, 165, 165, .12);
  --el-color-success-light-8: rgba(103, 194, 58, .18);
  --el-color-success-light-9: rgba(103, 194, 58, .11);
  --el-color-warning-light-8: rgba(230, 162, 60, .2);
  --el-color-warning-light-9: rgba(230, 162, 60, .12);
  --el-color-warning-dark-2: #f0bd70;
  --el-color-danger-light-8: rgba(245, 108, 108, .19);
  color: var(--el-text-color-primary);
}
@media (max-width: 760px) {
  .ds5-page { padding: 20px 16px 34px; }
  .health-row { grid-template-columns: minmax(0, 1fr) minmax(0, auto); gap: 8px; padding: 10px 0; }
  .health-detail { grid-column: 1 / -1; text-align: left; }
}
@media (max-width: 600px) {
  .page-header { align-items: flex-start; flex-direction: column; }
  .title-row, .status-row, .test-section { grid-template-columns: 1fr; gap: 12px; }
  .enable-control { justify-self: stretch; }
  .status-actions, .test-actions { justify-content: flex-start; }
  .profile-option { grid-template-columns: 16px minmax(0, 1fr) auto; padding-inline: 6px; }
  .compatibility-option { grid-template-columns: 1fr; gap: 8px; padding-inline: 6px; }
}
</style>
