<template>
  <nav class="desktop-sidebar" :class="{ 'collapsed': collapsed }">
    <div class="sidebar-content">
      <div 
        v-for="item in items" 
        :key="item.id"
        class="nav-item"
        :class="{ active: activeItem === item.id, disabled: item.disabled }"
        @click="handleItemClick(item)"
        :title="item.label"
      >
        <component v-if="item.icon" :is="item.icon" class="nav-icon" />
        <span v-if="!collapsed" class="nav-label">{{ item.label }}</span>
        <span v-if="item.badge && !collapsed" class="nav-badge">{{ item.badge }}</span>
      </div>
      
      <div v-if="items.length > 0 && showDivider" class="nav-divider"></div>
      
      <div 
        v-for="item in bottomItems" 
        :key="item.id"
        class="nav-item"
        :class="{ active: activeItem === item.id, disabled: item.disabled }"
        @click="handleItemClick(item)"
        :title="item.label"
      >
        <component v-if="item.icon" :is="item.icon" class="nav-icon" />
        <span v-if="!collapsed" class="nav-label">{{ item.label }}</span>
        <span v-if="item.badge && !collapsed" class="nav-badge">{{ item.badge }}</span>
      </div>
    </div>
    
    <div v-if="collapsible" class="sidebar-toggle" @click="collapsed = !collapsed">
      <svg :class="{ 'rotated': !collapsed }" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M9 18l6-6-6-6"/>
      </svg>
    </div>
  </nav>
</template>

<script setup>
import { ref, computed } from 'vue'

const props = defineProps({
  items: {
    type: Array,
    default: () => []
  },
  bottomItems: {
    type: Array,
    default: () => []
  },
  activeItem: {
    type: String,
    default: null
  },
  collapsed: {
    type: Boolean,
    default: false
  },
  collapsible: {
    type: Boolean,
    default: false
  },
  showDivider: {
    type: Boolean,
    default: true
  }
})

const emit = defineEmits(['item-click', 'update:collapsed', 'update:activeItem'])

function handleItemClick(item) {
  if (item.disabled) return
  emit('item-click', item)
  emit('update:activeItem', item.id)
}
</script>

<style lang="less" scoped>
.desktop-sidebar {
  width: 72px;
  background: rgba(15, 15, 35, 0.8);
  border-right: 1px solid rgba(0, 255, 245, 0.2);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 16px 0;
  gap: 8px;
  backdrop-filter: blur(10px);
  transition: width 0.3s ease;
  position: relative;

  &.collapsed {
    width: 56px;

    .nav-label {
      display: none;
    }

    .nav-badge {
      display: none;
    }
  }

  .sidebar-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .nav-item {
    width: 48px;
    min-height: 48px;
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s ease;
    position: relative;
    color: rgba(255, 255, 255, 0.7);
    padding: 8px;
    gap: 4px;

    &:hover:not(.disabled) {
      background: rgba(0, 255, 245, 0.1);
      color: #00fff5;
    }

    &.active {
      background: rgba(0, 255, 245, 0.15);
      color: #00fff5;
      box-shadow: 0 0 20px rgba(0, 255, 245, 0.3), 0 0 40px rgba(0, 255, 245, 0.1);

      &::before {
        content: '';
        position: absolute;
        left: -12px;
        width: 4px;
        height: 24px;
        background: #00fff5;
        border-radius: 0 4px 4px 0;
        box-shadow: 0 0 20px rgba(0, 255, 245, 0.3), 0 0 40px rgba(0, 255, 245, 0.1);
      }
    }

    &.disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }

    .nav-icon {
      width: 24px;
      height: 24px;
      flex-shrink: 0;
    }

    .nav-label {
      font-size: 11px;
      text-align: center;
      line-height: 1.2;
    }

    .nav-badge {
      position: absolute;
      top: 4px;
      right: 4px;
      background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);
      color: #0f0f23;
      font-size: 9px;
      padding: 2px 4px;
      border-radius: 8px;
      font-weight: 600;
      min-width: 16px;
      text-align: center;
    }
  }

  .nav-divider {
    width: 32px;
    height: 1px;
    background: rgba(0, 255, 245, 0.2);
    margin: 8px 0;
  }

  .sidebar-toggle {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: rgba(255, 255, 255, 0.5);
    transition: all 0.2s ease;

    &:hover {
      background: rgba(0, 255, 245, 0.1);
      color: #00fff5;
    }

    svg {
      width: 16px;
      height: 16px;
      transition: transform 0.3s ease;
      transform: rotate(0deg);

      &.rotated {
        transform: rotate(180deg);
      }
    }
  }
}
</style>

