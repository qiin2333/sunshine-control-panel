<template>
  <Teleport to="body">
    <Transition name="osk">
      <div v-if="state.open" ref="overlayRef" class="osk-overlay" @keydown="onKeydown">
        <div class="osk-panel">
          <div class="osk-header">
            <span class="osk-title">{{ state.title || t.osk.title }}</span>
            <span class="osk-counter" v-if="state.maxLength">
              {{ draft.length }} / {{ state.maxLength }}
            </span>
          </div>

          <div class="osk-preview" :class="{ empty: !draft }">
            <span class="osk-preview-text">{{ previewText }}</span>
            <span class="osk-caret"></span>
          </div>

          <div class="osk-rows">
            <div v-for="(row, rowIndex) in rows" :key="'row-' + rowIndex" class="osk-row">
              <button
                v-for="key in row"
                :key="key"
                type="button"
                class="osk-key"
                data-focusable
                :data-focus-key="'osk-' + key"
                @click="append(key)"
              >
                {{ shifted ? key.toUpperCase() : key }}
              </button>
            </div>
          </div>

          <div class="osk-row osk-actions">
            <button
              v-if="state.mode === 'text'"
              type="button"
              class="osk-key wide"
              :class="{ toggled: shifted }"
              data-focusable
              data-focus-key="osk-shift"
              @click="shifted = !shifted"
            >
              {{ t.osk.shift }}
            </button>
            <button
              v-if="state.mode === 'text'"
              type="button"
              class="osk-key wide"
              :class="{ toggled: symbols }"
              data-focusable
              data-focus-key="osk-symbols"
              @click="symbols = !symbols"
            >
              {{ symbols ? t.osk.letters : t.osk.symbols }}
            </button>
            <button
              v-if="state.mode === 'text'"
              type="button"
              class="osk-key grow"
              data-focusable
              data-focus-key="osk-space"
              @click="append(' ')"
            >
              {{ t.osk.space }}
            </button>
            <button
              type="button"
              class="osk-key wide"
              data-focusable
              data-focus-key="osk-backspace"
              @click="backspace"
            >
              ⌫
            </button>
            <button
              type="button"
              class="osk-key wide"
              data-focusable
              data-focus-key="osk-clear"
              @click="draft = ''"
            >
              {{ t.osk.clear }}
            </button>
          </div>

          <div class="osk-row osk-commit">
            <button
              type="button"
              class="osk-key cancel"
              data-focusable
              data-focus-key="osk-cancel"
              @click="cancel"
            >
              {{ t.osk.cancel }}
            </button>
            <button
              type="button"
              class="osk-key confirm"
              data-focusable
              data-focus-key="osk-confirm"
              @click="commit"
            >
              {{ t.osk.confirm }}
            </button>
          </div>

          <div class="osk-hint">{{ t.osk.hint }}</div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { nextTick, ref, computed, watch, onUnmounted } from 'vue'
import { cancelOsk, commitOsk, oskState } from '../composables/useOsk.js'
import { popFocusScope, pushFocusScope } from '../composables/useFocusNav.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const state = oskState
const overlayRef = ref(null)
const draft = ref('')
const shifted = ref(false)
const symbols = ref(false)

const LETTER_ROWS = [
  ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
  ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
  ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
  ['z', 'x', 'c', 'v', 'b', 'n', 'm', '-', '_'],
]

const SYMBOL_ROWS = [
  ['!', '@', '#', '$', '%', '^', '&', '*', '(', ')'],
  ['-', '_', '=', '+', '[', ']', '{', '}', ';', ':'],
  ["'", '"', ',', '.', '<', '>', '/', '?', '\\', '|', '~', '`'],
]

const NUMBER_ROWS = [
  ['1', '2', '3'],
  ['4', '5', '6'],
  ['7', '8', '9'],
  ['0'],
]

/** 预览框只显示尾部，长文本从左侧截断，光标始终可见。 */
const PREVIEW_TAIL = 60
const previewText = computed(() => {
  if (!draft.value) return state.value.placeholder || t.value.osk.placeholder
  if (draft.value.length <= PREVIEW_TAIL) return draft.value
  return `…${draft.value.slice(-PREVIEW_TAIL)}`
})

const rows = computed(() => {
  if (state.value.mode === 'number') return NUMBER_ROWS
  return symbols.value ? SYMBOL_ROWS : LETTER_ROWS
})

