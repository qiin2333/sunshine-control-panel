<template>
  <Teleport to="body">
    <Transition name="pet-consent">
      <div
        v-if="open"
        class="pet-consent-mask"
        @click.self="cancel"
      >
        <section
          ref="dialogRef"
          class="pet-consent-dialog"
          role="alertdialog"
          aria-modal="true"
          aria-labelledby="pet-vision-consent-title"
          aria-describedby="pet-vision-consent-message"
          tabindex="-1"
          @keydown.esc.stop.prevent="cancel"
          @keydown.tab="keepFocusInside"
        >
          <div class="pet-consent-icon" aria-hidden="true">!</div>
          <h3 id="pet-vision-consent-title">{{ text.title }}</h3>
          <p id="pet-vision-consent-message">{{ text.message }}</p>
          <div class="pet-consent-actions">
            <button
              ref="cancelButtonRef"
              type="button"
              class="pet-consent-button secondary"
              @click="cancel"
            >
              {{ text.cancel }}
            </button>
            <button type="button" class="pet-consent-button primary" @click="confirm">
              {{ text.accept }}
            </button>
          </div>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { nextTick, onUnmounted, ref, watch } from 'vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  text: { type: Object, required: true },
})

const emit = defineEmits(['confirm', 'cancel'])
const dialogRef = ref(null)
const cancelButtonRef = ref(null)
let previousFocus = null

const restoreFocus = () => {
  previousFocus?.focus?.()
  previousFocus = null
}

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      restoreFocus()
      return
    }

    previousFocus = document.activeElement
    await nextTick()
    if (cancelButtonRef.value) {
      cancelButtonRef.value.focus()
    } else {
      dialogRef.value?.focus()
    }
  },
)

const confirm = () => emit('confirm')
const cancel = () => emit('cancel')

const keepFocusInside = (event) => {
  const buttons = dialogRef.value?.querySelectorAll('button:not(:disabled)')
  if (!buttons?.length) return

  const first = buttons[0]
  const last = buttons[buttons.length - 1]
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

onUnmounted(restoreFocus)
</script>

<style lang="less" scoped>
.pet-consent-mask {
  position: fixed;
  inset: 0;
  z-index: 4000;
  display: grid;
  place-items: center;
  padding: 20px;
  box-sizing: border-box;
  background: rgba(8, 22, 42, 0.68);
  backdrop-filter: blur(8px);
}

.pet-consent-dialog {
  width: min(440px, 100%);
  max-height: calc(100vh - 40px);
  overflow-y: auto;
  box-sizing: border-box;
  padding: 28px;
  color: #24364d;
  background: linear-gradient(145deg, #ffffff 0%, #eef7ff 100%);
  border: 1px solid rgba(91, 166, 239, 0.42);
  border-radius: 20px;
  box-shadow: 0 24px 80px rgba(4, 24, 52, 0.38);
  outline: none;

  h3 {
    margin: 14px 0 10px;
    font-size: 20px;
    line-height: 1.35;
    text-align: center;
  }

  p {
    margin: 0;
    color: #52657c;
    font-size: 14px;
    line-height: 1.7;
    white-space: pre-line;
  }
}

.pet-consent-icon {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  margin: 0 auto;
  color: #ffffff;
  font-size: 26px;
  font-weight: 700;
  background: linear-gradient(145deg, #ffb65c, #ff7f50);
  border-radius: 16px;
  box-shadow: 0 10px 24px rgba(255, 127, 80, 0.28);
}

.pet-consent-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 24px;
}

.pet-consent-button {
  min-width: 112px;
  min-height: 44px;
  padding: 10px 18px;
  border-radius: 12px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  touch-action: manipulation;
  transition: transform 0.16s ease, box-shadow 0.16s ease, background 0.16s ease;

  &:active {
    transform: scale(0.97);
  }

  &:focus-visible {
    outline: 3px solid rgba(66, 153, 255, 0.35);
    outline-offset: 2px;
  }

  &.secondary {
    color: #50647c;
    background: rgba(255, 255, 255, 0.8);
    border: 1px solid #c7d9eb;
  }

  &.primary {
    color: #ffffff;
    background: linear-gradient(135deg, #3d91ef, #2676d5);
    border: 1px solid transparent;
    box-shadow: 0 8px 20px rgba(38, 118, 213, 0.28);
  }
}

.pet-consent-enter-active,
.pet-consent-leave-active {
  transition: opacity 0.18s ease;
}

.pet-consent-enter-active .pet-consent-dialog,
.pet-consent-leave-active .pet-consent-dialog {
  transition: transform 0.18s ease, opacity 0.18s ease;
}

.pet-consent-enter-from,
.pet-consent-leave-to {
  opacity: 0;
}

.pet-consent-enter-from .pet-consent-dialog,
.pet-consent-leave-to .pet-consent-dialog {
  opacity: 0;
  transform: translateY(12px) scale(0.97);
}

@media (prefers-reduced-motion: reduce) {
  .pet-consent-mask,
  .pet-consent-dialog,
  .pet-consent-button {
    transition: none;
  }
}

@media (max-width: 520px) {
  .pet-consent-dialog {
    padding: 22px;
  }

  .pet-consent-actions {
    flex-direction: column-reverse;
  }

  .pet-consent-button {
    width: 100%;
  }
}
</style>
