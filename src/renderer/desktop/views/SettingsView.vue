<template>
  <div class="settings-view">
    <div class="page-header fade-in">
      <h1 class="page-title">{{ t.settings.pageTitle }}</h1>
      <p class="page-subtitle">{{ t.settings.pageSubtitle }}</p>
    </div>

    <SettingsCard :title="t.settings.appearance" :icon="Brush">
      <SettingsRow
        :name="t.settings.themeEditorName"
        :description="t.settings.appearanceDesc"
      >
        <button class="desktop-btn" @click="$emit('openThemeEditor')">
          {{ t.settings.themeEditor }}
        </button>
      </SettingsRow>
    </SettingsCard>

    <SettingsCard :title="t.settings.startup" :icon="Promotion">
      <SettingsRow :name="t.settings.autoStart" :description="t.settings.autoStartDesc">
        <SettingsSwitch v-model="settings.autoStart" />
      </SettingsRow>

      <SettingsRow
        :name="t.settings.startMinimized"
        :description="t.settings.startMinimizedDesc"
      >
        <SettingsSwitch v-model="settings.startMinimized" />
      </SettingsRow>

      <SettingsRow
        :name="t.settings.autoStartSunshine"
        :description="t.settings.autoStartSunshineDesc"
      >
        <SettingsSwitch v-model="settings.autoStartSunshine" />
      </SettingsRow>
    </SettingsCard>

    <SettingsCard :title="t.settings.launchAssistant" :icon="Lightning">
      <p class="section-desc">
        {{ t.settings.launchAssistantDesc }}
      </p>

      <SettingsToolPathRow
        v-for="tmpl in helperTemplates"
        :key="tmpl.id"
        :helper="tmpl"
        :has-tauri="hasTauri"
        :get-path="getGlobalToolPath"
        @update-path="setGlobalToolPath"
        @browse="browseToolPath"
      />
    </SettingsCard>

    <SettingsCard :title="t.settings.notifications" :icon="Bell">
      <SettingsRow
        :name="t.settings.desktopNotifications"
        :description="t.settings.desktopNotificationsDesc"
      >
        <SettingsSwitch v-model="settings.notifications" />
      </SettingsRow>

      <SettingsRow
        :name="t.settings.connectionNotify"
        :description="t.settings.connectionNotifyDesc"
      >
        <SettingsSwitch v-model="settings.connectionNotify" />
      </SettingsRow>

      <SettingsRow :name="t.settings.updateNotify" :description="t.settings.updateNotifyDesc">
        <SettingsSwitch v-model="settings.updateNotify" />
      </SettingsRow>
    </SettingsCard>

    <SettingsCard :title="t.settings.advanced" :icon="Setting">
      <SettingsRow :name="t.settings.devMode" :description="t.settings.devModeDesc">
        <SettingsSwitch v-model="settings.devMode" />
      </SettingsRow>

      <SettingsRow :name="t.settings.logLevel" :description="t.settings.logLevelDesc">
        <SettingsSelect v-model="settings.logLevel" :options="logLevelOptions" />
      </SettingsRow>
    </SettingsCard>

    <SettingsCard :title="t.settings.pet" :icon="ChatDotRound">
      <SettingsRow :name="t.settings.deskObserve" :description="t.settings.deskObserveDesc">
        <SettingsSwitch v-model="petEnabled" @change="onPetToggle" />
      </SettingsRow>

      <SettingsRow
        v-if="petEnabled"
        :name="t.settings.observeInterval"
        :description="t.settings.observeIntervalDesc"
      >
        <SettingsSelect
          v-model="petIntervalSec"
          :options="petIntervalOptions"
          @change="onPetIntervalChange"
        />
      </SettingsRow>

      <SettingsRow
        v-if="petEnabled"
        :name="t.settings.pokeMita"
        :description="t.settings.pokeMitaDesc"
      >
        <button class="desktop-btn" :disabled="isObserving" @click="poke">
          {{ isObserving ? t.settings.pokeBtnObserving : t.settings.pokeBtn }}
        </button>
      </SettingsRow>
    </SettingsCard>

    <SettingsAboutCard
      :app-version="appVersion"
      :checking="checking"
      @open-link="openLink"
      @check-update="checkUpdate"
    />

    <Transition name="notice">
      <div
        v-if="statusNotice"
        class="settings-notice fade-in"
        :class="statusNotice.type"
      >
        {{ statusNotice.message }}
      </div>
    </Transition>

    <!-- Actions -->
    <div class="actions-bar fade-in">
      <button class="desktop-btn" @click="resetSettings">{{ t.settings.resetDefaults }}</button>
      <button class="desktop-btn primary" @click="saveSettings">{{ t.settings.saveSettings }}</button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { Bell, Brush, ChatDotRound, Lightning, Promotion, Setting } from '@element-plus/icons-vue'
import { useLaunchHelpers } from '../composables/useLaunchHelpers'
import { isTauriRuntime } from '../composables/useTauri'
import {
  defaultDesktopSettings,
  loadDesktopSettings,
  requestNotificationPermission,
  saveDesktopSettings,
} from '../composables/useDesktopSettings'
import { useDesktopPet } from '../../composables/useDesktopPet.js'
import { useI18n } from '../i18n/index.js'
import SettingsAboutCard from '../components/settings/SettingsAboutCard.vue'
import SettingsCard from '../components/settings/SettingsCard.vue'
import SettingsRow from '../components/settings/SettingsRow.vue'
import SettingsSelect from '../components/settings/SettingsSelect.vue'
import SettingsSwitch from '../components/settings/SettingsSwitch.vue'
import SettingsToolPathRow from '../components/settings/SettingsToolPathRow.vue'

const { t } = useI18n()

const invoke = ref(null)
const hasTauri = ref(false)