let disposeScope = null

watch(
  () => state.value.open,
  async (open) => {
    if (open) {
      draft.value = state.value.value
      shifted.value = false
      symbols.value = false
      await nextTick()
      if (overlayRef.value) disposeScope = pushFocusScope(overlayRef.value)
    } else {
      disposeScope?.()
      disposeScope = null
    }
  },
  { immediate: true }
)

onUnmounted(() => {
  disposeScope?.()
})

function atLimit() {
  return state.value.maxLength > 0 && draft.value.length >= state.value.maxLength
}

function append(character) {
  if (atLimit()) return
  const value = shifted.value ? character.toUpperCase() : character
  draft.value += value
  // Shift 只作用于一个字符，和手机键盘一致
  if (shifted.value) shifted.value = false
}

function backspace() {
  draft.value = draft.value.slice(0, -1)
}

function commit() {
  commitOsk(draft.value)
}

function cancel() {
  cancelOsk()
}

/** 物理键盘依然可用：串流用手柄，本机调试用键盘。 */
function onKeydown(event) {
  if (event.key === 'Escape') {
    event.preventDefault()
    cancel()
    return
  }
  if (event.key === 'Enter') {
    event.preventDefault()
    commit()
    return
  }
  if (event.key === 'Backspace') {
    event.preventDefault()
    backspace()
    return
  }
  if (event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey) {
    if (state.value.mode === 'number' && !/[0-9]/.test(event.key)) return
    event.preventDefault()
    if (!atLimit()) draft.value += event.key
  }
}
</script>

<style lang="less" scoped>
.osk-overlay {
  position: fixed;
  inset: 0;
  z-index: 20000;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: 0 0 4vh;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(6px);
}

.osk-panel {
  width: min(920px, 92vw);
  padding: 20px 24px 16px;
  border-radius: 18px;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.97);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.22);
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.osk-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;

  .osk-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--fd-accent, #00fff5);
  }

  .osk-counter {
    font-size: 12px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);
  }
}

.osk-preview {
  min-height: 46px;
  padding: 10px 14px;
  border-radius: 10px;
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.75);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.18);
  font-size: 20px;
  color: var(--fd-text-primary, #fff);
  display: flex;
  align-items: center;
  gap: 2px;
  overflow: hidden;

  &.empty .osk-preview-text {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25);
  }

  .osk-preview-text {
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .osk-caret {
    width: 2px;
    height: 24px;
    background: var(--fd-accent, #00fff5);
    animation: osk-blink 1s steps(2, start) infinite;
    flex-shrink: 0;
  }
}

@keyframes osk-blink {
  to {
    visibility: hidden;
  }
}

.osk-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.osk-row {
  display: flex;
  gap: 8px;
  justify-content: center;
}

.osk-key {
  min-width: 52px;
  height: 48px;
  padding: 0 12px;
  border-radius: 10px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.14);
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.7);
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.85);
  font-size: 17px;
  cursor: pointer;
  transition:
    background 0.15s ease,
    border-color 0.15s ease,
    transform 0.12s ease;

  &:hover,
  &:focus-visible {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.16);
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.5);
    color: var(--fd-accent, #00fff5);
  }

  &:active {
    transform: scale(0.94);
  }

  &.wide {
    min-width: 84px;
    font-size: 14px;
  }

  &.grow {
    flex: 1;
    font-size: 14px;
  }

  &.toggled {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.22);
    border-color: var(--fd-accent, #00fff5);
    color: var(--fd-accent, #00fff5);
  }

  &.confirm {
    min-width: 160px;
    font-size: 15px;
    font-weight: 600;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.18);
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.45);
    color: var(--fd-accent, #00fff5);
  }

  &.cancel {
    min-width: 160px;
    font-size: 15px;
  }
}

.osk-commit {
  margin-top: 4px;
}

.osk-hint {
  text-align: center;
  font-size: 12px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
}

.osk-enter-active,
.osk-leave-active {
  transition: opacity 0.18s ease;

  .osk-panel {
    transition: transform 0.22s cubic-bezier(0.22, 1, 0.36, 1);
  }
}

.osk-enter-from,
.osk-leave-to {
  opacity: 0;

  .osk-panel {
    transform: translateY(24px);
  }
}
</style>
