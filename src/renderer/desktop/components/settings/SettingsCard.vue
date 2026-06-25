<template>
  <section class="desktop-card fade-in settings-card">
    <div class="card-header">
      <div class="card-title">
        <span class="title-icon">
          <component :is="icon" />
        </span>
        {{ title }}
      </div>
      <div v-if="$slots.actions" class="card-actions">
        <slot name="actions"></slot>
      </div>
    </div>
    <div class="card-content">
      <slot></slot>
    </div>
  </section>
</template>

<script setup>
defineProps({
  title: {
    type: String,
    required: true,
  },
  icon: {
    type: [Object, Function, String],
    required: true,
  },
})
</script>

<style lang="less" scoped>
.settings-card {
  margin-bottom: 22px;
}
</style>
