<template>
  <div class="custom-dropdown" :class="{ open }" ref="root">
    <button class="dropdown-trigger" @click="open = !open">
      <span>{{ displayText }}</span>
      <span class="dropdown-arrow">▾</span>
    </button>
    <Transition name="dropdown">
      <div class="dropdown-menu" v-if="open">
        <div 
          v-for="opt in options" 
          :key="opt.value"
          class="dropdown-item"
          :class="{ active: modelValue === opt.value }"
          @click="select(opt.value)"
        >{{ opt.label }}</div>
      </div>
    </Transition>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'

const props = defineProps({
  modelValue: { type: [Number, String], required: true },
  options: { type: Array, required: true },
  placeholder: { type: String, default: '请选择' },
})

const emit = defineEmits(['update:modelValue'])

const open = ref(false)
const root = ref(null)

const displayText = computed(() => {
  const found = props.options.find(o => o.value === props.modelValue)
  return found ? found.label : props.placeholder
})

function select(value) {
  emit('update:modelValue', value)
  open.value = false
}

function onClickOutside(e) {
  if (root.value && !root.value.contains(e.target)) {
    open.value = false
  }
}

onMounted(() => document.addEventListener('click', onClickOutside))
onUnmounted(() => document.removeEventListener('click', onClickOutside))
</script>

<style lang="less" scoped>
.custom-dropdown {
  position: relative;
  flex-shrink: 0;
}

.dropdown-trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-radius: 8px;
  background: transparent;
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  cursor: pointer;
  min-width: 140px;
  transition: all 0.2s ease;

  &:hover {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
  }

  .dropdown-arrow {
    margin-left: auto;
    opacity: 0.4;
    font-size: 12px;
  }
}

.custom-dropdown.open .dropdown-trigger {
  border-color: var(--fd-accent, #00fff5);
}

.dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  min-width: 100%;
  background: var(--fd-bg-secondary, #1a1a2e);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 10px;
  padding: 4px;
  z-index: 100;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

.dropdown-enter-active {
  animation: dropdownIn 0.15s ease;
}
.dropdown-leave-active {
  animation: dropdownIn 0.15s ease reverse;
}

@keyframes dropdownIn {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}

.dropdown-item {
  padding: 10px 14px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.7);
  transition: all 0.15s ease;
  white-space: nowrap;

  &:hover {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
    color: var(--fd-text-primary, #fff);
  }

  &.active {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.12);
    color: var(--fd-accent, #00fff5);
  }
}
</style>
