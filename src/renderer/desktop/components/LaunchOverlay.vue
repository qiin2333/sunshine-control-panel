<template>
  <Teleport to="body">
    <Transition name="launch-overlay">
      <div v-if="state" ref="overlayRef" class="launch-overlay" :class="'status-' + state.status">
        <div class="launch-card">
          <div class="launch-cover">
            <img v-if="state.coverUrl" :src="state.coverUrl" :alt="state.appName" />
            <span v-else class="launch-cover-letter">{{ state.appName?.[0] || '?' }}</span>
            <div v-if="state.status === 'launching'" class="launch-cover-scan"></div>
          </div>

          <div class="launch-body">
            <div class="launch-status">
              <span v-if="state.status === 'launching'" class="launch-spinner"></span>
              <span v-else class="launch-glyph" :class="'glyph-' + state.status">{{ glyph }}</span>
              <span class="launch-headline">{{ headline }}</span>
            </div>

            <p class="launch-detail">{{ detail }}</p>

            <div class="launch-actions">
              <button
                v-if="state.status === 'conflict'"
                type="button"
                class="launch-btn danger"
                data-focusable
                data-focus-key="launch-stop"
                @click="emit('stop')"
              >
                {{ t.gameSession.stopRunning }}
              </button>
              <button
                v-if="state.status !== 'launching'"
                type="button"
                class="launch-btn"
                data-focusable
                data-focus-key="launch-dismiss"
                @click="emit('dismiss')"
              >
                {{ t.gameSession.dismiss }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { computed, nextTick, ref, watch } from 'vue'
import { formatDuration } from '../composables/useGameSession.js'
import { useModalFocusScope } from '../composables/useFocusNav.js'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

const props = defineProps({
  state: { type: Object, default: null },
})

const emit = defineEmits(['dismiss', 'stop'])

// 这是一层吞点击的全屏遮罩，必须和其他弹层一样压入焦点栈：否则手柄导航会继续在
// 遮罩背后的应用网格里走，而 conflict 态自己的「结束游戏 / 关闭」按钮反而够不到
// —— teleport 出去的节点不在 .desktop-window 里，收集不到。
const overlayRef = ref(null)
useModalFocusScope(overlayRef, () => !!props.state)

// launching 态没有任何按钮，作用域里无处可聚焦。主动把焦点从遮罩背后的卡片上移开，
// 否则 A 键会点到被完全挡住的应用卡片。关闭时 popFocusScope 会把焦点还回去。
watch(
  () => props.state?.status,
  async () => {
    await nextTick()
    const root = overlayRef.value
    if (!root || root.querySelector('[data-focusable]')) return
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur()
  },
  { immediate: true }
)

const glyph = computed(() => {
  switch (props.state?.status) {
    case 'started':
      return '✓'
    case 'exited':
      return '⏻'
    case 'untracked':
    case 'conflict':
      return '!'
    default:
      return '×'
  }
})

function text(key, replacements = {}) {
  let value = t.value.gameSession?.[key] || ''
  for (const [name, replacement] of Object.entries(replacements)) {
    value = value.replace(`{${name}}`, replacement)
  }
  return value
}

const headline = computed(() => {
  const name = props.state?.appName || ''
  switch (props.state?.status) {
    case 'launching':
      return text('launching', { name })
    case 'started':
      return text('started', { name })
    case 'untracked':
      return text('untracked', { name })
    case 'conflict':
      return text('conflict', { name: props.state.message || '' })
    case 'exited':
      return text('exited', { name })
    default:
      return text('launchFailed', { name })
  }
})

const detail = computed(() => {
  switch (props.state?.status) {
    case 'launching':
      return text('launchingHint')
    case 'started':
      return text('startedHint')
    case 'untracked':
      return props.state.message === 'detached-only'
        ? text('untrackedDetached')
        : text('untrackedNoHandle')
    case 'conflict':
      return text('conflictHint')
    case 'exited':
      return text('exitedHint', { duration: formatDuration(props.state.seconds) })
    default:
      return props.state?.message || ''
  }
})
</script>

<style lang="less" scoped>
.launch-overlay {
  position: fixed;
  inset: 0;
  z-index: 19000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.88);
  backdrop-filter: blur(10px);
}

.launch-card {
  display: flex;
  align-items: center;
  gap: 28px;
  padding: 28px 36px;
  border-radius: 20px;
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.9);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.22);
  box-shadow: 0 22px 70px rgba(0, 0, 0, 0.6);
  max-width: min(760px, 88vw);
}

.launch-cover {
  position: relative;
  width: 132px;
  height: 176px;
  flex-shrink: 0;
  border-radius: 14px;
  overflow: hidden;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.8);
  display: flex;
  align-items: center;
  justify-content: center;

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .launch-cover-letter {
    font-size: 56px;
    font-weight: 700;
    color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
    text-transform: uppercase;
  }

  .launch-cover-scan {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to bottom,
      transparent 0%,
      rgba(var(--fd-accent-rgb, 0, 255, 245), 0.35) 50%,
      transparent 100%
    );
    animation: launch-scan 1.6s ease-in-out infinite;
  }
}

@keyframes launch-scan {
  0% {
    transform: translateY(-100%);
  }
  100% {
    transform: translateY(100%);
  }
}

.launch-body {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.launch-status {
  display: flex;
  align-items: center;
  gap: 12px;
}

.launch-headline {
  font-size: 22px;
  font-weight: 600;
  color: var(--fd-text-primary, #fff);
}

.launch-spinner {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
  border: 3px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-top-color: var(--fd-accent, #00fff5);
  border-radius: 50%;
  animation: launch-spin 0.8s linear infinite;
}

@keyframes launch-spin {
  to {
    transform: rotate(360deg);
  }
}

.launch-glyph {
  width: 26px;
  height: 26px;
  flex-shrink: 0;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 15px;
  font-weight: 700;

  &.glyph-started,
  &.glyph-exited {
    background: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.16);
    color: var(--fd-status-success, #00ff88);
  }

  &.glyph-untracked,
  &.glyph-conflict {
    background: rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.16);
    color: var(--fd-status-warning, #ffd700);
  }

  &.glyph-error {
    background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.16);
    color: var(--fd-status-danger, #ff6b35);
  }
}

.launch-detail {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.55);
  white-space: pre-line;
  word-break: break-word;
}

.launch-actions {
  display: flex;
  gap: 10px;
  margin-top: 4px;
}

.launch-btn {
  padding: 8px 20px;
  border-radius: 8px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
  background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
  color: var(--fd-accent, #00fff5);
  font-size: 14px;
  cursor: pointer;
  transition:
    background 0.2s ease,
    border-color 0.2s ease;

  &:hover,
  &:focus-visible {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
    border-color: var(--fd-accent, #00fff5);
  }

  &.danger {
    border-color: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.4);
    background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.12);
    color: var(--fd-status-danger, #ff6b35);

    &:hover,
    &:focus-visible {
      background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.22);
      border-color: var(--fd-status-danger, #ff6b35);
    }
  }
}

.launch-overlay-enter-active,
.launch-overlay-leave-active {
  transition: opacity 0.2s ease;
}

.launch-overlay-enter-from,
.launch-overlay-leave-to {
  opacity: 0;
}
</style>
