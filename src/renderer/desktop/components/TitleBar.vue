<template>
  <div class="desktop-titlebar" :class="{ 'draggable': draggable }">
    <div class="titlebar-left">
      <slot name="left">
        <img v-if="icon" :src="icon" alt="App Icon" class="app-icon" />
        <span v-if="title" class="app-title">{{ title }}</span>
      </slot>
    </div>
    
    <div class="titlebar-center">
      <slot name="center"></slot>
    </div>
    
    <div class="titlebar-right">
      <slot name="right">
        <WindowControls v-if="showControls" />
      </slot>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import WindowControls from './WindowControls.vue'

const props = defineProps({
  title: {
    type: String,
    default: 'SUNSHINE DESKTOP'
  },
  icon: {
    type: String,
    default: null
  },
  draggable: {
    type: Boolean,
    default: true
  },
  showControls: {
    type: Boolean,
    default: true
  }
})
</script>

<style lang="less" scoped>
.desktop-titlebar {
  height: 32px;
  background: rgba(15, 15, 35, 0.95);
  border-bottom: 1px solid rgba(0, 255, 245, 0.2);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
  position: relative;
  z-index: 100;
  user-select: none;

  &.draggable {
    -webkit-app-region: drag;
  }

  .titlebar-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    -webkit-app-region: no-drag;

    .app-icon {
      width: 16px;
      height: 16px;
    }

    .app-title {
      font-size: 12px;
      font-weight: 500;
      color: rgba(255, 255, 255, 0.7);
      letter-spacing: 0.5px;
    }
  }

  .titlebar-center {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    -webkit-app-region: drag;
  }

  .titlebar-right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    -webkit-app-region: no-drag;
  }
}
</style>

