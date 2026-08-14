<template>
  <section class="ds5-page">
    <header class="page-header">
      <div class="page-title-group">
        <span class="page-title-icon" aria-hidden="true"><el-icon><Aim /></el-icon></span>
        <div>
          <h1>{{ t.controllers.title }}</h1>
          <p>{{ t.controllers.intro }}</p>
        </div>
      </div>
      <el-tag round effect="light">{{ t.dualSense.experimental }}</el-tag>
    </header>

    <article class="component-panel">
      <div class="component-heading">
        <div>
          <h2>{{ t.dualSense.title }}</h2>
          <p>{{ t.dualSense.intro }}</p>
        </div>
      </div>

      <section class="status-box" :class="`state-${status.state}`" aria-live="polite">
        <div class="status-top">
          <div class="status-heading">
            <span class="status-dot" aria-hidden="true"></span>
            <strong>{{ stateLabel }}</strong>
            <span v-if="overallVersion" class="status-version">{{ overallVersion }}</span>
          </div>
          <div class="status-actions">
            <el-button
              v-if="!status.installed"
              type="primary"
              :loading="operation === 'install'"
              :disabled="status.in_use"
              @click="install"
            >{{ t.dualSense.install }}</el-button>
            <el-button
              v-else-if="!status.verified"
              type="warning"
              :loading="operation === 'install'"
              :disabled="status.in_use"
              @click="install"
            >{{ t.dualSense.repair }}</el-button>
            <el-button
              v-else
              type="primary"
              :loading="operation === selectedTestProfile"
              :disabled="status.in_use || !canTestSelectedProfile"
              @click="test(selectedTestProfile)"
            >{{ selectedTestLabel }}</el-button>
            <el-button :loading="loading" :disabled="!!operation" @click="refresh()">
              <el-icon><Refresh /></el-icon>
              {{ t.dualSense.refresh }}
            </el-button>
          </div>
        </div>
        <article v-if="operation === 'install'" class="operation-card" aria-live="polite">
          <div>
            <strong>{{ operationStage }}</strong>
            <span>{{ operationProgress }}%</span>
          </div>
          <el-progress :percentage="operationProgress" :show-text="false" />
        </article>
      </section>

      <div class="section-title"><span>{{ t.dualSense.componentHealth }}</span></div>
      <div class="health-list">
        <div v-for="item in healthRows" :key="item.label" class="health-row">
          <span>{{ item.label }}</span>
          <strong class="health-state"><i :class="item.tone" aria-hidden="true"></i>{{ item.state }}</strong>
          <span class="health-detail">{{ item.detail }}</span>
        </div>
      </div>

      <template v-if="status.verified">
        <div class="section-title"><span>{{ t.dualSense.profileTitle }}</span></div>
        <div class="settings-list">
          <div class="setting-row">
            <div>
              <strong>{{ t.dualSense.enable }}</strong>
              <p>{{ t.dualSense.enableTip }}</p>
            </div>
            <el-switch
              v-model="enabled"
              :disabled="status.in_use || saving"
              @change="saveSettings"
            />
          </div>
          <div class="setting-row profile-row">
            <div>
              <strong>{{ t.dualSense.profileTitle }}</strong>
              <p>{{ selectedProfileDescription }}</p>
            </div>
            <el-radio-group
              :model-value="audioHaptics ? 'native' : 'standard'"
              :disabled="status.in_use || saving"
              @change="selectProfile($event === 'native')"
            >
              <el-radio-button value="standard">{{ t.dualSense.standardModeShort }}</el-radio-button>
              <el-radio-button value="native" :disabled="!status.usbip_available">
                {{ t.dualSense.nativeModeShort }}
              </el-radio-button>
            </el-radio-group>
          </div>
        </div>
      </template>

      <div v-if="showNotice" class="notice" role="status">
        <el-icon><Warning /></el-icon>
        <span>{{ nextAction }}</span>
      </div>

      <article v-if="testCompleted" class="validation-card" aria-live="polite">
        <div>
          <strong>{{ t.dualSense.validationTitle }}</strong>
          <p>{{ t.dualSense.validationTip }}</p>
        </div>
        <el-button type="primary" plain @click="emit('open-controller-meta')">
          {{ t.dualSense.openControllerMeta }}
        </el-button>
      </article>

      <footer class="panel-footer">
        <span v-if="status.install_path">{{ t.dualSense.installLocation }}：{{ status.install_path }}</span>
        <span>{{ t.dualSense.source }}</span>
        <div class="footer-actions">
          <details v-if="status.detail">
            <summary>{{ status.error_code || t.dualSense.technicalDetails }}</summary>
            <pre>{{ status.detail }}</pre>
          </details>
          <el-button
            v-if="status.installed"
            link
            type="danger"
            :loading="operation === 'uninstall'"
            :disabled="status.in_use"
            @click="uninstall"
          >{{ t.dualSense.uninstall }}</el-button>
        </div>
      </footer>
      <p class="limitation">{{ t.dualSense.limitation }}</p>
    </article>
  </section>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Aim, Refresh, Warning } from '@element-plus/icons-vue'
