<template>
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
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChatDotRound } from '@element-plus/icons-vue'
import { useDesktopPet } from '../../../composables/useDesktopPet.js'
import { useI18n } from '../../i18n/index.js'
import SettingsCard from './SettingsCard.vue'
import SettingsRow from './SettingsRow.vue'
import SettingsSelect from './SettingsSelect.vue'
import SettingsSwitch from './SettingsSwitch.vue'

const { t } = useI18n()

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

const petIntervalOptions = computed(() => [
  { value: 15, label: t.value.settings.intervals.s15 },
  { value: 30, label: t.value.settings.intervals.s30 },
  { value: 60, label: t.value.settings.intervals.s60 },
  { value: 120, label: t.value.settings.intervals.m2 },
  { value: 300, label: t.value.settings.intervals.m5 },
])

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
</script>
