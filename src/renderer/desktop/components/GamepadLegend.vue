<template>
  <span class="gamepad-legend" aria-hidden="true">
    <span v-for="hint in hints" :key="hint.button" class="legend-item">
      <span class="legend-button" :class="'btn-' + hint.tone">{{ hint.button }}</span>
      <span class="legend-label">{{ hint.label }}</span>
    </span>
  </span>
</template>

<script setup>
import { computed } from 'vue'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const props = defineProps({
  cursorMode: { type: Boolean, default: false },
})

/** 手柄映射不写在文档里没人会发现，所以直接贴在界面底部。 */
const hints = computed(() => {
  const legend = t.value.gamepadLegend
  if (props.cursorMode) {
    return [
      { button: 'A', tone: 'a', label: legend.click },
      { button: 'Y', tone: 'y', label: legend.menu },
      { button: 'B', tone: 'b', label: legend.exitCursor },
      { button: 'L3', tone: 'neutral', label: legend.cursorOff },
    ]
  }
  return [
    { button: 'A', tone: 'a', label: legend.confirm },
    { button: 'B', tone: 'b', label: legend.back },
    { button: 'X', tone: 'x', label: legend.favorite },
    { button: 'Y', tone: 'y', label: legend.menu },
    { button: 'LB/RB', tone: 'neutral', label: legend.switchPage },
    { button: 'LT/RT', tone: 'neutral', label: legend.switchFilter },
    { button: '☰', tone: 'neutral', label: legend.search },
    { button: 'L3', tone: 'neutral', label: legend.cursorOn },
  ]
})
</script>

<style lang="less" scoped>
.gamepad-legend {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
  justify-content: center;
  padding: 6px 14px;
  border-radius: 10px;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.72);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
  pointer-events: none;
}

.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.legend-button {
  min-width: 20px;
  height: 20px;
  padding: 0 5px;
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.12);
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.8);

  &.btn-a { background: rgba(0, 200, 100, 0.28); color: #7dffb0; }
  &.btn-b { background: rgba(220, 60, 60, 0.28); color: #ff9b9b; }
  &.btn-x { background: rgba(60, 120, 230, 0.28); color: #a3c4ff; }
  &.btn-y { background: rgba(220, 190, 40, 0.28); color: #ffe57d; }
}

.legend-label {
  font-size: 11px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.45);
  white-space: nowrap;
}
</style>
