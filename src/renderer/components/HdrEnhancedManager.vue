<template>
  <section class="ds5-page">
    <header class="ds5-page-header">
      <h1>{{ t.sidebar.hdrEnhanced }}</h1>
      <p class="ds5-intro">{{ t.hdrEnhanced.description }}</p>
    </header>
    <RtxHdrManager />
  </section>
</template>

<script setup>
import { useI18n } from '../desktop/i18n/index.js'
import RtxHdrManager from './RtxHdrManager.vue'

const { t } = useI18n()
</script>

<style scoped lang="less">
@import '../styles/DualSenseSettings.less';
</style>
