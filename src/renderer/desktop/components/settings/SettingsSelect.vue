<template>
  <select class="settings-select" :value="modelValue" @change="handleChange">
    <option
      v-for="option in options"
      :key="String(option.value)"
      :value="option.value"
    >
      {{ option.label }}
    </option>
  </select>
</template>

<script setup>
const props = defineProps({
  modelValue: {
    type: [String, Number],
    required: true,
  },
  options: {
    type: Array,
    required: true,
  },
})

const emit = defineEmits(['update:modelValue', 'change'])

function handleChange(event) {
  const selected = props.options.find(option => String(option.value) === event.target.value)
  const nextValue = selected ? selected.value : event.target.value
  emit('update:modelValue', nextValue)
  emit('change', nextValue)
}
</script>

<style lang="less" scoped>
.settings-select {
  padding: 8px 32px 8px 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2300fff5' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;

  &:focus {
    outline: none;
    border-color: var(--fd-accent, #00fff5);
  }

  option {
    background: var(--fd-bg-secondary, #1a1a2e);
    color: var(--fd-text-primary, #fff);
  }
}
</style>
