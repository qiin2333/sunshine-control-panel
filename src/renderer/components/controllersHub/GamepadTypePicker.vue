<template>
  <div class="chub-type-picker">
    <p class="chub-question">{{ t.controllersHub.emulation.typeQuestion }}</p>

    <el-alert
      v-if="unreachable"
      class="chub-notice"
      type="error"
      :title="t.controllersHub.emulation.sunshineUnreachable"
      show-icon
      :closable="false"
    >
      <el-button size="small" type="primary" :loading="loading" @click="loadConfig">
        {{ t.controllersHub.emulation.retry }}
      </el-button>
    </el-alert>

    <div v-else v-loading="loading" class="chub-picker-row">
      <div class="chub-mode-list" role="radiogroup" :aria-label="t.controllersHub.emulation.typeQuestion">
        <button
          v-for="option in modeOptions"
          :key="option.value"
          type="button"
          class="chub-mode-option"
          :class="{ 'is-selected': selectedMode === option.value }"
          role="radio"
          :aria-checked="selectedMode === option.value"
          :disabled="busyKey !== ''"
          @click="selectedMode = option.value"
        >
          <span class="chub-mode-cursor" aria-hidden="true">●</span>
          <span class="chub-mode-body">
            <strong>{{ option.label }}</strong>
            <small>{{ option.hint }}</small>
          </span>
        </button>
      </div>
    </div>

    <el-collapse v-if="!unreachable" class="chub-advanced">
      <el-collapse-item
        name="perGame"
        :title="t.controllersHub.emulation.perAppTitle"
      >
        <p class="chub-hint">{{ t.controllersHub.emulation.perAppHint }}</p>
        <el-alert
          v-if="appsError"
          class="chub-notice"
          type="error"
          :title="t.controllersHub.emulation.appLoadFailed"
          show-icon
          :closable="false"
        >
          <el-button size="small" :loading="appsLoading" @click="loadApps">
            {{ t.controllersHub.emulation.retry }}
          </el-button>
        </el-alert>
        <div v-else v-loading="appsLoading" class="chub-app-list">
          <div v-for="(app, index) in apps" :key="app.name + index" class="chub-app-row">
            <span class="chub-app-name">{{ app.name }}</span>
            <el-select
              :model-value="app.gamepad || ''"
              size="small"
              class="chub-app-select"
              :disabled="appSavingIndex === index"
              @change="(value) => saveAppGamepad(index, value)"
            >
              <el-option
                v-for="opt in appModeOptions"
                :key="opt.value"
                :value="opt.value"
                :label="opt.label"
              />
            </el-select>
          </div>
          <p v-if="!appsLoading && apps.length === 0" class="chub-hint">
            {{ t.controllersHub.emulation.noApps }}
          </p>
        </div>
      </el-collapse-item>
    </el-collapse>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { controllerHub } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'

const { t } = useI18n()
const emit = defineEmits(['mode-change'])

const APPS_TIMEOUT_MS = 8000

const config = ref({ gamepad: 'auto' })
const loading = ref(false)
const unreachable = ref(false)

const apps = ref([])
const appsLoading = ref(false)
const appsError = ref(false)
const appSavingIndex = ref(-1)
let proxyUrl = ''
let appsRequestId = 0

const busyKey = ref('')

const modeOptions = computed(() => {
  const e = t.value.controllersHub.emulation
  return [
    { value: 'auto', label: e.familyAuto, hint: e.familyAutoHint },
    { value: 'x360', label: e.xbox360, hint: e.xbox360Hint },
    { value: 'ds4', label: 'DualShock 4', hint: e.ds4ModeHint },
    { value: 'ds5', label: e.ds5Variant, hint: e.ds5ModeHint },
  ]
})

const selectedMode = computed({
  get: () => config.value.gamepad,
  set: (mode) => saveKey('gamepad', mode),
})

const appModeOptions = computed(() => {
  const e = t.value.controllersHub.emulation
  return [
    { value: '', label: e.followGlobal },
    { value: 'auto', label: e.familyAuto },
    { value: 'x360', label: e.xbox360 },
    { value: 'ds4', label: 'DualShock 4' },
    { value: 'ds5', label: e.ds5Variant },
  ]
})

async function loadConfig() {
  loading.value = true
  unreachable.value = false
  try {
    const result = await controllerHub.getConfig()
    if (result?.success) {
      config.value.gamepad = result.data.gamepad
      emit('mode-change', config.value.gamepad)
    } else {
      unreachable.value = true
    }
  } catch {
    unreachable.value = true
  } finally {
    loading.value = false
  }
}

async function saveKey(key, value) {
  if (busyKey.value !== '' || config.value[key] === value) return
  const prev = config.value[key]
  config.value[key] = value
  busyKey.value = key
  try {
    const result = await controllerHub.saveConfig({ [key]: value })
    if (result?.success) {
      emit('mode-change', value)
      ElMessage.success(result.data)
    } else {
      config.value[key] = prev
      ElMessage.error(result?.message || t.value.controllersHub.emulation.saveFailed)
    }
  } catch (error) {
    config.value[key] = prev
    ElMessage.error(String(error))
  } finally {
    busyKey.value = ''
  }
}

async function loadApps() {
  const requestId = ++appsRequestId
  appsLoading.value = true
  appsError.value = false
  let timeoutTimer = null
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    proxyUrl = await invoke('wait_for_proxy_ready')
    if (requestId !== appsRequestId) return

    const controller = new AbortController()
    timeoutTimer = setTimeout(() => controller.abort('timeout'), APPS_TIMEOUT_MS)
    const resp = await fetch(`${proxyUrl}/api/apps`, { signal: controller.signal })
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)

    const data = await resp.json()
    if (requestId !== appsRequestId) return
    apps.value = Array.isArray(data) ? data : data?.apps || []
  } catch (error) {
    if (requestId !== appsRequestId) return
    console.error('加载应用列表失败:', error)
    appsError.value = true
  } finally {
    if (timeoutTimer !== null) clearTimeout(timeoutTimer)
    if (requestId === appsRequestId) appsLoading.value = false
  }
}

async function saveAppGamepad(index, value) {
  const app = apps.value[index]
  if (!app || appSavingIndex.value !== -1) return
  const prev = app.gamepad || ''
  app.gamepad = value
  appSavingIndex.value = index
  try {
    const resp = await fetch(`${proxyUrl}/api/apps`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ apps: apps.value, editApp: { index, ...app } }),
    })
    if (resp.status === 409) {
      app.gamepad = prev
      ElMessage.error(t.value.controllersHub.emulation.appSaveConflict)
      await loadApps()
      return
    }
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
    ElMessage.success(t.value.controllersHub.emulation.appSaved)
  } catch {
    app.gamepad = prev
    ElMessage.error(t.value.controllersHub.emulation.appSaveFailed)
  } finally {
    appSavingIndex.value = -1
  }
}

defineExpose({ refresh: loadConfig })

onMounted(() => {
  loadConfig()
  loadApps()
})
</script>
