<template>
  <Transition name="running-bar">
    <div v-if="game" class="running-bar">
      <span class="running-pulse"></span>

      <div class="running-info">
        <span class="running-label">{{ t.gameSession.nowPlaying }}</span>
        <span class="running-name" :title="game.appName">{{ game.appName }}</span>
      </div>

      <div class="running-meta">
        <span class="running-elapsed">{{ formatDuration(elapsed) }}</span>
        <span v-if="totalLabel" class="running-total">{{ totalLabel }}</span>
        <span v-if="game.adopted" class="running-adopted" :title="t.gameSession.adoptedHint">
          {{ t.gameSession.adopted }}
        </span>
      </div>

      <div class="running-actions">
        <button
          type="button"
          class="running-btn"
          data-focusable
          data-focus-key="running-resume"
          @click="emit('resume')"
        >
          {{ t.gameSession.backToGame }}
        </button>
        <button
          type="button"
          class="running-btn danger"
          data-focusable
          data-focus-key="running-stop"
          @click="emit('stop')"
        >
          {{ t.gameSession.stopRunning }}
        </button>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { computed } from 'vue'
import { formatDuration } from '../composables/useGameSession.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const props = defineProps({
  game: { type: Object, default: null },
  elapsed: { type: Number, default: 0 },
  stat: { type: Object, default: null },
})

const emit = defineEmits(['resume', 'stop'])

const totalLabel = computed(() => {
  const total = Number(props.stat?.totalSeconds) || 0
  // 本局时长还没结算进统计，所以这里加上去，显示的才是真实累计
  const combined = total + (Number(props.elapsed) || 0)
  if (combined < 60) return ''
  return t.value.gameSession.totalPlaytime.replace('{duration}', formatDuration(combined))
})
</script>

<style lang="less" scoped>
.running-bar {
  // 定位交给 DesktopApp 的 .desktop-footer-dock，这样它能和按键提示条堆叠而不是重叠
  display: flex;
  align-items: center;
  gap: 18px;
  max-width: 100%;
  padding: 10px 18px;
  border-radius: 14px;
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.94);
  border: 1px solid rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.3);
  box-shadow: 0 10px 36px rgba(0, 0, 0, 0.5);
}

.running-pulse {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--fd-status-success, #00ff88);
  box-shadow: 0 0 12px rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.8);
  animation: running-pulse 1.8s ease-in-out infinite;
}

@keyframes running-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}

.running-info {
  display: flex;
  flex-direction: column;
  min-width: 0;

  .running-label {
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);
  }

  .running-name {
    font-size: 15px;
    font-weight: 600;
    color: var(--fd-text-primary, #fff);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }
}

.running-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;

  .running-elapsed {
    font-size: 20px;
    font-variant-numeric: tabular-nums;
    color: var(--fd-status-success, #00ff88);
  }

  .running-total {
    font-size: 12px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);
  }

  .running-adopted {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.12);
    color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.85);
    cursor: help;
  }
}

.running-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.running-btn {
  padding: 7px 16px;
  border-radius: 8px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.25);
  background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
  color: var(--fd-accent, #00fff5);
  font-size: 13px;
  cursor: pointer;
  transition:
    background 0.2s ease,
    border-color 0.2s ease;

  &:hover,
  &:focus-visible {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.18);
    border-color: var(--fd-accent, #00fff5);
  }

  &.danger {
    border-color: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.35);
    background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.1);
    color: var(--fd-status-danger, #ff6b35);

    &:hover,
    &:focus-visible {
      background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.2);
      border-color: var(--fd-status-danger, #ff6b35);
    }
  }
}

.running-bar-enter-active,
.running-bar-leave-active {
  transition:
    opacity 0.25s ease,
    transform 0.25s cubic-bezier(0.22, 1, 0.36, 1);
}

.running-bar-enter-from,
.running-bar-leave-to {
  opacity: 0;
  transform: translateY(16px);
}
</style>