import { dualsense } from '../tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()
const emit = defineEmits(['open-controller-meta'])
const loading = ref(true)
const saving = ref(false)
const operation = ref('')
const operationProgress = ref(0)
const operationStageKey = ref('preparing')
const enabled = ref(false)
const audioHaptics = ref(false)
const testCompleted = ref(false)
const status = ref({
  state: 'loading', installed: false, verified: false, enabled: false,
  audio_haptics: false, driver_installed: false, usbip_available: false,
  standard_profile: false, composite_profile: false, in_use: false,
  error_code: '', detail: '',
})
let pollTimer
let unlistenProgress

const stateLabel = computed(() => t.value.dualSense.states[status.value.state] || status.value.state)
const operationStage = computed(() => t.value.dualSense.stages[operationStageKey.value] || operationStageKey.value)
const nextAction = computed(() => ({
  not_installed: t.value.dualSense.nextNotInstalled,
  repair_required: t.value.dualSense.nextRepair,
  transport_missing: t.value.dualSense.nextTransport,
  ready: t.value.dualSense.nextReady,
  in_use: t.value.dualSense.nextInUse,
  loading: t.value.dualSense.states.loading,
}[status.value.state] || t.value.dualSense.nextRepair))

const overallVersion = computed(() => status.value.component_version || status.value.runtime_version || status.value.error_code)
const selectedTestProfile = computed(() => audioHaptics.value ? 'composite' : 'standard')
const selectedTestLabel = computed(() => audioHaptics.value ? t.value.dualSense.testComposite : t.value.dualSense.testStandard)
const selectedProfileDescription = computed(() => {
  if (!audioHaptics.value) return t.value.dualSense.standardModeTip
  return status.value.usbip_available ? t.value.dualSense.nativeModeTip : t.value.dualSense.nativeUnavailable
})
const canTestSelectedProfile = computed(() => audioHaptics.value
  ? status.value.composite_profile && status.value.usbip_available
  : status.value.standard_profile)
const showNotice = computed(() => ['not_installed', 'repair_required', 'transport_missing', 'in_use'].includes(status.value.state))
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
    detail: status.value.usbip_available ? t.value.dualSense.nativeModeShort : t.value.dualSense.standardStillAvailable,
    tone: status.value.usbip_available ? 'ok' : 'warn',
  },
  {
    label: t.value.dualSense.composite,
    state: status.value.composite_profile && status.value.usbip_available ? t.value.dualSense.available : t.value.dualSense.unavailable,
    detail: status.value.composite_profile ? '4 ch · 48 kHz' : '',
    tone: status.value.composite_profile && status.value.usbip_available ? 'ok' : 'warn',
  },
])

const showError = (message) => ElMessage.error(
  t.value.dualSense.operationFailed.replace('{error}', String(message || 'Unknown error')),
)

const ensureAdmin = async () => {
  if (await dualsense.isAdmin()) return true
  try {
    await ElMessageBox.confirm(t.value.dualSense.adminRequired, t.value.dualSense.adminTitle, {
      type: 'warning', confirmButtonText: t.value.dualSense.restartAdmin,
    })
  } catch { return false }
  const result = await dualsense.restartAsAdmin()
  if (!result.success) showError(result.message)
  return false
}

