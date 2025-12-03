<template>
  <div class="desktop-window" :class="windowClass">
    <slot name="titlebar">
      <TitleBar v-if="showTitleBar" :title="title" :icon="icon" />
    </slot>
    
    <div class="desktop-window-content" :class="{ 'has-sidebar': hasSidebar }">
      <slot name="sidebar"></slot>
      
      <main class="desktop-window-main">
        <slot></slot>
      </main>
    </div>
    
    <slot name="footer"></slot>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import TitleBar from './TitleBar.vue'

const props = defineProps({
  title: {
    type: String,
    default: 'Desktop Application'
  },
  icon: {
    type: String,
    default: null
  },
  showTitleBar: {
    type: Boolean,
    default: true
  },
  hasSidebar: {
    type: Boolean,
    default: false
  },
  theme: {
    type: String,
    default: 'dark', // dark, light
    validator: (value) => ['dark', 'light'].includes(value)
  }
})

const windowClass = computed(() => {
  return {
    [`theme-${props.theme}`]: true
  }
})
</script>

<style lang="less" scoped>
.desktop-window {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: linear-gradient(135deg, #0f0f23 0%, #1a1a2e 50%, #16213e 100%);
  position: relative;

  // 背景网格效果
  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-image: 
      linear-gradient(rgba(0, 255, 245, 0.03) 1px, transparent 1px),
      linear-gradient(90deg, rgba(0, 255, 245, 0.03) 1px, transparent 1px);
    background-size: 50px 50px;
    pointer-events: none;
    z-index: 0;
  }

  // 扫描线效果
  &::after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: repeating-linear-gradient(
      0deg,
      transparent,
      transparent 2px,
      rgba(0, 0, 0, 0.03) 2px,
      rgba(0, 0, 0, 0.03) 4px
    );
    pointer-events: none;
    z-index: 1;
  }

  .desktop-window-content {
    flex: 1;
    display: flex;
    position: relative;
    z-index: 2;
    height: calc(100vh - 32px);
    overflow: hidden;

    &.has-sidebar {
      height: calc(100vh - 32px);
    }
  }

  .desktop-window-main {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
    overflow-x: hidden;
    position: relative;
    z-index: 2;

    &::-webkit-scrollbar {
      width: 8px;
    }

    &::-webkit-scrollbar-track {
      background: transparent;
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(0, 255, 245, 0.2);
      border-radius: 4px;

      &:hover {
        background: rgba(0, 255, 245, 0.3);
      }
    }
  }
}

.theme-light {
  background: linear-gradient(135deg, #f5f5f5 0%, #ffffff 50%, #fafafa 100%);
  color: #1e293b;
}
</style>

