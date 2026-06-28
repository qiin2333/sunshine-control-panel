<template>
  <div class="apps-list">
    <div
      v-for="(app, index) in apps"
      :key="'list-' + app.name"
      class="app-list-item"
      :class="{ launching: launchingApp === app.name }"
      tabindex="0"
      @click="$emit('launch', app)"
      @keydown.enter="$emit('launch', app)"
      @contextmenu.prevent="$emit('contextmenu', $event, app)"
    >
      <div class="list-cover">
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
      <div class="list-info">
        <span class="list-name">{{ app.name }}</span>
        <span class="list-cmd" v-if="app.cmd">{{ app.cmd }}</span>
      </div>
      <div class="list-tags">
        <span v-if="isFavorite(app.name)" class="list-tag fav"><StarFilled /> {{ t.appContext.favorite }}</span>
        <span v-if="app.elevated && app.elevated !== 'false'" class="list-tag admin">{{ t.appContext.admin }}</span>
      </div>
      <button class="list-play" tabindex="-1"><VideoPlay /></button>
    </div>
  </div>
</template>

<script setup>
import { useI18n } from '../i18n/index.js'
import { StarFilled, VideoPlay } from '@element-plus/icons-vue'
const { t } = useI18n()

defineProps({
  apps: { type: Array, required: true },
  launchingApp: { type: String, default: null },
  isFavorite: { type: Function, required: true },
  getAppImageUrl: { type: Function, required: true },
  handleImageError: { type: Function, required: true },
})

defineEmits(['launch', 'contextmenu'])
</script>

<style lang="less" scoped>
.apps-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.app-list-item {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 16px;
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
  content-visibility: auto;
  contain-intrinsic-size: auto 68px;

  &:hover, &:focus-visible {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.06);
    transform: translateX(6px) scale(1.01);
    .list-name { color: var(--fd-accent, #00fff5); }
  }

  &:active {
    transform: scale(0.97);
    transition-duration: 0.1s;
  }

  &.launching {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    animation: list-bounce 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .list-cover {
    width: 48px;
    height: 48px;
    border-radius: 8px;
    overflow: hidden;
    background: rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.7);
    flex-shrink: 0;

    img { width: 100%; height: 100%; object-fit: cover; }
  }

  .list-info {
    flex: 1;
    min-width: 0;

    .list-name {
      display: block;
      font-size: 15px;
      font-weight: 500;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.85);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .list-cmd {
      display: block;
      font-size: 12px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  }

  .list-tags {
    display: flex;
    gap: 6px;
  }

  .list-tag {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 4px;
    display: inline-flex;
    align-items: center;
    gap: 4px;

    svg {
      width: 12px;
      height: 12px;
    }

    &.fav { background: rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.1); color: var(--fd-status-warning, #ffd700); }
    &.admin { background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.1); color: var(--fd-status-danger, #ff6b35); }
  }

  .list-play {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
    background: transparent;
    color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.6);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
    flex-shrink: 0;

    svg {
      width: 15px;
      height: 15px;
    }
  }

  &:hover .list-play {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    color: var(--fd-accent, #00fff5);
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.4);
    transform: scale(1.15) rotate(-10deg);
  }
}

@keyframes list-bounce {
  0% { transform: scale(1); }
  30% { transform: scale(0.96) translateX(-4px); }
  60% { transform: scale(1.03) translateX(4px); }
  100% { transform: scale(1) translateX(0); }
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
