<template>
  <div class="chub-shell">
    <header class="page-header chub-header">
      <div class="page-header-heading">
        <el-icon class="page-header-icon"><IconGamepad /></el-icon>
        <div class="page-header-title-stack">
          <span class="page-header-kicker">{{ t.deviceHub.eyebrow }}</span>
          <h2 class="page-header-title">{{ t.deviceHub.title }}</h2>
        </div>
      </div>
    </header>
    <section class="chub-page">
    <div class="chub-tabs"><ChubTabs v-model="activeTab" :options="tabs" /></div>

    <div class="chub-pane">
      <DeviceOverviewPanel v-if="activeTab === 'overview'" @navigate="activeTab = $event" />
      <template v-else-if="activeTab === 'controllers'">
        <GamepadTypePicker @mode-change="controllerMode = $event" />
        <DualSenseSettings
          v-if="controllerMode === 'ds5'"
          embedded
          @open-controller-meta="emit('open-controller-meta')"
        />
        <div class="chub-advanced-block">
          <el-collapse v-model="controllerAdvancedOpen" class="chub-advanced">
            <el-collapse-item name="advanced" :title="t.deviceHub.controllerAdvanced">
              <AdvancedControllerOptions />
            </el-collapse-item>
          </el-collapse>
        </div>
      </template>
      <MicrophonePanel v-else-if="activeTab === 'microphone'" />
      <UsbPassthroughPanel v-else-if="activeTab === 'usb'" />
      <template v-else-if="activeTab === 'components'">
        <RuntimeComponentsPanel />
        <PeripheralToolsPanel
          @open-controller-meta="emit('open-controller-meta')"
          @open-stylus-input-probe="emit('open-stylus-input-probe')"
        />
      </template>
    </div>
    </section>
  </div>
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
import PeripheralToolsPanel from './PeripheralToolsPanel.vue'
import RuntimeComponentsPanel from './RuntimeComponentsPanel.vue'
import UsbPassthroughPanel from './UsbPassthroughPanel.vue'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['open-controller-meta', 'open-stylus-input-probe'])
const { t } = useI18n()
const activeTab = ref('overview')
const controllerMode = ref('auto')
const controllerAdvancedOpen = ref([])
const tabs = computed(() => [
  { value: 'overview', label: t.value.deviceHub.tabs.overview },
  { value: 'controllers', label: t.value.deviceHub.tabs.controllers },
  { value: 'microphone', label: t.value.deviceHub.tabs.microphone },
  { value: 'usb', label: t.value.deviceHub.tabs.usb },
  { value: 'components', label: t.value.deviceHub.tabs.components },
])
</script>

<style lang="less">
@import '../../styles/ControllersHub.less';
</style>
