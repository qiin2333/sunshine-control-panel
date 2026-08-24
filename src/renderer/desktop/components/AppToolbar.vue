<template>
  <div class="apps-toolbar fade-in">
    <h1 class="apps-title">
      <span class="gradient-text">{{ t.apps.libraryTitle }}</span>
    </h1>

    <!-- 筛选标签 -->
    <div class="filter-tabs">
      <button
        v-for="tab in filterTabs"
        :key="tab.id"
        class="filter-tab"
        :class="{ active: activeFilter === tab.id }"
        tabindex="0"
        :data-focus-key="'filter-' + tab.id"
        @click="$emit('update:activeFilter', tab.id)"
      >
        {{ tab.label }}
        <span v-if="tab.count !== undefined" class="tab-count">{{ tab.count }}</span>
      </button>
    </div>

    <div class="toolbar-spacer"></div>

    <!-- 搜索 -->
    <div class="apps-search" :class="{ focused: searchFocused }">
      <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8"/>
        <line x1="21" y1="21" x2="16.65" y2="16.65"/>
      </svg>
      <input
        :value="searchQuery"
        type="text"
        class="search-input"
        data-focusable
        data-focus-key="apps-search"
        :aria-label="t.apps.searchPlaceholder"
        :placeholder="t.apps.searchPlaceholder"
        @input="$emit('update:searchQuery', $event.target.value)"
        @focus="searchFocused = true"
        @blur="searchFocused = false"
      />
    </div>

    <!-- 排序 -->
    <button class="toolbar-btn" tabindex="0" @click="$emit('cycleSortMode')" :title="`${t.apps.sortTitle}: ${sortLabel}`">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M3 6h18M3 12h12M3 18h6"/>
      </svg>
    </button>

    <!-- 网格大小 -->
    <button class="toolbar-btn" tabindex="0" @click="$emit('cycleGridSize')" :title="`${t.apps.cardSizeTitle}: ${t.apps.gridSizes?.[gridSize] || gridSize}`">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="3" y="3" width="7" height="7" rx="1"/>
        <rect x="14" y="3" width="7" height="7" rx="1"/>
        <rect x="3" y="14" width="7" height="7" rx="1"/>
        <rect x="14" y="14" width="7" height="7" rx="1"/>
      </svg>
    </button>

    <!-- 视图切换 -->
    <button class="toolbar-btn" tabindex="0" @click="$emit('toggleViewMode')" :title="viewMode === 'grid' ? t.apps.listView : t.apps.gridView">
      <svg v-if="viewMode === 'grid'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/>
      </svg>
      <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/>
      </svg>
    </button>

    <div class="apps-count">{{ t.apps.count.replace('{count}', totalCount) }}</div>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

defineProps({
  filterTabs: { type: Array, required: true },
  activeFilter: { type: String, required: true },
  searchQuery: { type: String, required: true },
  sortLabel: { type: String, required: true },
  gridSize: { type: String, required: true },
  viewMode: { type: String, required: true },
  totalCount: { type: Number, required: true },
})

defineEmits([
  'update:activeFilter',
  'update:searchQuery',
  'cycleSortMode',
  'cycleGridSize',
  'toggleViewMode',
])

const searchFocused = ref(false)
</script>

<style lang="less" scoped>
.apps-toolbar {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
  flex-wrap: wrap;
}

.apps-title {
  font-size: 26px;
  font-weight: 700;
  margin: 0;
  white-space: nowrap;

  .gradient-text {
    background: linear-gradient(135deg, var(--fd-accent, #00fff5) 0%, var(--fd-accent-secondary, #ff00ff) 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
}

.filter-tabs {
  display: flex;
  gap: 4px;
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.5);
  border-radius: 10px;
  padding: 3px;
}

.filter-tab {
  padding: 6px 14px;
  border: none;
  background: transparent;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  font-size: 13px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  gap: 6px;

  .tab-count {
    font-size: 11px;
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
    padding: 1px 6px;
    border-radius: 6px;
  }

  &.active {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
    color: var(--fd-accent, #00fff5);

    .tab-count {
      background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
    }
  }

  &:hover:not(.active) {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.8);
  }
}

.toolbar-spacer {
  flex: 1;
}

.apps-search {
  position: relative;
  width: 200px;
  flex-shrink: 0;
  transition: width 0.25s ease;

  &.focused {
    width: 320px;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    width: 16px;
    height: 16px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    padding: 7px 12px 7px 34px;
    background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.6);
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    border-radius: 8px;
    color: var(--fd-text-primary, #fff);
    font-size: 13px;
    outline: none;
    transition: border-color 0.2s ease, box-shadow 0.2s ease;

    &::placeholder { color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25); }
    &:focus {
      border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.35);
      box-shadow: 0 0 12px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
    }
  }
}

.toolbar-btn {
  width: 36px;
  height: 36px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
  background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.5);
  border-radius: 8px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
  padding: 0;

  svg { width: 18px; height: 18px; }

  &:hover {
    color: var(--fd-accent, #00fff5);
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
  }
}

.apps-count {
  font-size: 13px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
  white-space: nowrap;
}

.fade-in {
  animation: fadeInUp 0.35s ease both;
}

@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
