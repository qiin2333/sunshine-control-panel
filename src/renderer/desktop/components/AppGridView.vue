<template>
  <div class="apps-grid" :class="'grid-' + gridSize">
    <div
      v-for="(app, index) in apps"
      :key="app.name"
      class="app-tile fade-in"
      :class="{ launching: launchingApp === app.name, favorited: isFavorite(app.name) }"
      tabindex="0"
      :data-app-name="app.name"
      :data-focus-key="'app-' + app.name"
      :style="{ animationDelay: `${Math.min(index * 0.03, 0.4)}s` }"
      @click="$emit('launch', app)"
      @keydown.enter="$emit('launch', app)"
      @contextmenu.prevent="$emit('contextmenu', $event, app)"
    >
      <div class="tile-cover">
        <img
          v-if="getAppImageUrl(app)"
          :src="getAppImageUrl(app)"
          :alt="app.name"
          class="cover-image"
          loading="lazy"
          decoding="async"
          @error="handleImageError($event, app)"
        />
        <div v-else class="cover-placeholder">
          <span class="placeholder-letter">{{ app.name?.[0] || '?' }}</span>
        </div>
        <!-- 收藏角标 -->
        <div v-if="isFavorite(app.name)" class="favorite-badge" @click.stop="$emit('toggleFavorite', app.name)">
          <StarFilled />
        </div>
        <!-- 启动助手角标 -->
        <div v-if="helperIds(app.name).length" class="helper-badges">
          <LaunchHelperIcon
            v-for="helperId in helperIds(app.name)"
            :key="helperId"
            :template-id="helperId"
            :size="12"
            class="helper-badge-icon"
          />
        </div>
        <!-- Hover 遮罩 -->
        <div class="tile-overlay">
          <span class="play-icon"><VideoPlay /></span>
        </div>
        <!-- 启动动画 -->
        <div v-if="launchingApp === app.name" class="launch-overlay">
          <div class="launch-spinner"></div>
          <span>{{ t.appContext.launching }}</span>
        </div>
      </div>
      <div class="tile-info">
        <span class="tile-name" :title="app.name">{{ app.name }}</span>
        <span v-if="playtimeLabel(app.name)" class="tile-playtime">{{ playtimeLabel(app.name) }}</span>
        <span v-if="app.elevated && app.elevated !== 'false'" class="tile-badge admin">{{ t.appContext.admin }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useI18n } from '../i18n/index.js'
import { StarFilled, VideoPlay } from '@element-plus/icons-vue'
import LaunchHelperIcon from './LaunchHelperIcon.vue'
import { formatDuration } from '../composables/useGameSession.js'
const { t } = useI18n()

const props = defineProps({
  apps: { type: Array, required: true },
  gridSize: { type: String, required: true },
  launchingApp: { type: String, default: null },
  isFavorite: { type: Function, required: true },
  getAppImageUrl: { type: Function, required: true },
  handleImageError: { type: Function, required: true },
  helperIds: { type: Function, default: () => [] },
  stats: { type: Object, default: () => ({}) },
})

defineEmits(['launch', 'contextmenu', 'toggleFavorite'])

/** 累计时长不足一分钟就不显示，避免失败的启动留下 "0m" 噪声。 */
function playtimeLabel(appName) {
  const seconds = Number(props.stats?.[appName]?.totalSeconds) || 0
  if (seconds < 60) return ''
  return t.value.gameSession.playedFor.replace('{duration}', formatDuration(seconds))
}
</script>

<style lang="less" scoped>
.apps-grid {
  display: grid;
  gap: 20px;

  &.grid-small  { grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); }
  &.grid-medium { grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); }
  &.grid-large  { grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); }
}

.app-tile {
  cursor: pointer;
  transition: transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1), filter 0.3s ease;
  border-radius: 12px;
  position: relative;
  content-visibility: auto;
  contain-intrinsic-size: auto 280px;
  will-change: transform;

  &:hover, &:focus-visible {
    transform: scale(1.06) rotate(-0.8deg);
    filter: brightness(1.1);
    .tile-cover { box-shadow: 0 10px 30px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.22), 0 0 0 2px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15); }
    .tile-overlay { opacity: 1; }
    .cover-image { transform: scale(1.08); }
    .tile-name { color: var(--fd-accent, #00fff5); }
  }

  &:active {
    transform: scale(0.92) rotate(0.5deg);
    transition-duration: 0.1s;
  }

  &.launching {
    pointer-events: none;
    animation: tile-bounce 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
    .tile-cover { box-shadow: 0 0 40px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.4); }
  }
}

@keyframes tile-bounce {
  0% { transform: scale(1); }
  30% { transform: scale(0.88) rotate(-1.5deg); }
  60% { transform: scale(1.08) rotate(1deg); }
  100% { transform: scale(1) rotate(0); }
}

.tile-cover {
  position: relative;
  aspect-ratio: 3 / 4;
  border-radius: 12px;
  overflow: hidden;
  background: linear-gradient(145deg, rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.9) 0%, rgba(22, 33, 62, 0.9) 100%);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.06);
  transition: box-shadow 0.25s ease;

  .cover-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.3s ease;
  }

  .cover-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(145deg, rgba(var(--fd-bg-secondary-rgb, 26, 26, 46), 0.95) 0%, rgba(22, 33, 62, 0.95) 100%);

    .placeholder-letter {
      font-size: 48px;
      font-weight: 700;
      color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
      text-transform: uppercase;
    }
  }
}

.favorite-badge {
  position: absolute;
  top: 8px;
  right: 8px;
  color: var(--fd-status-warning, #ffd700);
  text-shadow: 0 0 8px rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.4);
  z-index: 3;
  cursor: pointer;
  transition: transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  display: flex;
  align-items: center;
  justify-content: center;

  &:hover {
    transform: scale(1.4) rotate(15deg);
  }

  svg {
    width: 16px;
    height: 16px;
  }
}

.helper-badges {
  position: absolute;
  bottom: 8px;
  left: 8px;
  display: flex;
  gap: 2px;
  z-index: 3;

  .helper-badge-icon {
    width: 18px;
    height: 18px;
    background: rgba(0, 0, 0, 0.6);
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.9);
    backdrop-filter: blur(4px);
    border-radius: 4px;
    padding: 3px;
  }
}

.tile-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(180deg, transparent 50%, rgba(0, 0, 0, 0.65) 100%);
  opacity: 0;
  transition: opacity 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;

  .play-icon {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.9);
    color: var(--fd-bg-primary, #0f0f23);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 20px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.35);
    animation: play-pop 0.4s cubic-bezier(0.34, 1.56, 0.64, 1) both;

    svg {
      width: 18px;
      height: 18px;
    }
  }
}

@keyframes play-pop {
  from { transform: scale(0) rotate(-30deg); opacity: 0; }
  to { transform: scale(1) rotate(0); opacity: 1; }
}

.launch-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  z-index: 4;
  color: var(--fd-accent, #00fff5);
  font-size: 13px;

  .launch-spinner {
    width: 28px;
    height: 28px;
    border: 3px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
    border-top-color: var(--fd-accent, #00fff5);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
}

.tile-info {
  padding: 8px 2px 2px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.tile-name {
  font-size: 13px;
  font-weight: 500;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.8);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.tile-badge {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 4px;
  font-weight: 600;
  flex-shrink: 0;

  &.admin { background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.15); color: var(--fd-status-danger, #ff6b35); }
}

.tile-playtime {
  font-size: 11px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.32);
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

@keyframes spin { to { transform: rotate(360deg); } }

.fade-in {
  animation: fadeInUp 0.35s ease both;
}

@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
