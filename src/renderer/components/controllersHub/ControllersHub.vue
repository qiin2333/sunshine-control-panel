<template>
  <section class="chub-page">
    <header class="chub-header">
      <span class="chub-header-icon" aria-hidden="true"><IconGamepad /></span>
      <div class="chub-header-copy">
        <h1>DEVICE HUB</h1>
        <p class="chub-intro">{{ t.deviceHub.intro }}</p>
      </div>
    </header>

    <div class="chub-tabs"><ChubTabs v-model="activeTab" :options="tabs" /></div>

    <div class="chub-pane">
      <DeviceOverviewPanel v-if="activeTab === 'overview'" @navigate="activeTab = $event" />
      <template v-else-if="activeTab === 'controllers'">
        <GamepadTypePicker @ds5-selection-change="ds5Selected = $event" />
        <DualSenseSettings
          v-if="ds5Selected"
          embedded
          @open-controller-meta="emit('open-controller-meta')"
          @manage-components="activeTab = 'components'"
        />
        <div class="chub-advanced-block">
          <el-collapse v-model="controllerAdvancedOpen" class="chub-advanced">
            <el-collapse-item name="advanced" :title="t.deviceHub.controllerAdvanced">
              <AdvancedControllerOptions />
            </el-collapse-item>
          </el-collapse>
        </div>
      </template>
      <MicrophonePanel v-else-if="activeTab === 'microphone'" @manage-components="activeTab = 'components'" />
      <UsbPassthroughPanel v-else-if="activeTab === 'usb'" />
      <RuntimeComponentsPanel v-else-if="activeTab === 'components'" />
      <template v-else-if="activeTab === 'tools'">
        <DeviceDiagnosticsPanel
          @open-controller-meta="emit('open-controller-meta')"
          @open-stylus-input-probe="emit('open-stylus-input-probe')"
        />
      </template>
    </div>
  </section>
</template>

<script setup>
import { computed, ref } from 'vue'
import IconGamepad from '../../desktop/icons/IconGamepad.vue'
import DualSenseSettings from '../DualSenseSettings.vue'
import AdvancedControllerOptions from './AdvancedControllerOptions.vue'
import ChubTabs from './ChubTabs.vue'
import DeviceOverviewPanel from './DeviceOverviewPanel.vue'
import GamepadTypePicker from './GamepadTypePicker.vue'
import MicrophonePanel from './MicrophonePanel.vue'
import DeviceDiagnosticsPanel from './DeviceDiagnosticsPanel.vue'
import RuntimeComponentsPanel from './RuntimeComponentsPanel.vue'
import UsbPassthroughPanel from './UsbPassthroughPanel.vue'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['open-controller-meta', 'open-stylus-input-probe'])
const { t } = useI18n()
const activeTab = ref('overview')
const ds5Selected = ref(false)
const controllerAdvancedOpen = ref([])
const tabs = computed(() => [
  { value: 'overview', label: t.value.deviceHub.tabs.overview },
  { value: 'controllers', label: t.value.deviceHub.tabs.controllers },
  { value: 'microphone', label: t.value.deviceHub.tabs.microphone },
  { value: 'usb', label: t.value.deviceHub.tabs.usb },
  { value: 'components', label: t.value.deviceHub.tabs.components },
  { value: 'tools', label: t.value.deviceHub.tabs.tools },
])
</script>

<style lang="less">
@import '../../styles/ControllersHub.less';
</style>
