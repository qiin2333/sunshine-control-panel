<template>
  <Teleport to="body">
    <Transition name="confirm">
      <div v-if="state.open" class="confirm-mask" @keydown.esc.prevent="reject">
        <div ref="dialogRef" class="confirm-dialog" :class="{ danger: state.danger }">
          <h3 v-if="state.title" class="confirm-title">{{ state.title }}</h3>
          <p class="confirm-message">{{ state.message }}</p>
          <div class="confirm-actions">
            <button
              type="button"
              class="confirm-btn"
              data-focusable
              data-focus-key="confirm-cancel"
              @click="reject"
            >
              {{ state.cancelLabel || t.common.cancel }}
            </button>
            <button
              type="button"
              class="confirm-btn primary"
              data-focusable
              data-focus-key="confirm-accept"
              @click="accept"
            >
              {{ state.confirmLabel || t.common.confirm }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { ref } from 'vue'
import { acceptConfirm, confirmState, rejectConfirm } from '../composables/useConfirm.js'
import { useModalFocusScope } from '../composables/useFocusNav.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const state = confirmState
const dialogRef = ref(null)
useModalFocusScope(dialogRef, () => state.value.open)

function accept() {
  acceptConfirm()
}

function reject() {
  rejectConfirm()
}
</script>

<style lang="less" scoped>
.confirm-mask {
  position: fixed;
  inset: 0;
  z-index: 21000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(6px);
}

.confirm-dialog {
  width: min(460px, 88vw);
  padding: 24px 28px 20px;
  border-radius: 16px;
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.97);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.22);
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.6);

  &.danger {
    border-color: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.35);
  }
}

.confirm-title {
  margin: 0 0 10px;
  font-size: 18px;
  font-weight: 600;
  color: var(--fd-text-primary, #fff);
}

.confirm-message {
  margin: 0 0 20px;
  font-size: 14px;
  line-height: 1.65;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  white-space: pre-line;
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.confirm-btn {
  padding: 8px 22px;
  border-radius: 8px;
  border: 1px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.14);
  background: transparent;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.7);
  font-size: 14px;
  font-family: inherit;
  cursor: pointer;
  transition:
    background 0.2s ease,
    border-color 0.2s ease,
    color 0.2s ease;

  &:hover,
  &:focus-visible {
    border-color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);
    color: var(--fd-text-primary, #fff);
  }

  &.primary {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.4);
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.14);
    color: var(--fd-accent, #00fff5);

    &:hover,
    &:focus-visible {
      background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.24);
      border-color: var(--fd-accent, #00fff5);
    }
  }
}

.danger .confirm-btn.primary {
  border-color: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.45);
  background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.14);
  color: var(--fd-status-danger, #ff6b35);

  &:hover,
  &:focus-visible {
    background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.24);
    border-color: var(--fd-status-danger, #ff6b35);
  }
}

.confirm-enter-active,
.confirm-leave-active {
  transition: opacity 0.18s ease;

  .confirm-dialog {
    transition: transform 0.2s cubic-bezier(0.22, 1, 0.36, 1);
  }
}

.confirm-enter-from,
.confirm-leave-to {
  opacity: 0;

  .confirm-dialog {
    transform: scale(0.96);
  }
}
</style>