// Desktop pet settings
const {
  petEnabled,
  isObserving,
  observeInterval,
  startObserving,
  stopObserving,
  setIntervalSeconds,
  poke,
} = useDesktopPet()

const petIntervalSec = ref(Math.round(observeInterval.value / 1000))

function onPetToggle(nextValue = petEnabled.value) {
  if (nextValue) {
    startObserving()
  } else {
    stopObserving()
  }
}

function onPetIntervalChange(nextValue = petIntervalSec.value) {
  setIntervalSeconds(nextValue)
}

const {
  templates: allTemplates,
  getGlobalPath: getGlobalToolPath,
  setGlobalPath: setGlobalToolPath,
} = useLaunchHelpers(t)

const helperTemplates = computed(() =>
  allTemplates.value.filter(t => t.id !== 'custom')
)

const logLevelOptions = computed(() => [
  { value: 'error', label: t.value.settings.logLevels.error },
  { value: 'warn', label: t.value.settings.logLevels.warn },
  { value: 'info', label: t.value.settings.logLevels.info },
  { value: 'debug', label: t.value.settings.logLevels.debug },
])

const petIntervalOptions = computed(() => [
  { value: 15, label: t.value.settings.intervals.s15 },
  { value: 30, label: t.value.settings.intervals.s30 },
  { value: 60, label: t.value.settings.intervals.s60 },
  { value: 120, label: t.value.settings.intervals.m2 },
  { value: 300, label: t.value.settings.intervals.m5 },
])

async function browseToolPath(templateId, paramKey) {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const path = await open({
      filters: [
        { name: t.value.launchHelper?.executableFiles || 'Executables', extensions: ['exe', 'bat', 'cmd', 'lnk', 'com', 'scr'] },
        { name: t.value.launchHelper?.allFiles || 'All Files', extensions: ['*'] },
      ],
    })
    if (path) {
      setGlobalToolPath(templateId, paramKey, path)
    }
  } catch (e) {
    console.warn('File dialog not available:', e)
  }
}

const settings = ref({ ...defaultDesktopSettings })
const appVersion = ref('0.0.0-dev')

defineEmits(['openThemeEditor'])

const statusNotice = ref(null)
const checking = ref(false)
let statusTimer = null

function showStatus(type, message) {
  clearTimeout(statusTimer)
  statusNotice.value = { type, message }
  statusTimer = setTimeout(() => {
    statusNotice.value = null
  }, 3500)
}

async function loadSettings() {
  try {
    settings.value = await loadDesktopSettings()
  } catch (e) {
    console.error('Failed to load settings:', e)
  }
}

async function resetSettings() {
  settings.value = { ...defaultDesktopSettings }
  try {
    await saveDesktopSettings(settings.value)
    showStatus('info', t.value.settings.resetSuccess)
  } catch (e) {
    console.error('Failed to reset settings:', e)
    showStatus('error', e.message || String(e))
  }
}

async function saveSettings() {
  try {
    await saveDesktopSettings(settings.value)
    await requestNotificationPermission(settings.value)
    showStatus('success', t.value.settings.saveSuccess)
  } catch (e) {
    console.error('Failed to save settings:', e)
    showStatus('error', e.message || String(e))
  }
}

function openLink(type) {
  const urls = {
    github: 'https://github.com/LizardByte/Sunshine',
    docs: 'https://docs.lizardbyte.dev/projects/sunshine/',
    discord: 'https://discord.gg/lizardbyte',
  }
  if (invoke.value) {
    invoke.value('open_external_url', { url: urls[type] }).catch(() => {
      window.open(urls[type], '_blank')
    })
  } else {
    window.open(urls[type], '_blank')
  }
}

async function checkUpdate() {
  if (!invoke.value) {
    showStatus('warning', t.value.settings.updateUnavailable)
    return
  }
  checking.value = true
  try {
    const update = await invoke.value('check_for_updates')
    if (update) {
      showStatus('success', `${t.value.settings.updateFound}${update.version}`)
    } else {
      showStatus('info', t.value.settings.updateLatest)
    }
  } catch (e) {
    showStatus('error', t.value.settings.updateError)
  } finally {
    checking.value = false
  }
}

onMounted(async () => {
  hasTauri.value = await isTauriRuntime()
  if (!hasTauri.value) {
    loadSettings()
    return
  }
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
    const info = await invoke.value('get_system_info').catch(() => null)
    if (info?.app_version) appVersion.value = info.app_version
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }
  await loadSettings()
})

onUnmounted(() => {
  clearTimeout(statusTimer)
})
</script>

<style lang="less" scoped>
.settings-view {
  max-width: 1000px;
  margin: 0 auto;
}

.page-header {
  margin-bottom: 32px;

  .page-title {
    font-size: 36px;
    font-weight: 700;
    color: var(--fd-text-primary, #fff);
    margin: 0 0 10px 0;
  }

  .page-subtitle {
    font-size: 18px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    margin: 0;
  }
}

.actions-bar {
  display: flex;
  justify-content: flex-end;
  gap: 16px;
  margin-top: 32px;
  padding-top: 24px;
  border-top: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
}

.settings-notice {
  display: flex;
  align-items: center;
  min-height: 42px;
  margin: 8px 0 20px;
  padding: 10px 14px;
  border-radius: 10px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.72);
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.82);
  font-size: 13px;

  &.success {
    border-color: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.35);
    color: var(--fd-status-success, #00ff88);
  }

  &.warning,
  &.info {
    border-color: rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.3);
  }

  &.error {
    border-color: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.35);
    color: var(--fd-status-danger, #ff6b35);
  }
}

.notice-enter-active,
.notice-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.notice-enter-from,
.notice-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.section-desc {
  font-size: 14px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  margin: 0 0 16px 0;
}

</style>

