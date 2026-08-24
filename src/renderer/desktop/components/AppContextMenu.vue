<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="menuRef"
      class="context-menu"
      :style="{ left: position.left + 'px', top: position.top + 'px' }"
      @click.stop
      @keydown.esc.prevent="$emit('close')"
    >
      <button type="button" class="menu-item" data-focusable data-focus-key="ctx-launch" @click="$emit('launch')">
        <span class="menu-icon"><VideoPlay /></span> {{ t.appContext.launch }}
      </button>
      <button type="button" class="menu-item" data-focusable data-focus-key="ctx-favorite" @click="$emit('toggleFavorite')">
        <span class="menu-icon">
          <component :is="isFavorited ? StarFilled : Star" />
        </span>
        {{ isFavorited ? t.appContext.unfavorite : t.appContext.favorite }}
      </button>
      <div class="menu-divider"></div>
      <button type="button" class="menu-item" data-focusable data-focus-key="ctx-cover" @click="$emit('updateCover')">
        <span class="menu-icon"><Picture /></span> {{ t.appContext.updateCover }}
      </button>
      <button type="button" class="menu-item" data-focusable data-focus-key="ctx-helpers" @click="$emit('configHelpers')">
        <span class="menu-icon"><Lightning /></span> {{ t.appContext.launchHelper }}
      </button>
      <button type="button" class="menu-item" data-focusable data-focus-key="ctx-copy" v-if="hasCmd" @click="$emit('copyCmd')">
        <span class="menu-icon"><DocumentCopy /></span> {{ t.appContext.copyCommand }}
      </button>
      <button type="button" class="menu-item" data-focusable data-focus-key="ctx-dir" v-if="hasWorkingDir" @click="$emit('openDir')">
        <span class="menu-icon"><Folder /></span> {{ t.appContext.openDirectory }}
      </button>
    </div>
  </Teleport>
</template>

<script setup>
import { nextTick, onUnmounted, ref, watch } from 'vue'
import { useI18n } from '../i18n/index.js'
import { popFocusScope, pushFocusScope } from '../composables/useFocusNav.js'
import { DocumentCopy, Folder, Lightning, Picture, Star, StarFilled, VideoPlay } from '@element-plus/icons-vue'
const { t } = useI18n()

const props = defineProps({
  visible: { type: Boolean, required: true },
  x: { type: Number, default: 0 },
  y: { type: Number, default: 0 },
  isFavorited: { type: Boolean, default: false },
  hasCmd: { type: Boolean, default: false },
  hasWorkingDir: { type: Boolean, default: false },
})

defineEmits(['launch', 'toggleFavorite', 'copyCmd', 'openDir', 'configHelpers', 'updateCover', 'close'])

const menuRef = ref(null)
const position = ref({ left: 0, top: 0 })
const VIEWPORT_MARGIN = 8

let disposeScope = null

/**
 * 用菜单自己的实测尺寸夹到视口内。之前调用方按固定 200px 估算，
 * 菜单项数量变化或大屏缩放后就会溢出屏幕。
 */
function place() {
  const menu = menuRef.value
  const width = menu?.offsetWidth || 180
  const height = menu?.offsetHeight || 200
  position.value = {
    left: Math.max(VIEWPORT_MARGIN, Math.min(props.x, window.innerWidth - width - VIEWPORT_MARGIN)),
    top: Math.max(VIEWPORT_MARGIN, Math.min(props.y, window.innerHeight - height - VIEWPORT_MARGIN)),
  }
}

watch(
  () => [props.visible, props.x, props.y],
  async ([visible]) => {
    if (!visible) {
      disposeScope?.()
      disposeScope = null
      return
    }
    // 先按请求坐标渲染，再测量并纠正，避免出现一帧的错位
    position.value = { left: props.x, top: props.y }
    await nextTick()
    place()
    if (menuRef.value && !disposeScope) disposeScope = pushFocusScope(menuRef.value)
  },
  { immediate: true }
)

onUnmounted(() => {
  disposeScope?.()
})
</script>

<style lang="less" scoped>
.context-menu {
  position: fixed;
  background: rgba(var(--fd-bg-primary-rgb, 20, 20, 40), 0.98);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-radius: 10px;
  padding: 6px 0;
  min-width: 180px;
  z-index: 10000;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);

  .menu-item {
    width: 100%;
    padding: 8px 16px;
    font-size: 13px;
    font-family: inherit;
    text-align: left;
    border: none;
    background: transparent;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.8);
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 10px;
    transition: background 0.1s ease;

    .menu-icon {
      width: 18px;
      height: 18px;
      text-align: center;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;

      :deep(svg) {
        width: 15px;
        height: 15px;
      }
    }

    &:hover,
    &:focus-visible {
      background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
      color: var(--fd-accent, #00fff5);
    }
  }

  .menu-divider {
    height: 1px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
    margin: 4px 12px;
  }
}
</style>
