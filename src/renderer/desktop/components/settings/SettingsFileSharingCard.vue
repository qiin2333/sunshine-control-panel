<template>
  <SettingsCard title="文件夹共享" :icon="FolderOpened">
    <SettingsRow name="共享状态" description="右键共享使用只读权限，仅允许已配对设备访问">
      <div class="sharing-status">
        <span class="status-indicator" :class="statusClass"></span>
        <span>{{ statusText }}</span>
      </div>
    </SettingsRow>

    <SettingsRow name="共享文件夹" description="选择主机上的文件夹后，Moonlight 可在串流中读取">
      <div class="sharing-control-group">
        <button class="desktop-btn icon-btn" :disabled="refreshDisabled" title="刷新" @click="loadMappings">
          <Refresh />
        </button>
        <button class="desktop-btn primary" :disabled="actionDisabled" @click="addFolder">
          <Plus />
          添加
        </button>
      </div>
    </SettingsRow>

    <SettingsRow name="资源管理器右键菜单" description="在文件夹右键菜单中显示“通过 Sunshine 共享”">
      <div class="sharing-control-group">
        <button class="desktop-btn compact" :disabled="actionDisabled" @click="installMenu">
          <Link />
          启用
        </button>
        <button class="desktop-btn compact" :disabled="actionDisabled" @click="uninstallMenu">
          关闭
        </button>
      </div>
    </SettingsRow>

    <div class="sharing-policy">
      <span class="share-chip safe">只读</span>
      <span class="share-chip safe">仅已配对设备</span>
      <span class="share-chip safe">阻止链接穿透</span>
    </div>

    <Transition name="notice">
      <div v-if="notice.text" class="sharing-notice" :class="notice.type">
        <span>{{ notice.text }}</span>
        <button class="notice-close" title="关闭" @click="clearNotice">×</button>
      </div>
    </Transition>

    <div v-if="error" class="sharing-error">
      {{ error }}
    </div>

    <div v-if="!runtimeChecked || loading" class="sharing-empty">
      {{ runtimeChecked ? '正在读取共享列表' : '正在检测运行环境' }}
    </div>

    <div v-else-if="!hasTauri" class="sharing-empty">
      <div class="empty-title">文件夹共享需要在桌面端使用</div>
    </div>

    <div v-else-if="mappings.length === 0" class="sharing-empty">
      <div class="empty-title">还没有共享文件夹</div>
      <div class="empty-actions">
        <button class="desktop-btn primary" :disabled="actionDisabled" @click="addFolder">
          <Plus />
          选择文件夹
        </button>
      </div>
    </div>

    <div v-else class="sharing-list">
      <div v-for="mapping in mappings" :key="mapping.id" class="sharing-row">
        <div class="share-main">
          <div class="share-name">{{ mapping.name || mapping.id }}</div>
          <div class="share-path" :title="mapping.path">{{ mapping.path }}</div>
        </div>
        <div class="share-meta">
          <span class="share-chip">{{ mapping.mode === 'readwrite' ? '读写' : '只读' }}</span>
          <span class="share-chip">{{ clientLabel(mapping) }}</span>
          <span v-if="!mapping.follow_reparse_points" class="share-chip safe">阻止链接穿透</span>
        </div>
        <button
          class="desktop-btn danger icon-btn"
          :disabled="actionDisabled"
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
import { isTauriRuntime } from '../../composables/useTauri.js'
import { fileMapping } from '../../../tauri-adapter.js'
import SettingsCard from './SettingsCard.vue'
import SettingsRow from './SettingsRow.vue'

const mappings = ref([])
const loading = ref(false)
const busy = ref(false)
const runtimeChecked = ref(false)
const hasTauri = ref(false)
const error = ref('')
const notice = ref({ type: 'success', text: '' })

const canUseSharing = computed(() => runtimeChecked.value && hasTauri.value)
const actionDisabled = computed(() => busy.value || !canUseSharing.value)
const refreshDisabled = computed(() => loading.value || !canUseSharing.value)

const statusClass = computed(() => {
  if (!runtimeChecked.value || loading.value) return 'connecting'
  if (!hasTauri.value) return 'offline'
  return mappings.value.length > 0 ? 'online' : 'offline'
})

const statusText = computed(() => {
  if (!runtimeChecked.value) return '检测中'
  if (!hasTauri.value) return '仅桌面端可用'
  if (loading.value) return '同步中'
  if (mappings.value.length === 0) return '未共享'
  return `${mappings.value.length} 个共享`
})

