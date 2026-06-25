<template>
  <SettingsCard :title="t.settings.appearance" :icon="Brush">
    <SettingsRow
      :name="t.settings.themeEditorName"
      :description="t.settings.appearanceDesc"
    >
      <button class="desktop-btn" @click="emit('open-theme-editor')">
        {{ t.settings.themeEditor }}
      </button>
    </SettingsRow>
  </SettingsCard>
</template>

<script setup>
import { Brush } from '@element-plus/icons-vue'
import { useI18n } from '../../i18n/index.js'
import SettingsCard from './SettingsCard.vue'
import SettingsRow from './SettingsRow.vue'

const emit = defineEmits(['open-theme-editor'])
const { t } = useI18n()
</script>
