<template>
  <SettingsCard :title="t.settings.advanced" :icon="Setting">
    <SettingsRow :name="t.settings.devMode" :description="t.settings.devModeDesc">
      <SettingsSwitch
        :model-value="Boolean(values.devMode)"
        @update:model-value="emit('update-value', 'devMode', $event)"
      />
    </SettingsRow>

    <SettingsRow :name="t.settings.logLevel" :description="t.settings.logLevelDesc">
      <SettingsSelect
        :model-value="values.logLevel"
        :options="logLevelOptions"
        @update:model-value="emit('update-value', 'logLevel', $event)"
      />
    </SettingsRow>
  </SettingsCard>
</template>

<script setup>
import { computed } from 'vue'
import { Setting } from '@element-plus/icons-vue'
import { useI18n } from '../../i18n/index.js'
import SettingsCard from './SettingsCard.vue'
import SettingsRow from './SettingsRow.vue'
import SettingsSelect from './SettingsSelect.vue'
import SettingsSwitch from './SettingsSwitch.vue'

defineProps({
  values: {
    type: Object,
    required: true,
  },
})

const emit = defineEmits(['update-value'])
const { t } = useI18n()

const logLevelOptions = computed(() => [
  { value: 'error', label: t.value.settings.logLevels.error },
  { value: 'warn', label: t.value.settings.logLevels.warn },
  { value: 'info', label: t.value.settings.logLevels.info },
  { value: 'debug', label: t.value.settings.logLevels.debug },
])
</script>
