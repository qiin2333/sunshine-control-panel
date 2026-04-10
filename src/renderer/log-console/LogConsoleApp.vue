<template>
  <div class="log-console">
    <!-- 头部 -->
    <div class="header">
      <div class="title">
        <el-icon class="title-icon"><Document /></el-icon>
        {{ t.logConsole.title }}
      </div>
      <div class="controls">
        <button class="btn" @click="loadLogs">
          <el-icon><RefreshRight /></el-icon>
          {{ t.logConsole.refresh }}
        </button>
        <button class="btn" @click="exportLogs('txt')">
          <el-icon><Download /></el-icon>
          {{ t.logConsole.exportTxt }}
        </button>
        <button class="btn" @click="exportLogs('json')">
          <el-icon><Download /></el-icon>
          {{ t.logConsole.exportJson }}
        </button>
        <button class="btn danger" @click="clearLogs">
          <el-icon><Delete /></el-icon>
          {{ t.logConsole.clear }}
        </button>
        <button class="btn" @click="toggleLocale">
          {{ locale === 'zh' ? 'EN' : '中文' }}
        </button>
      </div>
    </div>

    <!-- 过滤栏 -->
    <div class="filter-bar">
      <div class="filter-group">
        <span class="filter-label">{{ t.logConsole.filterLevel }}:</span>
        <label class="filter-checkbox">
          <input type="checkbox" v-model="filters.error" />
          <span class="filter-label-error">{{ t.logConsole.error }}</span>
        </label>
        <label class="filter-checkbox">
          <input type="checkbox" v-model="filters.warn" />
          <span class="filter-label-warn">{{ t.logConsole.warn }}</span>
        </label>
        <label class="filter-checkbox">
          <input type="checkbox" v-model="filters.info" />
          <span class="filter-label-info">{{ t.logConsole.info }}</span>
        </label>
        <label class="filter-checkbox">
          <input type="checkbox" v-model="filters.debug" />
          <span class="filter-label-debug">{{ t.logConsole.debug }}</span>
        </label>
        <label class="filter-checkbox">
          <input type="checkbox" v-model="filters.trace" />
          <span class="filter-label-trace">{{ t.logConsole.trace }}</span>
        </label>
      </div>
      <div class="filter-group">
        <span class="filter-label">{{ t.logConsole.sourceFile }}:</span>
        <select v-model="filters.file" class="file-filter-select">
          <option value="">{{ t.logConsole.allFiles }}</option>
          <option v-for="file in availableFiles" :key="file" :value="file">{{ file }}</option>
        </select>
      </div>
    </div>

    <!-- 搜索栏 -->
    <div class="search-bar">
      <div class="search-input-wrapper">
        <el-icon class="search-icon"><Search /></el-icon>
        <input
          v-model="searchKeyword"
          type="text"
          class="search-input"
          :placeholder="t.logConsole.searchPlaceholder"
          @input="handleSearchInput"
        />
        <button v-if="searchKeyword" class="search-clear-btn" @click="clearSearch" :title="t.logConsole.clearSearch">
          <el-icon><Close /></el-icon>
        </button>
      </div>
      <div v-if="searchKeyword" class="search-info">
        <span class="search-info-icon">🔍</span>
        {{ t.logConsole.found }} <span class="search-info-count">{{ filteredLogs.length }}</span> {{ t.logConsole.matchResults }}
      </div>
    </div>

    <!-- 日志容器 -->
    <div class="log-container" ref="logContainer">
      <div v-if="filteredLogs.length === 0" class="empty-state">
        <div class="empty-state-icon-wrapper">
          <el-icon class="empty-state-icon" :size="56"><Document /></el-icon>
          <div class="sparkle sparkle-1">✨</div>
          <div class="sparkle sparkle-2">✨</div>
          <div class="sparkle sparkle-3">✨</div>
        </div>
        <div class="empty-state-text">
          {{ loading ? t.logConsole.loadingLogs : searchKeyword ? t.logConsole.noMatch : t.logConsole.noLogs }}
        </div>
      </div>
      <div v-for="log in filteredLogs" :key="`${log.timestamp}-${log.message}`" :class="['log-entry', log.level]">
        <span class="log-timestamp">{{ log.timestamp }}</span>
        <span :class="['log-level', log.level]">{{ log.level }}</span>
        <span v-if="log.file" class="log-source">{{ log.file }}<span v-if="log.line">:{{ log.line }}</span></span>
        <span class="log-message" v-html="highlightMessage(log.message)"></span>
      </div>
    </div>

    <!-- 统计栏 -->
    <div class="stats">
      <div class="stat-item">
        <span>{{ t.logConsole.total }}:</span>
        <span class="stat-value">{{ stats.total }}</span>
      </div>
      <div class="stat-item">
        <span class="stat-label-error">{{ t.logConsole.error }}:</span>
        <span class="stat-value">{{ stats.error }}</span>
      </div>
      <div class="stat-item">
        <span class="stat-label-warn">{{ t.logConsole.warn }}:</span>
        <span class="stat-value">{{ stats.warn }}</span>
      </div>
      <div class="stat-item">
        <span class="stat-label-info">{{ t.logConsole.info }}:</span>
        <span class="stat-value">{{ stats.info }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Document, RefreshRight, Delete, Search, Close, Download } from '@element-plus/icons-vue'
