<template>
  <span class="gamepad-legend" aria-hidden="true">
    <span v-for="hint in hints" :key="hint.id" class="legend-item">
      <span class="legend-button" :class="'btn-' + hint.tone">{{ hint.glyph }}</span>
      <span class="legend-label">{{ hint.label }}</span>
    </span>
  </span>
</template>

<script setup>
import { computed } from 'vue'
import { viewActions } from '../composables/useGamepadActions.js'
import { bigScreenSettings } from '../composables/useBigScreenSettings.js'
import { chipFor } from '../composables/useGamepadLayout.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const props = defineProps({
  cursorMode: { type: Boolean, default: false },
})

/** 动作 id → 文案。按键符号（A/B/X/Y 还是 ✕○□△）由 useGamepadLayout 按手柄布局给出。 */
const LABELS = () => {
  const legend = t.value.gamepadLegend
  return {
    click: legend.click,
    exitCursor: legend.exitCursor,
    confirm: legend.confirm,
    back: legend.back,
    search: legend.search,
    favorite: legend.favorite,
    menu: legend.menu,
    scroll: legend.scroll,
    pages: legend.switchPage,
    cursor: legend.cursorOn,
  }
}

/**
 * 只显示当前上下文真的会响应的按键：
 * 全局动作（确认/返回/换页/滚动/光标）始终在，视图级动作（搜索/收藏/菜单）
 * 由各视图通过 useGamepadActions 声明——没声明的页面就不显示，不撒谎。
 * 按键符号跟随当前活跃手柄的布局，换手柄即时切换。
 */
const hints = computed(() => {
  const labels = LABELS()
  if (props.cursorMode) {
    return ['confirm', 'menu', 'back', 'scroll'].map((id) => {
      const { glyph, tone } = chipFor(id)
      const labelId = id === 'confirm' ? 'click' : id === 'back' ? 'exitCursor' : id
      return { id, glyph, tone, label: labels[labelId] }
    })
  }

  const actions = ['confirm', 'back']
  if (viewActions.value.has('search')) actions.push('search')
  if (viewActions.value.has('favorite')) actions.push('favorite')
  if (viewActions.value.has('menu')) actions.push('menu')
  actions.push('scroll', 'pages')
  // 设置里关掉光标模式时 L3 不会响应，提示条不能显示无效按键
  if (bigScreenSettings.value.gamepadCursorEnabled) actions.push('cursor')

  return actions.map((id) => {
    const { glyph, tone } = chipFor(id)
    return { id, glyph, tone, label: labels[id] }
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

  // Xbox 与 PS 布局共用四个色位，语义是「按钮家族色」：
  // 绿=确认系、红=返回系、蓝/黄(粉)=功能键（PS 的 ✕蓝 ○红 △绿 □粉）
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
