<template>
  <div class="recent-section fade-in">
    <div class="section-label">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="section-icon">
        <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
      </svg>
      最近启动
    </div>
    <div class="recent-strip">
      <div
        v-for="app in apps"
        :key="'recent-' + app.name"
        class="recent-tile"
        tabindex="0"
        @click="$emit('launch', app)"
        @keydown.enter="$emit('launch', app)"
        @contextmenu.prevent="$emit('contextmenu', $event, app)"
      >
        <div class="recent-cover">
          <img
            v-if="getAppImageUrl(app)"
            :src="getAppImageUrl(app)"
            :alt="app.name"
            loading="lazy"
            decoding="async"
            @error="handleImageError($event, app)"
          />
          <div v-else class="mini-placeholder">{{ app.name?.[0] || '?' }}</div>
        </div>
        <span class="recent-name">{{ app.name }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  apps: { type: Array, required: true },
  getAppImageUrl: { type: Function, required: true },
  handleImageError: { type: Function, required: true },
})

defineEmits(['launch', 'contextmenu'])
</script>

<style lang="less" scoped>
.recent-section {
  margin-bottom: 28px;
}

.section-label {
  font-size: 14px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);
  margin-bottom: 12px;
  display: flex;
  align-items: center;
  gap: 8px;

  .section-icon {
    width: 16px;
    height: 16px;
  }
}

.recent-strip {
  display: flex;
  gap: 16px;
  overflow-x: auto;
  padding-bottom: 4px;

  &::-webkit-scrollbar { height: 4px; }
  &::-webkit-scrollbar-thumb { background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15); border-radius: 2px; }
}

.recent-tile {
  content-visibility: auto;
  contain-intrinsic-size: auto 112px;
  flex-shrink: 0;
  width: 80px;
  cursor: pointer;
  text-align: center;
  transition: transform 0.2s ease, opacity 0.2s ease;

  &:hover {
    transform: translateY(-3px);
    .recent-name { color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.9); }
  }

  &:active {
    transform: scale(0.95);
  }

  .recent-cover {
    width: 80px;
    height: 80px;
    border-radius: 12px;
    overflow: hidden;
    background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.7);
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
    margin-bottom: 6px;

    img { width: 100%; height: 100%; object-fit: cover; }
  }

  .recent-name {
    font-size: 11px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.mini-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: 700;
  color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
  text-transform: uppercase;
}

.fade-in {
  animation: fadeInUp 0.35s ease both;
}

@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
