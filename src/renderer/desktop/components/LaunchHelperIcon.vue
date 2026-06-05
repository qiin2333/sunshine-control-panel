<template>
  <span class="launch-helper-icon" :style="{ '--helper-icon-size': `${size}px` }">
    <component :is="iconComponent" />
  </span>
</template>

<script setup>
import { computed } from 'vue'
import { Aim, Connection, Document, Key, Setting, Tools } from '@element-plus/icons-vue'

const props = defineProps({
  templateId: { type: String, default: '' },
  size: { type: Number, default: 18 },
})

const helperIconMap = {
  'controller-mapper': Key,
  'locale-emulator': Connection,
  translator: Document,
  'game-trainer': Tools,
  'rtss-fps-limiter': Aim,
  custom: Setting,
}

const iconComponent = computed(() => helperIconMap[props.templateId] || Setting)
</script>

<style lang="less" scoped>
.launch-helper-icon {
  width: var(--helper-icon-size);
  height: var(--helper-icon-size);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: currentColor;

  :deep(svg) {
    width: 100%;
    height: 100%;
  }
}
</style>