const refresh = async (quiet = false) => {
  if (!quiet) loading.value = true
  const result = await dualsense.getStatus()
  if (result.success) {
    status.value = result.data
    enabled.value = result.data.enabled
    audioHaptics.value = result.data.audio_haptics && result.data.usbip_available
  } else if (!quiet) {
    showError(result.message)
  }
  loading.value = false
}

const install = async () => {
  if (!await ensureAdmin()) return
  try {
    await ElMessageBox.confirm(t.value.dualSense.installConfirm, t.value.dualSense.installTitle, {
      type: 'warning', confirmButtonText: t.value.dualSense.install,
    })
  } catch { return }
  operation.value = 'install'
  operationProgress.value = 0
  const result = await dualsense.install()
  operation.value = ''
  if (!result.success) return showError(result.message)
  ElMessage.success(t.value.dualSense.installSuccess)
  await refresh()
}

const saveSettings = async () => {
  saving.value = true
  const result = await dualsense.setConfig(enabled.value, audioHaptics.value)
  saving.value = false
  if (!result.success) {
    enabled.value = status.value.enabled
    audioHaptics.value = status.value.audio_haptics && status.value.usbip_available
    return showError(result.message)
  }
  status.value = result.data
  ElMessage.success(t.value.dualSense.configSuccess)
}

const selectProfile = async (nativeHaptics) => {
  if (audioHaptics.value === nativeHaptics) return
  audioHaptics.value = nativeHaptics
  testCompleted.value = false
  await saveSettings()
}

const test = async (profile) => {
  if (!await ensureAdmin()) return
  operation.value = profile
  const result = await dualsense.selfTest(profile)
  operation.value = ''
  if (!result.success) return showError(result.message)
  testCompleted.value = true
  ElMessage.success(t.value.dualSense.testSuccess)
  await refresh()
}

const uninstall = async () => {
  if (!await ensureAdmin()) return
  try {
    await ElMessageBox.confirm(t.value.dualSense.uninstallConfirm, t.value.dualSense.uninstallTitle, {
      type: 'warning', confirmButtonText: t.value.dualSense.uninstall,
    })
  } catch { return }
  operation.value = 'uninstall'
  const result = await dualsense.uninstall()
  operation.value = ''
  if (!result.success) return showError(result.message)
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
    if (operation.value || saving.value) return
    refresh(true)
  }, 30000)
})
onUnmounted(() => {
  window.clearInterval(pollTimer)
  unlistenProgress?.()
})
</script>

