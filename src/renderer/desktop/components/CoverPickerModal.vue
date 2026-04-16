<template>
  <Teleport to="body">
    <Transition name="cover-modal">
      <div v-if="open" class="cover-modal-mask" @click.self="$emit('close')">
        <div class="cover-modal">
          <div class="modal-header">
            <h3><Picture /> 更新封面 — {{ appName }}</h3>
            <button class="modal-close" @click="$emit('close')">✕</button>
          </div>

          <!-- 搜索栏 -->
          <div class="search-bar">
            <input
              v-model="searchQuery"
              class="search-input"
              placeholder="搜索 Steam 游戏..."
              @keyup.enter="doSearch"
            />
            <button class="search-btn" @click="doSearch" :disabled="searching">
              {{ searching ? '搜索中...' : '搜索' }}
            </button>
          </div>

          <!-- 搜索结果 -->
          <div v-if="error" class="search-error">{{ error }}</div>

          <div v-if="candidates.length > 0" class="candidates-grid">
            <div
              v-for="c in candidates"
              :key="c.steam_id"
              class="candidate-card"
              :class="{ selected: selected?.steam_id === c.steam_id, uploading: uploadingId === c.steam_id }"
              @click="selectAndUpload(c)"
            >
              <img :src="c.header_url" class="candidate-img" loading="lazy" />
              <div class="candidate-name">{{ c.name }}</div>
              <div v-if="uploadingId === c.steam_id" class="uploading-overlay">
                <div class="upload-spinner"></div>
              </div>
            </div>
          </div>

          <div v-else-if="!searching && !error && searched" class="no-results">
            未找到匹配结果
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { ref, watch } from 'vue'
import { Picture } from '@element-plus/icons-vue'
import { tauriInvoke } from '../composables/useTauri'

const props = defineProps({
  open: { type: Boolean, required: true },
  appName: { type: String, default: '' },
  proxyUrl: { type: String, default: '' },
})

const emit = defineEmits(['close', 'updated'])

const searchQuery = ref('')
const candidates = ref([])
const selected = ref(null)
const searching = ref(false)
const searched = ref(false)
const error = ref('')
const uploadingId = ref(null)

// 打开时自动用 app 名搜索
watch(() => props.open, (v) => {
  if (v && props.appName) {
    searchQuery.value = props.appName
    doSearch()
  } else if (!v) {
    candidates.value = []
    selected.value = null
    error.value = ''
    searched.value = false
    uploadingId.value = null
  }
})

async function doSearch() {
  if (!searchQuery.value.trim()) return
  searching.value = true
  error.value = ''
  candidates.value = []
  searched.value = false
  selected.value = null

  try {
    candidates.value = await tauriInvoke('search_steam_covers', {
      query: searchQuery.value.trim(),
    })
  } catch (e) {
    error.value = String(e)
  } finally {
    searching.value = false
    searched.value = true
  }
}

async function selectAndUpload(candidate) {
  if (uploadingId.value) return
  selected.value = candidate
  uploadingId.value = candidate.steam_id

  try {
    await tauriInvoke('upload_steam_cover', {
      headerUrl: candidate.header_url,
      appName: props.appName,
      proxyUrl: props.proxyUrl,
    })
    emit('updated', props.appName)
    emit('close')
  } catch (e) {
    error.value = `上传失败: ${e}`
  } finally {
    uploadingId.value = null
  }
}
</script>

<style lang="less" scoped>
.cover-modal-mask {
  position: fixed;
  inset: 0;
  z-index: 10000;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  backdrop-filter: blur(4px);
}

.cover-modal {
  background: rgba(var(--fd-bg-primary-rgb, 20, 20, 40), 0.98);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-radius: 16px;
  width: 680px;
  max-width: 90vw;
  max-height: 80vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 16px 64px rgba(0, 0, 0, 0.5);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);

  h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.9);
  }

  .modal-close {
    background: none;
    border: none;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    font-size: 18px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    &:hover { color: #fff; background: rgba(255, 255, 255, 0.1); }
  }
}

.search-bar {
  display: flex;
  gap: 8px;
  padding: 12px 20px;

  .search-input {
    flex: 1;
    background: rgba(var(--fd-bg-secondary-rgb, 30, 30, 50), 0.8);
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.12);
    border-radius: 8px;
    padding: 8px 12px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.9);
    font-size: 14px;
    outline: none;
    &:focus { border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3); }
  }

  .search-btn {
    padding: 8px 16px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
    border-radius: 8px;
    color: var(--fd-accent, #00fff5);
    font-size: 13px;
    cursor: pointer;
    white-space: nowrap;
    &:hover { background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.25); }
    &:disabled { opacity: 0.5; cursor: default; }
  }
}

.search-error {
  padding: 8px 20px;
  color: #ff6b6b;
  font-size: 13px;
}

.candidates-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  padding: 8px 20px 20px;
  overflow-y: auto;
  max-height: 50vh;
}

.candidate-card {
  position: relative;
  border-radius: 10px;
  overflow: hidden;
  cursor: pointer;
  border: 2px solid transparent;
  transition: border-color 0.15s, transform 0.15s;

  &:hover {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
    transform: scale(1.02);
  }

  &.selected {
    border-color: var(--fd-accent, #00fff5);
  }

  &.uploading {
    pointer-events: none;
    opacity: 0.7;
  }

  .candidate-img {
    width: 100%;
    aspect-ratio: 460 / 215;
    object-fit: cover;
    display: block;
  }

  .candidate-name {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 6px 10px;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.8));
    color: #fff;
    font-size: 12px;
    font-weight: 500;
  }

  .uploading-overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .upload-spinner {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
    border-top-color: var(--fd-accent, #00fff5);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
}

@keyframes spin { to { transform: rotate(360deg); } }

.no-results {
  text-align: center;
  padding: 40px 20px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);
  font-size: 14px;
}

// 弹窗动画
.cover-modal-enter-active { transition: opacity 0.2s ease; }
.cover-modal-leave-active { transition: opacity 0.15s ease; }
.cover-modal-enter-from, .cover-modal-leave-to { opacity: 0; }
.cover-modal-enter-active .cover-modal { transition: transform 0.2s ease; }
.cover-modal-enter-from .cover-modal { transform: scale(0.95); }
</style>
