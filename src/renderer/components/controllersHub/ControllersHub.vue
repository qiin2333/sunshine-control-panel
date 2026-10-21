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
      <KeepAlive>
        <component :is="activePane.component" v-bind="activePane.props" v-on="activePane.on" />
      </KeepAlive>
    </div>
  </section>
</template>

<script setup>
import { computed, ref } from 'vue'
import IconGamepad from '../../desktop/icons/IconGamepad.vue'
import ChubTabs from './ChubTabs.vue'
import ControllersPane from './ControllersPane.vue'
import DeviceOverviewPanel from './DeviceOverviewPanel.vue'
import DeviceDiagnosticsPanel from './DeviceDiagnosticsPanel.vue'
import MicrophonePanel from './MicrophonePanel.vue'
import RuntimeComponentsPanel from './RuntimeComponentsPanel.vue'
import UsbPassthroughPanel from './UsbPassthroughPanel.vue'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['open-controller-meta', 'open-stylus-input-probe'])
const { t } = useI18n()
const activeTab = ref('overview')

const gotoComponents = () => {
  activeTab.value = 'components'
}

// KeepAlive 缓存各 pane：切换页签不销毁组件，进行中的安装进度与已加载状态得以保留。
const panes = computed(() => ({
  overview: {
    component: DeviceOverviewPanel,
    on: { navigate: (tab) => { activeTab.value = tab } },
  },
  controllers: {
    component: ControllersPane,
    on: {
      'open-controller-meta': () => emit('open-controller-meta'),
      'manage-components': gotoComponents,
    },
  },
  microphone: { component: MicrophonePanel, on: { 'manage-components': gotoComponents } },
  usb: { component: UsbPassthroughPanel, on: {} },
  components: { component: RuntimeComponentsPanel, on: {} },
  tools: {
    component: DeviceDiagnosticsPanel,
    on: {
      'open-controller-meta': () => emit('open-controller-meta'),
      'open-stylus-input-probe': () => emit('open-stylus-input-probe'),
    },
  },
}))

const activePane = computed(() => panes.value[activeTab.value] ?? panes.value.overview)

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
