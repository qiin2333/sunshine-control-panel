<template>
  <div class="setting-item" :class="{ disabled }">
    <div class="setting-info">
      <div class="setting-name" :class="nameClass">
        <slot name="name">{{ name }}</slot>
      </div>
      <div class="setting-desc">
        <slot name="description">{{ description }}</slot>
      </div>
    </div>
    <div class="setting-control" :class="controlClass">
      <slot></slot>
    </div>
  </div>
</template>

<script setup>
defineProps({
  name: {
    type: String,
    default: '',
  },
  description: {
    type: String,
    default: '',
  },
  nameClass: {
    type: [String, Array, Object],
    default: null,
  },
  controlClass: {
    type: [String, Array, Object],
    default: null,
  },
  disabled: {
    type: Boolean,
    default: false,
  },
})
</script>

<style lang="less" scoped>
.setting-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 0;
  border-bottom: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);

  &:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  &:first-child {
    padding-top: 0;
  }

  &.disabled {
    opacity: 0.5;
  }
}

.setting-name {
  font-size: 16px;
  font-weight: 500;
  color: var(--fd-text-primary, #fff);
  margin-bottom: 4px;
}

.setting-desc {
  font-size: 14px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  line-height: 1.45;
}

.tool-path-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.tool-path-item {
  flex-direction: column;
  align-items: flex-start;
  gap: 10px;

  .setting-control {
    width: 100%;
  }
}
</style>
