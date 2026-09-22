<template>
  <section class="chub-panel chub-runtime-components">
    <p class="chub-hint">{{ t.deviceHub.components.managementHint }}</p>
    <DualSenseComponentPanel :component="component" :description="t.deviceHub.components.hostHint" />
    <UsbTransportPanel :transport="transport" maintenance />
    <InputDriversPanel />
  </section>
</template>

<script setup>
import InputDriversPanel from './InputDriversPanel.vue'
import DualSenseComponentPanel from './DualSenseComponentPanel.vue'
import { useDualSenseComponent } from '../../composables/useDualSenseComponent.js'
import UsbTransportPanel from './UsbTransportPanel.vue'
import { useUsbTransport } from '../../composables/useUsbTransport.js'
import { useI18n } from '../../desktop/i18n/index.js'
const { t } = useI18n()
const component = useDualSenseComponent()
const transport = useUsbTransport()
</script>
