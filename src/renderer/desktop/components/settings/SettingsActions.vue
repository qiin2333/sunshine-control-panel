<template>
  <div class="actions-bar fade-in">
    <button class="desktop-btn" @click="emit('reset')">{{ t.settings.resetDefaults }}</button>
    <button class="desktop-btn primary" @click="emit('save')">{{ t.settings.saveSettings }}</button>
  </div>
</template>

<script setup>
import { useI18n } from '../../i18n/index.js'

const emit = defineEmits(['reset', 'save'])
const { t } = useI18n()
</script>

<style lang="less" scoped>
.actions-bar {
  display: flex;
  justify-content: flex-end;
  gap: 16px;
  margin-top: 32px;
  padding-top: 24px;
  border-top: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
}
</style>
