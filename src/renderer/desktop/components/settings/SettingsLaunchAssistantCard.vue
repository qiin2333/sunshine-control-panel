<template>
  <SettingsCard :title="t.settings.launchAssistant" :icon="Lightning">
    <p class="section-desc">
      {{ t.settings.launchAssistantDesc }}
    </p>

    <SettingsToolPathRow
      v-for="template in helperTemplates"
      :key="template.id"
      :helper="template"
      :has-tauri="hasTauri"
      :get-path="getGlobalToolPath"
      @update-path="setGlobalToolPath"
      @browse="browseToolPath"
    />
  </SettingsCard>
</template>

<script setup>
import { computed } from 'vue'
import { Lightning } from '@element-plus/icons-vue'
import { useLaunchHelpers } from '../../composables/useLaunchHelpers'
import { useI18n } from '../../i18n/index.js'
import SettingsCard from './SettingsCard.vue'
import SettingsToolPathRow from './SettingsToolPathRow.vue'

defineProps({
  hasTauri: {
    type: Boolean,
    default: false,
  },
})

const { t } = useI18n()

const {
  templates: allTemplates,
  getGlobalPath: getGlobalToolPath,
  setGlobalPath: setGlobalToolPath,
} = useLaunchHelpers(t)

const helperTemplates = computed(() =>
  allTemplates.value.filter(template => template.id !== 'custom')
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
  } catch (error) {
    console.warn('File dialog not available:', error)
  }
}
</script>

<style lang="less" scoped>
.section-desc {
  font-size: 14px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  margin: 0 0 16px 0;
}
</style>
