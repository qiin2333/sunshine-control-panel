<template>
  <div class="settings-view">
    <div class="page-header fade-in">
      <h1 class="page-title">{{ t.settings.pageTitle }}</h1>
      <p class="page-subtitle">{{ t.settings.pageSubtitle }}</p>
    </div>

    <!-- Appearance settings -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon"><Brush /></span>
          {{ t.settings.appearance }}
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.themeEditorName }}</div>
            <div class="setting-desc">{{ t.settings.appearanceDesc }}</div>
          </div>
          <div class="setting-control">
            <button class="desktop-btn" @click="$emit('openThemeEditor')">{{ t.settings.themeEditor }}</button>
          </div>
        </div>
      </div>
    </div>

    <!-- Startup settings -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon"><Promotion /></span>
          {{ t.settings.startup }}
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.autoStart }}</div>
            <div class="setting-desc">{{ t.settings.autoStartDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.autoStart" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.startMinimized }}</div>
            <div class="setting-desc">{{ t.settings.startMinimizedDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.startMinimized" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.autoStartSunshine }}</div>
            <div class="setting-desc">{{ t.settings.autoStartSunshineDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.autoStartSunshine" />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <!-- Launch assistant global tool paths -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon"><Lightning /></span>
          {{ t.settings.launchAssistant }}
        </div>
      </div>
      <div class="card-content">
        <p class="section-desc">
          {{ t.settings.launchAssistantDesc }}
        </p>
        <div
          v-for="tmpl in helperTemplates"
          :key="tmpl.id"
          class="setting-item tool-path-item"
        >
          <div class="setting-info">
            <div class="setting-name tool-path-name">
              <LaunchHelperIcon :template-id="tmpl.id" :size="18" />
              <span>{{ tmpl.name }}</span>
            </div>
            <div class="setting-desc">{{ tmpl.description }}</div>
          </div>
          <div class="setting-control tool-path-control">
            <div
              v-for="param in tmpl.params.filter(p => p.key === 'path')"
              :key="param.key"
              class="tool-path-row"
            >
              <input
                type="text"
                class="path-input"
                :placeholder="param.placeholder"
                :value="getGlobalToolPath(tmpl.id, param.key)"
                @input="setGlobalToolPath(tmpl.id, param.key, $event.target.value)"
              />
              <button
                v-if="hasTauri"
                class="browse-btn-small"
                @click="browseToolPath(tmpl.id, param.key)"
              ><FolderOpened /></button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Notification settings -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon"><Bell /></span>
          {{ t.settings.notifications }}
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.desktopNotifications }}</div>
            <div class="setting-desc">{{ t.settings.desktopNotificationsDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.notifications" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.connectionNotify }}</div>
            <div class="setting-desc">{{ t.settings.connectionNotifyDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.connectionNotify" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.updateNotify }}</div>
            <div class="setting-desc">{{ t.settings.updateNotifyDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.updateNotify" />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <!-- Advanced settings -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon"><Setting /></span>
          {{ t.settings.advanced }}
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.devMode }}</div>
            <div class="setting-desc">{{ t.settings.devModeDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="settings.devMode" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.logLevel }}</div>
            <div class="setting-desc">{{ t.settings.logLevelDesc }}</div>
          </div>
          <div class="setting-control">
            <select v-model="settings.logLevel" class="select-control">
              <option value="error">{{ t.settings.logLevels.error }}</option>
              <option value="warn">{{ t.settings.logLevels.warn }}</option>
              <option value="info">{{ t.settings.logLevels.info }}</option>
              <option value="debug">{{ t.settings.logLevels.debug }}</option>
            </select>
          </div>
        </div>
      </div>
    </div>

    <!-- Desktop pet -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon"><ChatDotRound /></span>
          {{ t.settings.pet }}
        </div>
      </div>
      <div class="card-content">
        <div class="setting-item">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.deskObserve }}</div>
            <div class="setting-desc">{{ t.settings.deskObserveDesc }}</div>
          </div>
          <div class="setting-control">
            <label class="switch">
              <input type="checkbox" v-model="petEnabled" @change="onPetToggle" />
              <span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="setting-item" v-if="petEnabled">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.observeInterval }}</div>
            <div class="setting-desc">{{ t.settings.observeIntervalDesc }}</div>
          </div>
          <div class="setting-control">
            <select v-model="petIntervalSec" class="select-control" @change="onPetIntervalChange">
              <option :value="15">{{ t.settings.intervals.s15 }}</option>
              <option :value="30">{{ t.settings.intervals.s30 }}</option>
              <option :value="60">{{ t.settings.intervals.s60 }}</option>
              <option :value="120">{{ t.settings.intervals.m2 }}</option>
              <option :value="300">{{ t.settings.intervals.m5 }}</option>
            </select>
          </div>
        </div>

        <div class="setting-item" v-if="petEnabled">
          <div class="setting-info">
            <div class="setting-name">{{ t.settings.pokeMita }}</div>
            <div class="setting-desc">{{ t.settings.pokeMitaDesc }}</div>
          </div>
          <div class="setting-control">
            <button class="desktop-btn" :disabled="isObserving" @click="poke">
              {{ isObserving ? t.settings.pokeBtnObserving : t.settings.pokeBtn }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- About -->
    <div class="desktop-card about-card fade-in">
      <div class="about-content">
        <div class="about-logo"><Sunny /></div>
        <div class="about-info">
          <div class="about-name">Foundation Desktop</div>
          <div class="about-version">{{ t.settings.version }} {{ appVersion }}</div>
          <div class="about-links">
            <a href="#" @click.prevent="openLink('github')">GitHub</a>
            <span>•</span>
            <a href="#" @click.prevent="openLink('docs')">{{ t.settings.docs }}</a>
            <span>•</span>
            <a href="#" @click.prevent="openLink('discord')">Discord</a>
          </div>
        </div>
      </div>
      <button class="desktop-btn" :disabled="checking" @click="checkUpdate">
        {{ checking ? t.settings.checking : t.settings.checkUpdate }}
      </button>
    </div>

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
import { Bell, Brush, ChatDotRound, FolderOpened, Lightning, Promotion, Setting, Sunny } from '@element-plus/icons-vue'
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
import LaunchHelperIcon from '../components/LaunchHelperIcon.vue'

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

function onPetToggle() {
  if (petEnabled.value) {
    startObserving()
  } else {
    stopObserving()
  }
}

function onPetIntervalChange() {
  setIntervalSeconds(petIntervalSec.value)
}

const {
  templates: allTemplates,
  getGlobalPath: getGlobalToolPath,
  setGlobalPath: setGlobalToolPath,
} = useLaunchHelpers(t)

const helperTemplates = computed(() =>
  allTemplates.value.filter(t => t.id !== 'custom')
)

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

.desktop-card {
  margin-bottom: 22px;
}

.setting-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 0;
  border-bottom: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);

  &:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  &:first-child {
    padding-top: 0;
  }

  .setting-info {
    .setting-name {
      font-size: 16px;
      font-weight: 500;
      color: var(--fd-text-primary, #fff);
      margin-bottom: 4px;
    }

    .setting-desc {
      font-size: 14px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
      line-height: 1.45;
    }
  }
}

.tool-path-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.select-control {
  padding: 8px 32px 8px 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2300fff5' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;

  &:focus {
    outline: none;
    border-color: var(--fd-accent, #00fff5);
  }

  option {
    background: var(--fd-bg-secondary, #1a1a2e);
    color: var(--fd-text-primary, #fff);
  }
}

// Switch styles
.switch {
  position: relative;
  display: inline-block;
  width: 48px;
  height: 26px;

  input {
    opacity: 0;
    width: 0;
    height: 0;

    &:checked + .slider {
      background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);

      &::before {
        transform: translateX(22px);
      }
    }
  }

  .slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
    border-radius: 26px;
    transition: 0.3s;

    &::before {
      position: absolute;
      content: "";
      height: 20px;
      width: 20px;
      left: 3px;
      bottom: 3px;
      background: white;
      border-radius: 50%;
      transition: 0.3s;
    }
  }
}

.about-card {
  display: flex;
  align-items: center;
  justify-content: space-between;

  .about-content {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .about-logo {
    width: 42px;
    height: 42px;
    color: var(--fd-accent, #00fff5);
    display: flex;
    align-items: center;
    justify-content: center;

    svg {
      width: 100%;
      height: 100%;
    }
  }

  .about-info {
    .about-name {
      font-size: 18px;
      font-weight: 600;
      color: var(--fd-text-primary, #fff);
    }

    .about-version {
      font-size: 14px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
      margin-bottom: 4px;
    }

    .about-links {
      font-size: 13px;
      display: flex;
      gap: 8px;

      a {
        color: var(--fd-accent, #00fff5);
        text-decoration: none;

        &:hover {
          text-decoration: underline;
        }
      }

      span {
        color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
      }
    }
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

.tool-path-item {
  flex-direction: column;
  align-items: flex-start !important;
  gap: 10px;

  .setting-control {
    width: 100%;
  }
}

.tool-path-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.path-input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 13px;
  font-family: 'Consolas', 'Monaco', monospace;

  &::placeholder {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25);
  }

  &:focus {
    outline: none;
    border-color: var(--fd-accent, #00fff5);
  }
}

.browse-btn-small {
  padding: 7px 10px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  flex-shrink: 0;

  &:hover {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    border-color: var(--fd-accent, #00fff5);
  }
}
</style>

