<template>
  <label class="settings-switch">
    <input
      type="checkbox"
      :checked="modelValue"
      @change="handleChange"
    />
    <span class="slider"></span>
  </label>
</template>

<script setup>
defineProps({
  modelValue: {
    type: Boolean,
    required: true,
  },
})

const emit = defineEmits(['update:modelValue', 'change'])

function handleChange(event) {
  const nextValue = event.target.checked
  emit('update:modelValue', nextValue)
  emit('change', nextValue)
}
</script>

<style lang="less" scoped>
.settings-switch {
  position: relative;
  display: inline-block;
  width: 48px;
  height: 26px;

  input {
    opacity: 0;
    width: 0;
    height: 0;

    &:checked + .slider {
      background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);

      &::before {
        transform: translateX(22px);
      }
    }
  }
}

.slider {
  position: absolute;
  cursor: pointer;
  inset: 0;
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
  border-radius: 26px;
  transition: 0.3s;

  &::before {
    position: absolute;
    content: "";
    height: 20px;
    width: 20px;
    left: 3px;
    bottom: 3px;
    background: white;
    border-radius: 50%;
    transition: 0.3s;
  }
}
</style>
