<template>
  <SettingsCard :title="title" :icon="icon">
    <SettingsRow
      v-for="item in items"
      :key="item.key"
      :name="item.name"
      :description="item.description"
    >
      <SettingsSwitch
        :model-value="Boolean(values[item.key])"
        @update:model-value="emit('update-value', item.key, $event)"
      />
    </SettingsRow>
  </SettingsCard>
</template>

<script setup>
import SettingsCard from './SettingsCard.vue'
import SettingsRow from './SettingsRow.vue'
import SettingsSwitch from './SettingsSwitch.vue'

defineProps({
  title: {
    type: String,
    required: true,
  },
  icon: {
    type: [Object, Function, String],
    required: true,
  },
  items: {
    type: Array,
    required: true,
  },
  values: {
    type: Object,
    required: true,
  },
})

const emit = defineEmits(['update-value'])
</script>
