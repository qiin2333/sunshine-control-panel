<template>
  <div class="desktop-grid" :class="[`cols-${cols}`, `gap-${gap}`, { responsive: responsive }]">
    <slot></slot>
  </div>
</template>

<script setup>
const props = defineProps({
  cols: {
    type: Number,
    default: 2,
    validator: (value) => [1, 2, 3, 4, 5, 6].includes(value),
  },
  gap: {
    type: String,
    default: 'md', // xs, sm, md, lg, xl
    validator: (value) => ['xs', 'sm', 'md', 'lg', 'xl'].includes(value),
  },
  responsive: {
    type: Boolean,
    default: true,
  },
})
</script>

<style lang="less" scoped>
.desktop-grid {
  display: grid;
  gap: 24px;

  // 列数
  &.cols-1 {
    grid-template-columns: 1fr;
  }

  &.cols-2 {
    grid-template-columns: repeat(2, 1fr);
  }

  &.cols-3 {
    grid-template-columns: repeat(3, 1fr);
  }

  &.cols-4 {
    grid-template-columns: repeat(4, 1fr);
  }

  &.cols-5 {
    grid-template-columns: repeat(5, 1fr);
  }

  &.cols-6 {
    grid-template-columns: repeat(6, 1fr);
  }

  // 间距
  &.gap-xs {
    gap: 8px;
  }

  &.gap-sm {
    gap: 16px;
  }

  &.gap-md {
    gap: 24px;
  }

  &.gap-lg {
    gap: 32px;
  }

  &.gap-xl {
    gap: 48px;
  }

  // 响应式
  &.responsive {
    @media (max-width: 1400px) {
      &.cols-6 {
        grid-template-columns: repeat(3, 1fr);
      }
      &.cols-5 {
        grid-template-columns: repeat(3, 1fr);
      }
    }

    @media (max-width: 1200px) {
      &.cols-4 {
        grid-template-columns: repeat(2, 1fr);
      }
      &.cols-3 {
        grid-template-columns: repeat(2, 1fr);
      }
    }

    @media (max-width: 800px) {
      &.cols-2,
      &.cols-3,
      &.cols-4,
      &.cols-5,
      &.cols-6 {
        grid-template-columns: 1fr;
      }
    }
  }
}
</style>
