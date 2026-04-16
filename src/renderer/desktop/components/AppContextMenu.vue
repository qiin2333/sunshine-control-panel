<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="context-menu"
      :style="{ left: x + 'px', top: y + 'px' }"
      @click.stop
    >
      <div class="menu-item" @click="$emit('launch')">
        <span class="menu-icon">▶</span> {{ t.appContext.launch }}
      </div>
      <div class="menu-item" @click="$emit('toggleFavorite')">
        <span class="menu-icon">{{ isFavorited ? '★' : '☆' }}</span>
        {{ isFavorited ? t.appContext.unfavorite : t.appContext.favorite }}
      </div>
      <div class="menu-divider"></div>
      <div class="menu-item" @click="$emit('updateCover')">
        <span class="menu-icon"><Picture /></span> {{ t.appContext.updateCover }}
      </div>
      <div class="menu-item" @click="$emit('configHelpers')">
        <span class="menu-icon"><Lightning /></span> {{ t.appContext.launchHelper }}
      </div>
      <div class="menu-item" @click="$emit('copyCmd')" v-if="hasCmd">
        <span class="menu-icon"><DocumentCopy /></span> {{ t.appContext.copyCommand }}
      </div>
      <div class="menu-item" @click="$emit('openDir')" v-if="hasWorkingDir">
        <span class="menu-icon"><Folder /></span> {{ t.appContext.openDirectory }}
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { useI18n } from '../i18n/index.js'
import { Picture, Lightning, DocumentCopy, Folder } from '@element-plus/icons-vue'
const { t } = useI18n()

defineProps({
  visible: { type: Boolean, required: true },
  x: { type: Number, default: 0 },
  y: { type: Number, default: 0 },
  isFavorited: { type: Boolean, default: false },
  hasCmd: { type: Boolean, default: false },
  hasWorkingDir: { type: Boolean, default: false },
})

defineEmits(['launch', 'toggleFavorite', 'copyCmd', 'openDir', 'configHelpers', 'updateCover'])
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
    padding: 8px 16px;
    font-size: 13px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.8);
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 10px;
    transition: background 0.1s ease;

    .menu-icon {
      width: 18px;
      text-align: center;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-size: 14px;
      flex-shrink: 0;
    }

    &:hover {
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