import { useI18n } from '../desktop/i18n/index.js'

const { t, locale, toggleLocale } = useI18n()

// 响应式数据
const allLogs = ref([])
const loading = ref(false)
const logContainer = ref(null)
const searchKeyword = ref('')

// 过滤器
const filters = ref({
  error: true,
  warn: true,
  info: true,
  debug: false,
  trace: false,
  file: '', // 文件来源过滤
})

// 计算属性：获取所有可用的文件来源
const availableFiles = computed(() => {
  const files = new Set()
  allLogs.value.forEach((log) => {
    if (log.file) {
      files.add(log.file)
    }
  })
  return Array.from(files).sort()
})

// 计算属性：过滤后的日志（同时考虑级别、文件来源和关键词）
const filteredLogs = computed(() => {
  const enabledLevels = Object.entries(filters.value)
    .filter(([key, enabled]) => key !== 'file' && enabled)
    .map(([level, _]) => level)

  let filtered = allLogs.value.filter((log) => enabledLevels.includes(log.level))

  // 如果选择了文件来源，进行文件过滤
  if (filters.value.file) {
    filtered = filtered.filter((log) => log.file === filters.value.file)
  }

  // 如果有关键词，进行搜索过滤
  if (searchKeyword.value.trim()) {
    const keyword = searchKeyword.value.trim().toLowerCase()
    filtered = filtered.filter((log) => {
      const message = log.message.toLowerCase()
      const timestamp = log.timestamp.toLowerCase()
      const level = log.level.toLowerCase()
      const file = (log.file || '').toLowerCase()
      return message.includes(keyword) || timestamp.includes(keyword) || level.includes(keyword) || file.includes(keyword)
    })
  }

  return filtered
})

// 高亮消息中的关键词
function highlightMessage(message) {
  if (!searchKeyword.value.trim()) {
    return escapeHtml(message)
  }

  const keyword = searchKeyword.value.trim()
  const regex = new RegExp(`(${escapeRegex(keyword)})`, 'gi')
  const highlighted = message.replace(regex, '<mark class="highlight">$1</mark>')
  return highlighted
}

// HTML 转义
function escapeHtml(text) {
  const div = document.createElement('div')
  div.textContent = text
  return div.innerHTML
}

// 正则表达式转义
function escapeRegex(str) {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

// 处理搜索输入
function handleSearchInput() {
  // 搜索时自动滚动到第一个匹配项
  nextTick(() => {
    if (logContainer.value && filteredLogs.value.length > 0) {
      const firstMatch = logContainer.value.querySelector('.log-entry')
      if (firstMatch) {
        firstMatch.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
      }
    }
  })
}

// 清除搜索
function clearSearch() {
  searchKeyword.value = ''
}

// 计算属性：统计信息
const stats = computed(() => {
  return {
    total: allLogs.value.length,
    error: allLogs.value.filter((l) => l.level === 'error').length,
    warn: allLogs.value.filter((l) => l.level === 'warn').length,
    info: allLogs.value.filter((l) => l.level === 'info').length,
  }
})

// 加载所有日志
async function loadLogs() {
  loading.value = true
  try {
    const logs = await invoke('get_all_logs')
    allLogs.value = logs.reverse() // 最新的在前
  } catch (error) {
    console.error('Failed to load logs:', error)
  } finally {
    loading.value = false
  }
}

// 清空日志
async function clearLogs() {
  if (await confirm(t.value.logConsole.confirmClear)) {
    try {
      await invoke('clear_logs')
      allLogs.value = []
    } catch (error) {
      console.error('Failed to clear logs:', error)
      alert(t.value.logConsole.clearFailed + ': ' + error)
    }
  }
}

// 导出日志
async function exportLogs(format) {
  try {
    const result = await invoke('export_logs', { format })
    alert(result || t.value.logConsole.exportSuccess)
  } catch (error) {
    console.error('Failed to export logs:', error)
    if (error && !error.includes('用户取消了保存')) {
      alert(t.value.logConsole.exportFailed + ': ' + error)
    }
  }
}

// 滚动到底部
function scrollToBottom() {
  nextTick(() => {
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight
    }
  })
}

// 监听新日志事件
let unsubscribe = null

onMounted(async () => {
  // 初始加载
  await loadLogs()
  scrollToBottom()

  // 监听新日志事件
  unsubscribe = await listen('log-entry', (event) => {
    const newLog = event.payload
    allLogs.value.unshift(newLog) // 添加到开头

    // 限制日志数量
    if (allLogs.value.length > 10000) {
      allLogs.value = allLogs.value.slice(0, 10000)
    }

    scrollToBottom()
  })
})

onUnmounted(() => {
  if (unsubscribe) {
    unsubscribe()
  }
})
</script>

<style scoped lang="less">
@import './LogConsoleApp.less';
</style>
