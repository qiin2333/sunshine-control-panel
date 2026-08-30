<template>
  <section class="chub-panel">
    <div class="chub-window">
      <span class="chub-window-tab">USB PASSTHROUGH</span>
      <div class="chub-hud-row">
        <div class="chub-hud-state"><span class="chub-status-dot"></span><strong>{{ t.deviceHub.usb.title }}</strong></div>
        <el-tag size="small" type="info" effect="plain">{{ t.deviceHub.planned }}</el-tag>
      </div>
      <div class="chub-soon"><strong>{{ t.deviceHub.usb.notAvailable }}</strong><p>{{ t.deviceHub.usb.hint }}</p></div>
      <div class="chub-runtime-grid">
        <div><span>{{ t.deviceHub.usb.direction }}</span><strong>{{ t.deviceHub.usb.clientToHost }}</strong></div>
        <div><span>{{ t.deviceHub.usb.ownership }}</span><strong>{{ t.deviceHub.usb.sessionBound }}</strong></div>
        <div><span>{{ t.deviceHub.usb.release }}</span><strong>{{ t.deviceHub.usb.autoRelease }}</strong></div>
      </div>
      <div class="chub-context-note">
        <strong>{{ t.deviceHub.usb.boundaryTitle }}</strong>
        <span>{{ t.deviceHub.usb.boundaryHint }}</span>
      </div>
    </div>
  </section>
</template>

<script setup>
import { useI18n } from '../../desktop/i18n/index.js'
const { t } = useI18n()
</script>
