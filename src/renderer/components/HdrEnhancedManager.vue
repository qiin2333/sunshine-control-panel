<template>
  <section class="ds5-page">
    <header class="ds5-page-header">
      <h1>{{ t.sidebar.hdrEnhanced }}</h1>
    </header>
    <EnhancementManager />
    <EnhancementManager kind="nr" />
  </section>
</template>

<script setup>
import { useI18n } from '../desktop/i18n/index.js'
import EnhancementManager from './EnhancementManager.vue'

const { t } = useI18n()
</script>

<style scoped lang="less">
@import '../styles/DualSenseSettings.less';
</style>
