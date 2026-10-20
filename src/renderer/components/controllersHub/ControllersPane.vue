<template>
  <GamepadTypePicker @ds5-selection-change="ds5Selected = $event" />
  <DualSenseSettings
    v-if="ds5Selected"
    @open-controller-meta="emit('open-controller-meta')"
    @manage-components="emit('manage-components')"
  />
  <div class="chub-advanced-block">
    <el-collapse v-model="controllerAdvancedOpen" class="chub-advanced">
      <el-collapse-item name="advanced" :title="t.deviceHub.controllerAdvanced">
        <AdvancedControllerOptions />
      </el-collapse-item>
    </el-collapse>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import DualSenseSettings from '../DualSenseSettings.vue'
import AdvancedControllerOptions from './AdvancedControllerOptions.vue'
import GamepadTypePicker from './GamepadTypePicker.vue'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['open-controller-meta', 'manage-components'])
const { t } = useI18n()
const ds5Selected = ref(false)
const controllerAdvancedOpen = ref([])
</script>
