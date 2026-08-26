<template>
  <div class="settings-view">
    <div class="page-header fade-in">
      <h1 class="page-title">{{ t.settings.pageTitle }}</h1>
      <p class="page-subtitle">{{ t.settings.pageSubtitle }}</p>
    </div>

    <SettingsAppearanceCard @open-theme-editor="emit('openThemeEditor')" />

    <SettingsToggleSection
      :title="t.settings.startup"
      :icon="Promotion"
      :items="startupSettings"
      :values="settings"
      @update-value="setSettingValue"
    />

    <SettingsToggleSection
      :title="t.settings.bigScreen"
      :icon="Monitor"
      :items="bigScreenItems"
      :values="bigScreenSettings"
      @update-value="setBigScreenSetting"
    />

    <SettingsLaunchAssistantCard :has-tauri="hasTauri" />

    <SettingsFileSharingCard />

    <SettingsToggleSection
      :title="t.settings.notifications"
      :icon="Bell"
      :items="notificationSettings"
      :values="settings"
      @update-value="setSettingValue"
    />

    <SettingsAdvancedCard
      :values="settings"
      @update-value="setSettingValue"
    />

    <SettingsPetCard />

    <SettingsAboutCard
      :app-version="appVersion"
      :checking="checking"
      @open-link="openLink"
      @check-update="checkUpdate"
    />

    <SettingsNotice :notice="statusNotice" />

    <SettingsActions @reset="resetSettings" @save="saveSettings" />
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted } from 'vue'
import { Bell, Monitor, Promotion } from '@element-plus/icons-vue'
import { useSettingsState } from '../composables/useSettingsState'
import { bigScreenSettings, setBigScreenSetting } from '../composables/useBigScreenSettings.js'
import { useI18n } from '../i18n/index.js'
import SettingsActions from '../components/settings/SettingsActions.vue'
import SettingsAboutCard from '../components/settings/SettingsAboutCard.vue'
import SettingsAdvancedCard from '../components/settings/SettingsAdvancedCard.vue'
import SettingsAppearanceCard from '../components/settings/SettingsAppearanceCard.vue'
import SettingsFileSharingCard from '../components/settings/SettingsFileSharingCard.vue'
import SettingsLaunchAssistantCard from '../components/settings/SettingsLaunchAssistantCard.vue'
import SettingsNotice from '../components/settings/SettingsNotice.vue'
import SettingsPetCard from '../components/settings/SettingsPetCard.vue'
import SettingsToggleSection from '../components/settings/SettingsToggleSection.vue'

const { t } = useI18n()

const {
  appVersion,
  checking,
  hasTauri,
  settings,
  statusNotice,
  checkUpdate,
  disposeSettingsState,
  initializeSettingsState,
  openLink,
  resetSettings,
  saveSettings,
} = useSettingsState(t)

const startupSettings = computed(() => [
  {
    key: 'autoStart',
    name: t.value.settings.autoStart,
    description: t.value.settings.autoStartDesc,
  },
  {
    key: 'startMinimized',
    name: t.value.settings.startMinimized,
    description: t.value.settings.startMinimizedDesc,
  },
  {
    key: 'autoStartSunshine',
    name: t.value.settings.autoStartSunshine,
    description: t.value.settings.autoStartSunshineDesc,
  },
])

const bigScreenItems = computed(() => [
  {
    key: 'autoYieldOnLaunch',
    name: t.value.settings.autoYield,
    description: t.value.settings.autoYieldDesc,
  },
  {
    key: 'gamepadCursorEnabled',
    name: t.value.settings.gamepadCursor,
    description: t.value.settings.gamepadCursorDesc,
  },
  {
    key: 'oskAutoOpen',
    name: t.value.settings.oskAutoOpen,
    description: t.value.settings.oskAutoOpenDesc,
  },
  {
    key: 'navSounds',
    name: t.value.settings.navSounds,
    description: t.value.settings.navSoundsDesc,
  },
])

const notificationSettings = computed(() => [  {
    key: 'notifications',
    name: t.value.settings.desktopNotifications,
    description: t.value.settings.desktopNotificationsDesc,
  },
  {
    key: 'connectionNotify',
    name: t.value.settings.connectionNotify,
    description: t.value.settings.connectionNotifyDesc,
    disabled: !settings.value.notifications,
  },
  {
    key: 'updateNotify',
    name: t.value.settings.updateNotify,
    description: t.value.settings.updateNotifyDesc,
    disabled: !settings.value.notifications,
  },
])

function setSettingValue(key, value) {
  settings.value = {
    ...settings.value,
    [key]: value,
  }
}

const emit = defineEmits(['openThemeEditor'])

onMounted(async () => {
  await initializeSettingsState()
})

onUnmounted(() => {
  disposeSettingsState()
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

</style>

