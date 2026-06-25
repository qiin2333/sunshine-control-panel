<template>
  <SettingsRow
    class="tool-path-item"
    name-class="tool-path-name"
    :description="helper.description"
  >
    <template #name>
      <LaunchHelperIcon :template-id="helper.id" :size="18" />
      <span>{{ helper.name }}</span>
    </template>

    <div
      v-for="param in pathParams"
      :key="param.key"
      class="tool-path-row"
    >
      <input
        type="text"
        class="path-input"
        :placeholder="param.placeholder"
        :value="getPath(helper.id, param.key)"
        @input="emit('update-path', helper.id, param.key, $event.target.value)"
      />
      <button
        v-if="hasTauri"
        type="button"
        class="browse-btn-small"
        @click="emit('browse', helper.id, param.key)"
      >
        <FolderOpened />
      </button>
    </div>
  </SettingsRow>
</template>

<script setup>
import { computed } from 'vue'
import { FolderOpened } from '@element-plus/icons-vue'
import LaunchHelperIcon from '../LaunchHelperIcon.vue'
import SettingsRow from './SettingsRow.vue'

const props = defineProps({
  helper: {
    type: Object,
    required: true,
  },
  hasTauri: {
    type: Boolean,
    required: true,
  },
  getPath: {
    type: Function,
    required: true,
  },
})

const emit = defineEmits(['update-path', 'browse'])

const pathParams = computed(() => props.helper.params.filter(param => param.key === 'path'))
</script>

<style lang="less" scoped>
.tool-path-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.path-input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 13px;
  font-family: 'Consolas', 'Monaco', monospace;

  &::placeholder {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25);
  }

  &:focus {
    outline: none;
    border-color: var(--fd-accent, #00fff5);
  }
}

.browse-btn-small {
  padding: 7px 10px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
  flex-shrink: 0;

  &:hover {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    border-color: var(--fd-accent, #00fff5);
  }
}
</style>