async function loadMappings() {
  if (!canUseSharing.value) {
    mappings.value = []
    loading.value = false
    error.value = ''
    return
  }

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
  if (actionDisabled.value) return
  busy.value = true
  error.value = ''
  try {
    const path = await open({
      directory: true,
      multiple: false,
    })
    if (!path) return
    const mapping = await fileMapping.quickShareFolder(path)
    await loadMappings()
    showNotice('success', `已共享“${mapping?.name || folderName(path)}”，Moonlight 可只读访问`)
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

async function removeMapping(mapping) {
  if (!mapping?.id || actionDisabled.value) return
  if (!confirm(`撤销共享“${mapping.name || mapping.id}”？`)) return

  busy.value = true
  error.value = ''
  try {
    await fileMapping.remove(mapping.id)
    mappings.value = mappings.value.filter(item => item.id !== mapping.id)
    showNotice('success', `已撤销“${mapping.name || mapping.id}”`)
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

async function installMenu() {
  if (actionDisabled.value) return
  busy.value = true
  error.value = ''
  try {
    await fileMapping.installMenu()
    showNotice('success', '已添加资源管理器右键共享入口')
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

async function uninstallMenu() {
  if (actionDisabled.value) return
  busy.value = true
  error.value = ''
  try {
    await fileMapping.uninstallMenu()
    showNotice('success', '已移除资源管理器右键共享入口')
  } catch (err) {
    error.value = friendlyError(err)
  } finally {
    busy.value = false
  }
}

function clientLabel(mapping) {
  return mapping.clients?.length ? `${mapping.clients.length} 台设备` : '所有已配对设备'
}

function folderName(path) {
  return String(path || '')
    .split(/[\\/]/)
    .filter(Boolean)
    .pop() || '文件夹'
}

function showNotice(type, text) {
  notice.value = { type, text }
}

function clearNotice() {
  notice.value = { type: 'success', text: '' }
}

function friendlyError(err) {
  const text = String(err || '').trim()
  if (!text) return '操作失败'
  if (text.includes('Connection') || text.includes('connection') || text.includes('refused')) {
    return '无法连接 Sunshine，请确认 Sunshine 正在运行'
  }
  if (text.includes('not a folder')) return '请选择一个文件夹'
  if (text.includes('does not exist') || text.includes('cannot be accessed')) return '文件夹不存在或无法访问'
  if (text.includes('readwrite mode is not supported')) return '当前阶段仅支持只读共享'
  if (text.includes('allow_delete')) return '当前阶段不允许远端删除文件'
  if (text.includes('follow_reparse_points')) return '当前阶段不允许穿透符号链接或 junction'
  return text
}

onMounted(async () => {
  hasTauri.value = await isTauriRuntime()
  runtimeChecked.value = true
  if (hasTauri.value) {
    await loadMappings()
  }
})
</script>

<style lang="less" scoped>
.sharing-status,
.sharing-control-group {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.sharing-status {
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.78);
  justify-content: flex-end;
  white-space: nowrap;
}

.sharing-control-group {
  justify-content: flex-end;
  flex-wrap: wrap;
}

.sharing-control-group .desktop-btn:not(.icon-btn) {
  justify-content: center;
  min-width: 76px;
  white-space: nowrap;
}

.sharing-policy {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 10px 0 14px;
}

:deep(.setting-info) {
  min-width: 0;
  padding-right: 18px;
}

:deep(.setting-control) {
  flex: 0 0 auto;
  max-width: 48%;
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

.sharing-notice {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 14px;
  line-height: 1.45;

  &.success {
    color: var(--fd-status-success, #00ff88);
    background: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.1);
    border: 1px solid rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.24);
  }
}

.notice-close {
  border: 0;
  background: transparent;
  color: currentColor;
  cursor: pointer;
  font-size: 18px;
  line-height: 1;
  opacity: 0.8;
  padding: 0 2px;
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
  padding: 16px;
  text-align: center;
}

.empty-title {
  margin-bottom: 12px;
}

.empty-actions {
  display: flex;
  justify-content: center;
  flex-wrap: wrap;
  gap: 10px;
}

.sharing-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sharing-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(180px, auto) 40px;
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
  min-width: 0;
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

.notice-enter-active,
.notice-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.notice-enter-from,
.notice-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

@media (max-width: 760px) {
  :deep(.setting-item) {
    align-items: flex-start;
    flex-direction: column;
    gap: 12px;
  }

  :deep(.setting-info) {
    padding-right: 0;
  }

  :deep(.setting-control) {
    width: 100%;
    max-width: none;
  }

  .sharing-status,
  .sharing-control-group {
    justify-content: flex-start;
  }

  .sharing-control-group {
    width: 100%;
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
