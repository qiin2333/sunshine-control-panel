<template>
  <div class="chub-tabbar" role="tablist">
    <button
      v-for="(opt, index) in options"
      :key="opt.value"
      :ref="(element) => setButtonRef(element, index)"
      type="button"
      role="tab"
      class="chub-tab"
      :class="{ 'is-active': modelValue === opt.value }"
      :aria-selected="modelValue === opt.value"
      :tabindex="modelValue === opt.value ? 0 : -1"
      @click="emit('update:modelValue', opt.value)"
      @keydown="handleKeydown($event, index)"
    >{{ opt.label }}</button>
  </div>
</template>

<script setup>
import { nextTick, ref } from 'vue'

const props = defineProps({
  modelValue: { type: [Number, String], required: true },
  options: { type: Array, required: true },
})
const emit = defineEmits(['update:modelValue'])
const buttons = ref([])

function setButtonRef(element, index) {
  if (element) buttons.value[index] = element
}

async function selectAt(index) {
  const option = props.options[index]
  if (!option) return
  emit('update:modelValue', option.value)
  await nextTick()
  buttons.value[index]?.focus()
}

function handleKeydown(event, index) {
  const last = props.options.length - 1
  let next = null
  if (event.key === 'ArrowRight') next = index === last ? 0 : index + 1
  else if (event.key === 'ArrowLeft') next = index === 0 ? last : index - 1
  else if (event.key === 'Home') next = 0
  else if (event.key === 'End') next = last
  if (next === null) return
  event.preventDefault()
  selectAt(next)
}
</script>
