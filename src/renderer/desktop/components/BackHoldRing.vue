<template>
  <Teleport to="body">
    <Transition name="back-ring">
      <div v-if="visible" class="back-hold-ring" aria-hidden="true">
        <svg viewBox="0 0 48 48">
          <circle class="ring-track" cx="24" cy="24" r="20" />
          <circle
            class="ring-fill"
            cx="24"
            cy="24"
            r="20"
            :stroke-dasharray="CIRCUMFERENCE"
            :stroke-dashoffset="CIRCUMFERENCE * (1 - progress)"
          />
        </svg>
        <span class="ring-label">{{ t.gamepadLegend.backRootHint }}</span>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { computed } from 'vue'
import { backHoldProgress } from '../composables/useGamepad.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const CIRCUMFERENCE = 2 * Math.PI * 20
const props = defineProps({
  progress: { type: Number, default: 0 },
})

const visible = computed(() => props.progress > 0.06)
</script>

<style lang="less" scoped>
.back-hold-ring {
  position: fixed;
  left: 50%;
  bottom: 22vh;
  transform: translateX(-50%);
  z-index: 22000;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  pointer-events: none;

  svg {
    width: 56px;
    height: 56px;
    transform: rotate(-90deg);
    filter: drop-shadow(0 2px 10px rgba(0, 0, 0, 0.6));
  }

  .ring-track,
  .ring-fill {
    fill: none;
    stroke-width: 4;
    stroke-linecap: round;
  }

  .ring-track {
    stroke: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.18);
  }

  .ring-fill {
    stroke: var(--fd-accent, #00fff5);
    transition: stroke-dashoffset 16ms linear;
  }

  .ring-label {
    font-size: 13px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.75);
    background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.85);
    padding: 4px 14px;
    border-radius: 8px;
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  }
}

.back-ring-enter-active,
.back-ring-leave-active {
  transition:
    opacity 0.12s ease,
    transform 0.12s ease;
}

.back-ring-enter-from,
.back-ring-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(6px);
}
</style>
