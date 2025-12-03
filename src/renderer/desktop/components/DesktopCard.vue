<template>
  <div 
    class="desktop-card" 
    :class="{
      'hoverable': hoverable,
      'clickable': clickable,
      [`card-${variant}`]: variant
    }"
    @click="handleClick"
  >
    <div v-if="showHeader" class="card-header">
      <div class="card-title">
        <component v-if="icon" :is="icon" class="title-icon" />
        <span v-if="title">{{ title }}</span>
        <slot name="title"></slot>
      </div>
      <div class="card-actions">
        <slot name="actions"></slot>
      </div>
    </div>
    
    <div class="card-content" :class="{ 'no-padding': noPadding }">
      <slot></slot>
    </div>
    
    <div v-if="$slots.footer" class="card-footer">
      <slot name="footer"></slot>
    </div>
  </div>
</template>

<script setup>
const props = defineProps({
  title: {
    type: String,
    default: null
  },
  icon: {
    type: [Object, String],
    default: null
  },
  variant: {
    type: String,
    default: 'default', // default, primary, secondary, success, warning, danger
    validator: (value) => ['default', 'primary', 'secondary', 'success', 'warning', 'danger'].includes(value)
  },
  hoverable: {
    type: Boolean,
    default: false
  },
  clickable: {
    type: Boolean,
    default: false
  },
  showHeader: {
    type: Boolean,
    default: true
  },
  noPadding: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['click'])

function handleClick() {
  if (props.clickable) {
    emit('click')
  }
}
</script>

<style lang="less" scoped>
.desktop-card {
  background: linear-gradient(145deg, rgba(26, 26, 46, 0.9) 0%, rgba(22, 33, 62, 0.9) 100%);
  border: 1px solid rgba(0, 255, 245, 0.2);
  border-radius: 16px;
  padding: 24px;
  backdrop-filter: blur(10px);
  transition: all 0.3s ease;

  &.hoverable:hover {
    border-color: rgba(0, 255, 245, 0.5);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 0 20px rgba(0, 255, 245, 0.3), 0 0 40px rgba(0, 255, 245, 0.1);
    transform: translateY(-2px);
  }

  &.clickable {
    cursor: pointer;
  }

  &.card-primary {
    border-color: rgba(0, 255, 245, 0.4);
    background: linear-gradient(145deg, rgba(0, 255, 245, 0.1) 0%, rgba(26, 26, 46, 0.9) 100%);
  }

  &.card-secondary {
    border-color: rgba(255, 0, 255, 0.4);
    background: linear-gradient(145deg, rgba(255, 0, 255, 0.1) 0%, rgba(26, 26, 46, 0.9) 100%);
  }

  &.card-success {
    border-color: rgba(0, 255, 136, 0.4);
    background: linear-gradient(145deg, rgba(0, 255, 136, 0.1) 0%, rgba(26, 26, 46, 0.9) 100%);
  }

  &.card-warning {
    border-color: rgba(255, 215, 0, 0.4);
    background: linear-gradient(145deg, rgba(255, 215, 0, 0.1) 0%, rgba(26, 26, 46, 0.9) 100%);
  }

  &.card-danger {
    border-color: rgba(255, 107, 53, 0.4);
    background: linear-gradient(145deg, rgba(255, 107, 53, 0.1) 0%, rgba(26, 26, 46, 0.9) 100%);
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;

    .card-title {
      font-size: 18px;
      font-weight: 600;
      color: white;
      display: flex;
      align-items: center;
      gap: 8px;

      .title-icon {
        width: 20px;
        height: 20px;
        color: #00fff5;
      }
    }

    .card-actions {
      display: flex;
      gap: 8px;
    }
  }

  .card-content {
    color: rgba(255, 255, 255, 0.7);

    &.no-padding {
      padding: 0;
      margin: 0;
    }
  }

  .card-footer {
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid rgba(0, 255, 245, 0.1);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
  }
}
</style>

