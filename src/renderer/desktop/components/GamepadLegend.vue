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
import { viewActions } from '../composables/useGamepadActions.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const props = defineProps({
  cursorMode: { type: Boolean, default: false },
})

/** 动作 id → [按键文本, 色调]。顺序即显示顺序。 */
const ACTION_CHIPS = {
  confirm: ['A', 'a'],
  back: ['B', 'b'],
  search: ['Y', 'y'],
  favorite: ['X', 'x'],
  menu: ['☰', 'neutral'],
  scroll: ['LT/RT', 'neutral'],
  pages: ['LB/RB', 'neutral'],
  cursor: ['L3', 'neutral'],
}

/**
 * 只显示当前上下文真的会响应的按键：
 * 全局动作（确认/返回/换页/滚动/光标）始终在，视图级动作（搜索/收藏/菜单）
 * 由各视图通过 useGamepadActions 声明——没声明的页面就不显示，不撒谎。
 */
const hints = computed(() => {
  const legend = t.value.gamepadLegend
  if (props.cursorMode) {
    return [
      { button: 'A', tone: 'a', label: legend.click },
      { button: 'B', tone: 'b', label: legend.exitCursor },
      { button: 'LT/RT', tone: 'neutral', label: legend.scroll },
    ]
  }

  const actions = ['confirm', 'back']
  if (viewActions.value.has('search')) actions.push('search')
  if (viewActions.value.has('favorite')) actions.push('favorite')
  if (viewActions.value.has('menu')) actions.push('menu')
  actions.push('scroll', 'pages', 'cursor')

  const labels = {
    confirm: legend.confirm,
    back: legend.back,
    search: legend.search,
    favorite: legend.favorite,
    menu: legend.menu,
    scroll: legend.scroll,
    pages: legend.switchPage,
    cursor: legend.cursorOn,
  }

  return actions.map((id) => {
    const [button, tone] = ACTION_CHIPS[id]
    return { button, tone, label: labels[id] }
  })
})
</script>

<style lang="less" scoped>
.gamepad-legend {
  display: flex;
  align-items: center;
  gap: 14px;
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
