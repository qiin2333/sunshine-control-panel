<template>
  <section class="ds5-page">
    <header class="page-header">
      <div>
        <div class="eyebrow">{{ t.dualSense.experimental }}</div>
        <h1>{{ t.dualSense.title }}</h1>
        <p>{{ t.dualSense.intro }}</p>
      </div>
      <el-button text circle :aria-label="t.dualSense.refresh" :loading="loading" @click="refresh()">
        <el-icon><Refresh /></el-icon>
      </el-button>
    </header>

    <article class="status-card" :class="`state-${status.state}`" aria-live="polite">
      <div class="status-heading">
        <span class="status-dot" aria-hidden="true"></span>
        <div>
          <span class="status-kicker">{{ t.dualSense.status }}</span>
          <strong>{{ stateLabel }}</strong>
        </div>
      </div>
      <p>{{ nextAction }}</p>
      <code v-if="status.error_code">{{ status.error_code }}</code>
    </article>

    <article v-if="operation === 'install'" class="operation-card" aria-live="polite">
      <div>
        <strong>{{ operationStage }}</strong>
        <span>{{ operationProgress }}%</span>
      </div>
      <el-progress :percentage="operationProgress" :show-text="false" />
    </article>

    <div class="capability-grid">
      <div v-for="item in capabilities" :key="item.label" class="capability-card">
        <span>{{ item.label }}</span>
        <strong :class="item.ok ? 'ok' : 'muted'">
          <el-icon><CircleCheck v-if="item.ok" /><Warning v-else /></el-icon>
          {{ item.ok ? t.dualSense.available : t.dualSense.unavailable }}
        </strong>
      </div>
    </div>

    <article class="settings-card">
      <div class="setting-row">
        <div>
          <strong>{{ t.dualSense.enable }}</strong>
          <p>{{ t.dualSense.enableTip }}</p>
        </div>
        <el-switch
          v-model="enabled"
          :disabled="!status.verified || status.in_use || saving"
          @change="saveSettings"
        />
      </div>
      <div class="setting-row">
        <div>
          <strong>{{ t.dualSense.haptics }}</strong>
          <p>{{ t.dualSense.hapticsTip }}</p>
        </div>
        <el-switch
          v-model="audioHaptics"
          :disabled="!status.verified || status.in_use || saving"
          @change="saveSettings"
        />
      </div>
    </article>

    <div class="actions">
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
      <template v-else>
        <el-button :loading="operation === 'standard'" :disabled="status.in_use" @click="test('standard')">
          {{ t.dualSense.testStandard }}
        </el-button>
        <el-button
          type="primary"
          plain
          :loading="operation === 'composite'"
          :disabled="status.in_use || !status.composite_profile || !status.usbip_available"
          @click="test('composite')"
        >{{ t.dualSense.testComposite }}</el-button>
        <el-button type="danger" plain :loading="operation === 'uninstall'" :disabled="status.in_use" @click="uninstall">
          {{ t.dualSense.uninstall }}
        </el-button>
      </template>
    </div>

    <footer class="notes">
      <p>{{ t.dualSense.source }}</p>
      <p>{{ t.dualSense.limitation }}</p>
      <details v-if="status.detail">
        <summary>{{ status.error_code || 'Details' }}</summary>
        <pre>{{ status.detail }}</pre>
      </details>
    </footer>
  </section>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { CircleCheck, Refresh, Warning } from '@element-plus/icons-vue'
import { dualsense } from '../tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()
const loading = ref(true)
const saving = ref(false)
const operation = ref('')
const operationProgress = ref(0)
const operationStageKey = ref('preparing')
const enabled = ref(false)
const audioHaptics = ref(true)
const status = ref({
  state: 'loading', installed: false, verified: false, enabled: false,
  audio_haptics: true, driver_installed: false, usbip_available: false,
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

const capabilities = computed(() => [
  { label: t.value.dualSense.component, ok: status.value.verified },
  { label: t.value.dualSense.runtime, ok: !!status.value.runtime_version },
  { label: t.value.dualSense.standard, ok: status.value.standard_profile },
  { label: t.value.dualSense.composite, ok: status.value.composite_profile && status.value.usbip_available },
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
    audioHaptics.value = result.data.audio_haptics
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
    audioHaptics.value = status.value.audio_haptics
    return showError(result.message)
  }
  status.value = result.data
  ElMessage.success(t.value.dualSense.configSuccess)
}

const test = async (profile) => {
  if (!await ensureAdmin()) return
  operation.value = profile
  const result = await dualsense.selfTest(profile)
  operation.value = ''
  if (!result.success) return showError(result.message)
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
.ds5-page { max-width: 920px; margin: 0 auto; padding: 26px 28px 42px; color: var(--el-text-color-primary); }
.page-header { display: flex; justify-content: space-between; gap: 20px; align-items: flex-start; margin-bottom: 20px;
  h1 { margin: 4px 0 8px; font-size: 28px; } p { margin: 0; max-width: 720px; color: var(--el-text-color-secondary); line-height: 1.55; }
}
.eyebrow { color: var(--el-color-primary); font-size: 12px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; }
.status-card, .settings-card, .capability-card { border: 1px solid var(--el-border-color); border-radius: 14px; background: var(--el-bg-color-overlay); }
.operation-card { margin-top: 14px; padding: 14px 16px; border: 1px solid var(--el-color-primary-light-7); border-radius: 12px; background: var(--el-color-primary-light-9); div { display: flex; justify-content: space-between; margin-bottom: 10px; } }
.status-card { padding: 18px 20px; border-left: 4px solid var(--el-color-info); p { margin: 10px 0 0 28px; color: var(--el-text-color-secondary); } code { display: inline-block; margin: 10px 0 0 28px; } }
.state-ready { border-left-color: var(--el-color-success); } .state-in_use { border-left-color: var(--el-color-warning); }
.state-repair_required { border-left-color: var(--el-color-danger); }
.status-heading { display: flex; gap: 12px; align-items: center; strong, span { display: block; } }
.status-dot { width: 12px; height: 12px; border-radius: 50%; background: currentColor; color: var(--el-color-info); }
.state-ready .status-dot { color: var(--el-color-success); } .state-in_use .status-dot { color: var(--el-color-warning); }
.state-repair_required .status-dot { color: var(--el-color-danger); }
.status-kicker { color: var(--el-text-color-secondary); font-size: 12px; margin-bottom: 2px; }
.capability-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; margin: 14px 0; }
.capability-card { padding: 14px; span { display: block; color: var(--el-text-color-secondary); font-size: 12px; margin-bottom: 8px; } strong { display: flex; gap: 6px; align-items: center; } .ok { color: var(--el-color-success); } .muted { color: var(--el-text-color-placeholder); } }
.settings-card { padding: 0 20px; }
.setting-row { display: flex; justify-content: space-between; align-items: center; gap: 30px; padding: 18px 0; & + & { border-top: 1px solid var(--el-border-color-lighter); } p { margin: 5px 0 0; color: var(--el-text-color-secondary); font-size: 13px; line-height: 1.45; } }
.actions { display: flex; flex-wrap: wrap; gap: 10px; margin: 18px 0; }
.notes { border-top: 1px solid var(--el-border-color-lighter); padding-top: 14px; color: var(--el-text-color-secondary); font-size: 12px; line-height: 1.55; pre { white-space: pre-wrap; overflow-wrap: anywhere; } }
@media (max-width: 760px) { .ds5-page { padding: 20px 16px 34px; } .capability-grid { grid-template-columns: repeat(2, 1fr); } .setting-row { align-items: flex-start; } }
</style>