<style scoped lang="less">
.ds5-page { max-width: 960px; margin: 0 auto; padding: 26px 28px 42px; color: var(--el-text-color-primary); }
.page-header { display: flex; justify-content: space-between; gap: 20px; align-items: flex-start; margin-bottom: 18px; }
.page-title-group { display: flex; gap: 12px; align-items: flex-start; h1 { margin: 0 0 6px; font-size: 24px; font-weight: 600; } p { margin: 0; max-width: 700px; color: var(--el-text-color-secondary); line-height: 1.5; } }
.page-title-icon { display: grid; place-items: center; width: 34px; height: 34px; flex: 0 0 auto; border-radius: 10px; color: var(--el-color-primary); background: var(--el-color-primary-light-9); font-size: 18px; }
.component-panel { max-width: 780px; margin: 0 auto; padding: 24px; border: 1px solid var(--el-border-color); border-radius: 15px; background: var(--el-bg-color-overlay); box-shadow: var(--el-box-shadow-light); }
.component-heading { h2 { margin: 0 0 8px; font-size: 20px; font-weight: 600; } p { margin: 0 0 18px; color: var(--el-text-color-secondary); line-height: 1.55; } }
.status-box { padding: 16px; border: 1px solid var(--el-border-color-lighter); border-radius: 12px; background: var(--el-fill-color-lighter); }
.status-top { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
.status-heading { display: flex; gap: 10px; align-items: center; }
.status-version { color: var(--el-text-color-secondary); font-size: 13px; }
.status-dot { width: 10px; height: 10px; flex: 0 0 auto; border-radius: 50%; background: var(--el-text-color-placeholder); }
.state-ready .status-dot { background: var(--el-color-success); box-shadow: 0 0 0 4px var(--el-color-success-light-8); }
.state-transport_missing .status-dot, .state-in_use .status-dot { background: var(--el-color-warning); box-shadow: 0 0 0 4px var(--el-color-warning-light-8); }
.state-repair_required .status-dot { background: var(--el-color-danger); box-shadow: 0 0 0 4px var(--el-color-danger-light-8); }
.status-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.operation-card { margin-top: 14px; div { display: flex; justify-content: space-between; margin-bottom: 7px; color: var(--el-text-color-secondary); font-size: 12px; } }
.section-title { display: flex; align-items: center; gap: 10px; margin: 22px 0 9px; color: var(--el-text-color-regular); font-weight: 600; &::after { content: ''; flex: 1; height: 1px; background: var(--el-border-color-lighter); } }
.health-list { display: grid; }
.health-row { display: grid; grid-template-columns: minmax(140px, 1fr) minmax(110px, auto) minmax(0, 1fr); gap: 16px; align-items: center; min-height: 44px; border-bottom: 1px solid var(--el-border-color-extra-light); &:last-child { border-bottom: 0; } }
.health-state { display: flex; align-items: center; gap: 7px; font-weight: 500; i { width: 7px; height: 7px; border-radius: 50%; background: var(--el-text-color-placeholder); } i.ok { background: var(--el-color-success); } i.busy { background: var(--el-color-primary); } i.warn { background: var(--el-color-warning); } i.bad { background: var(--el-color-danger); } }
.health-detail { min-width: 0; color: var(--el-text-color-secondary); font-size: 12px; text-align: right; overflow-wrap: anywhere; }
.settings-list { border: 1px solid var(--el-border-color-lighter); border-radius: 12px; padding: 0 16px; }
.setting-row { display: flex; justify-content: space-between; align-items: center; gap: 24px; min-height: 64px; padding: 12px 0; & + & { border-top: 1px solid var(--el-border-color-extra-light); } p { margin: 5px 0 0; color: var(--el-text-color-secondary); font-size: 12px; line-height: 1.45; } }
.notice { display: flex; gap: 9px; align-items: flex-start; margin-top: 16px; padding: 12px; border-radius: 10px; color: var(--el-color-warning-dark-2); background: var(--el-color-warning-light-9); }
.validation-card { display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 14px 16px; margin-top: 16px; border: 1px solid var(--el-color-success-light-5); border-radius: 12px; background: var(--el-color-success-light-9); p { margin: 5px 0 0; color: var(--el-text-color-secondary); font-size: 12px; line-height: 1.45; } }
.panel-footer { display: grid; gap: 8px; margin-top: 19px; padding-top: 14px; border-top: 1px solid var(--el-border-color-lighter); color: var(--el-text-color-secondary); font-size: 12px; overflow-wrap: anywhere; }
.footer-actions { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; details { min-width: 0; summary { cursor: pointer; } pre { white-space: pre-wrap; overflow-wrap: anywhere; } } }
.limitation { margin: 12px 0 0; color: var(--el-text-color-secondary); font-size: 12px; line-height: 1.5; }
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
  .page-title-icon { color: #d8a0a0; border: 1px solid rgba(216, 160, 160, .24); background: rgba(216, 160, 160, .12); }
  .component-panel { box-shadow: 0 10px 30px rgba(0, 0, 0, .2); }
}
@media (max-width: 760px) {
  .ds5-page { padding: 20px 16px 34px; }
  .component-panel { padding: 18px; }
  .health-row { grid-template-columns: minmax(0, 1fr) minmax(0, auto); gap: 8px; padding: 10px 0; }
  .health-detail { grid-column: 1 / -1; text-align: left; }
  .profile-row { align-items: flex-start; flex-direction: column; }
  .validation-card { align-items: flex-start; flex-direction: column; }
}
</style>
