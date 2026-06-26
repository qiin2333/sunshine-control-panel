<template>
  <SettingsCard title="文件夹共享" :icon="FolderOpened">
    <template #actions>
      <button class="desktop-btn icon-btn" :disabled="loading" title="刷新" @click="loadMappings">
        <Refresh />
      </button>
      <button class="desktop-btn primary" :disabled="busy" @click="addFolder">
        <Plus />
        添加文件夹
      </button>
    </template>

    <div class="sharing-toolbar">
      <div class="sharing-status">
        <span class="status-indicator" :class="mappings.length > 0 ? 'online' : 'connecting'"></span>
        <span>{{ statusText }}</span>
      </div>
      <div class="sharing-actions">
        <button class="desktop-btn compact" :disabled="busy" @click="installMenu">
          <Link />
          添加右键菜单
        </button>
        <button class="desktop-btn compact" :disabled="busy" @click="uninstallMenu">
          移除右键菜单
        </button>
      </div>
    </div>

    <div v-if="error" class="sharing-error">
      {{ error }}
    </div>

    <div v-if="loading" class="sharing-empty">
      正在读取共享列表
    </div>

    <div v-else-if="mappings.length === 0" class="sharing-empty">
      还没有共享文件夹
    </div>

    <div v-else class="sharing-list">
      <div v-for="mapping in mappings" :key="mapping.id" class="sharing-row">
        <div class="share-main">
          <div class="share-name">{{ mapping.name || mapping.id }}</div>
          <div class="share-path" :title="mapping.path">{{ mapping.path }}</div>
        </div>
        <div class="share-meta">
          <span class="share-chip">{{ mapping.mode === 'readwrite' ? '读写' : '只读' }}</span>
          <span class="share-chip">{{ mapping.clients?.length ? `${mapping.clients.length} 台设备` : '已配对设备' }}</span>
          <span v-if="!mapping.follow_reparse_points" class="share-chip safe">阻止链接穿透</span>
        </div>
        <button
          class="desktop-btn danger icon-btn"
          :disabled="busy"
          title="撤销共享"
          @click="removeMapping(mapping)"
        >
          <Delete />
        </button>
      </div>
    </div>
  </SettingsCard>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { Delete, FolderOpened, Link, Plus, Refresh } from '@element-plus/icons-vue'
import { open } from '@tauri-apps/plugin-dialog'
import { fileMapping } from '../../../tauri-adapter.js'
import SettingsCard from './SettingsCard.vue'

const mappings = ref([])
const loading = ref(false)
const busy = ref(false)
const error = ref('')

const statusText = computed(() => {
  if (loading.value) return '同步中'
  if (mappings.value.length === 0) return '未共享'
  return `${mappings.value.length} 个共享`
})

async function loadMappings() {
  loading.value = true
  error.value = ''
  try {
    mappings.value = await fileMapping.list()
  } catch (err) {
    mappings.value = []
    error.value = friendlyError(err)
  } finally {
    loading.value = false
  }
}

async function addFolder() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  try {
    const path = await open({
      directory: true,
      multiple: false,
    })
    if (!path) return
    await fileMapping.quickShareFolder(path)
    await loadMappings()
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

async function removeMapping(mapping) {
  if (!mapping?.id || busy.value) return
  if (!confirm(`撤销共享“${mapping.name || mapping.id}”？`)) return

  busy.value = true
  error.value = ''
  try {
    await fileMapping.remove(mapping.id)
    mappings.value = mappings.value.filter(item => item.id !== mapping.id)
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

async function installMenu() {
  busy.value = true
  error.value = ''
  try {
    await fileMapping.installMenu()
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

async function uninstallMenu() {
  busy.value = true
  error.value = ''
  try {
    await fileMapping.uninstallMenu()
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

function friendlyError(err) {
  const text = String(err || '').trim()
  if (!text) return '操作失败'
  if (text.includes('Connection') || text.includes('connection') || text.includes('refused')) {
    return '无法连接 Sunshine，请确认 Sunshine 正在运行'
  }
  return text
}

onMounted(loadMappings)
</script>

<style lang="less" scoped>
.sharing-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  margin-bottom: 16px;
}

.sharing-status,
.sharing-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.desktop-btn {
  &:disabled {
    opacity: 0.55;
    cursor: default;
    transform: none;
    box-shadow: none;
  }
}

.compact {
  padding: 8px 12px;
}

.icon-btn {
  width: 40px;
  height: 40px;
  padding: 0;
  justify-content: center;

  svg {
    width: 18px;
    height: 18px;
  }
}

.sharing-error {
  color: var(--fd-status-danger, #ff6b35);
  background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.1);
  border: 1px solid rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.24);
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 14px;
  line-height: 1.45;
}

.sharing-empty {
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  border: 1px dashed rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  padding: 18px;
  text-align: center;
}

.sharing-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sharing-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto 40px;
  align-items: center;
  gap: 14px;
  padding: 12px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
  border-radius: 8px;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.2);
}

.share-main {
  min-width: 0;
}

.share-name {
  color: var(--fd-text-primary, #fff);
  font-weight: 600;
  margin-bottom: 4px;
}

.share-path {
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.52);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.share-meta {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.share-chip {
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.75);
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.08);
  border-radius: 999px;
  padding: 4px 9px;
  font-size: 12px;
  white-space: nowrap;

  &.safe {
    color: var(--fd-status-success, #00ff88);
    background: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.1);
  }
}

@media (max-width: 760px) {
  .sharing-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .sharing-actions {
    flex-wrap: wrap;
  }

  .sharing-row {
    grid-template-columns: minmax(0, 1fr) 40px;
  }

  .share-meta {
    grid-column: 1 / -1;
    justify-content: flex-start;
  }
}
</style>
