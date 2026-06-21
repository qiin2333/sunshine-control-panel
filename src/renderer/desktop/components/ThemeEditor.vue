<template>
  <Teleport to="body">
    <!-- 半透明遮罩 -->
    <Transition name="fade">
      <div v-if="open" class="theme-overlay" @click="$emit('close')"></div>
    </Transition>

    <!-- 编辑面板 -->
    <Transition name="slide">
      <div v-if="open" class="theme-editor" @click.stop>
        <div class="editor-header">
          <h2><Brush /> {{ t.themeEditor.title }}</h2>
          <button class="close-btn" @click="$emit('close')">✕</button>
        </div>

        <div class="editor-body">
          <!-- 壁纸 -->
          <section class="editor-section">
            <label class="section-title">{{ t.themeEditor.wallpaper }}</label>
            <div
              class="wallpaper-drop"
              :class="{ dragging: isDragging, 'has-wallpaper': wallpaper }"
              @dragover.prevent="isDragging = true"
              @dragleave="isDragging = false"
              @drop.prevent="handleDrop"
              @click="triggerFileInput"
            >
              <template v-if="wallpaper">
                <img :src="wallpaper" class="wallpaper-preview" />
                <div class="wallpaper-actions">
                  <button class="wp-btn" @click.stop="triggerFileInput"><Refresh /> {{ t.themeEditor.replace }}</button>
                  <button class="wp-btn danger" @click.stop="$emit('removeWallpaper')">✕ {{ t.themeEditor.remove }}</button>
                </div>
              </template>
              <template v-else>
                <div class="drop-icon"><Picture /></div>
                <div class="drop-text">{{ t.themeEditor.dropImage }}<br/><small>{{ t.themeEditor.chooseFile }}</small></div>
              </template>
            </div>
            <input
              ref="fileInputRef"
              type="file"
              accept="image/*"
              style="display: none"
              @change="handleFileSelect"
            />
            <!-- 提取的色板 -->
            <div v-if="wallpaperColors.length" class="color-palette">
              <div
                v-for="(color, i) in wallpaperColors"
                :key="i"
                class="palette-dot"
                :style="{ background: `rgb(${color.map(Math.round).join(',')})` }"
                :title="`rgb(${color.map(Math.round).join(', ')})`"
              ></div>
            </div>
          </section>

          <!-- 预设主题 -->
          <section class="editor-section">
            <label class="section-title">{{ t.themeEditor.presets }}</label>
            <div class="preset-grid">
              <button
                v-for="(preset, key) in presets"
                :key="key"
                class="preset-btn"
                :class="{ active: activePreset === key }"
                @click="$emit('applyPreset', key)"
              >
                <span class="preset-dot" :style="{ background: preset.vars['--fd-accent'] }"></span>
                {{ t.themeEditor.presetNames[key] || preset.label }}
              </button>
            </div>
          </section>

          <!-- 颜色 -->
          <section class="editor-section">
            <label class="section-title">{{ t.themeEditor.colors }}</label>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.accentColor }}</span>
              <input type="color" :value="vars['--fd-accent']" @input="throttledSetVar('--fd-accent', $event.target.value)" @change="$emit('setVar', '--fd-accent', $event.target.value)" />
            </div>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.secondaryColor }}</span>
              <input type="color" :value="vars['--fd-accent-secondary']" @input="throttledSetVar('--fd-accent-secondary', $event.target.value)" @change="$emit('setVar', '--fd-accent-secondary', $event.target.value)" />
            </div>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.backgroundPrimary }}</span>
              <input type="color" :value="vars['--fd-bg-primary']" @input="throttledSetVar('--fd-bg-primary', $event.target.value)" @change="$emit('setVar', '--fd-bg-primary', $event.target.value)" />
            </div>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.backgroundSecondary }}</span>
              <input type="color" :value="vars['--fd-bg-secondary']" @input="throttledSetVar('--fd-bg-secondary', $event.target.value)" @change="$emit('setVar', '--fd-bg-secondary', $event.target.value)" />
            </div>
          </section>

          <!-- 外观 -->
          <section class="editor-section">
            <label class="section-title">{{ t.themeEditor.appearance }}</label>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.cardRadius }}</span>
              <input type="range" min="0" max="32" :value="parseInt(vars['--fd-card-radius'])" @input="$emit('setVar', '--fd-card-radius', $event.target.value + 'px')" />
              <span class="control-value">{{ vars['--fd-card-radius'] }}</span>
            </div>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.fontSize }}</span>
              <input type="range" min="12" max="18" :value="parseInt(vars['--fd-font-size'])" @input="$emit('setVar', '--fd-font-size', $event.target.value + 'px')" />
              <span class="control-value">{{ vars['--fd-font-size'] }}</span>
            </div>
          </section>

          <!-- 效果 -->
          <section class="editor-section">
            <label class="section-title">{{ t.themeEditor.effects }}</label>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.backgroundGrid }}</span>
              <label class="switch">
                <input type="checkbox" :checked="vars['--fd-grid-visible'] === '1'" @change="$emit('setVar', '--fd-grid-visible', $event.target.checked ? '1' : '0')" />
                <span class="slider"></span>
              </label>
            </div>
            <div class="control-row">
              <span class="control-label">{{ t.themeEditor.scanlines }}</span>
              <label class="switch">
                <input type="checkbox" :checked="vars['--fd-scanline-visible'] === '1'" @change="$emit('setVar', '--fd-scanline-visible', $event.target.checked ? '1' : '0')" />
                <span class="slider"></span>
              </label>
            </div>
          </section>

          <!-- 导入导出 -->
          <section class="editor-section">
            <label class="section-title">{{ t.themeEditor.data }}</label>
            <div class="action-row">
              <button class="action-btn" @click="handleExport"><Upload /> {{ t.themeEditor.exportTheme }}</button>
              <button class="action-btn" @click="handleImport"><Download /> {{ t.themeEditor.importTheme }}</button>
            </div>
          </section>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { ref } from 'vue'
import { Brush, Refresh, Picture, Upload, Download } from '@element-plus/icons-vue'
import { useI18n } from '../i18n/index.js'

const props = defineProps({
  open: { type: Boolean, required: true },
  vars: { type: Object, required: true },
  activePreset: { type: String, required: true },
  presets: { type: Object, required: true },
  wallpaper: { type: String, default: null },
  wallpaperColors: { type: Array, default: () => [] },
})

const emit = defineEmits(['close', 'setVar', 'applyPreset', 'export', 'import', 'setWallpaper', 'removeWallpaper'])

const { t } = useI18n()
const isDragging = ref(false)
const fileInputRef = ref(null)

// 颜色选择器节流 —— 拖拽时最多每 50ms 更新一次
let colorThrottleTimer = null
function throttledSetVar(key, value) {
  if (colorThrottleTimer) return
  emit('setVar', key, value)
  colorThrottleTimer = setTimeout(() => { colorThrottleTimer = null }, 50)
}

function handleExport() {
  emit('export')
}

function handleImport() {
  emit('import')
}

function triggerFileInput() {
  fileInputRef.value?.click()
}

function handleFileSelect(e) {
  const file = e.target.files?.[0]
  if (file && file.type.startsWith('image/')) {
    emit('setWallpaper', file)
  }
  e.target.value = ''
}

function handleDrop(e) {
  isDragging.value = false
  const file = e.dataTransfer?.files?.[0]
  if (file && file.type.startsWith('image/')) {
    emit('setWallpaper', file)
  }
}
</script>

<style lang="less" scoped>
.theme-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 9998;
}

.theme-editor {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  width: 340px;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 30), 0.97);
  backdrop-filter: blur(20px);
  border-left: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  z-index: 9999;
  display: flex;
  flex-direction: column;
  box-shadow: -8px 0 40px rgba(0, 0, 0, 0.5);

  :deep(svg) {
    width: 16px;
    height: 16px;
    display: block;
    flex: 0 0 auto;
  }
}

.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.06);

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--fd-text-primary, #fff);
    display: flex;
    align-items: center;
    gap: 8px;
    line-height: 1;

    :deep(svg) {
      width: 20px;
      height: 20px;
      color: var(--fd-accent, #00fff5);
    }
  }

  .close-btn {
    width: 32px;
    height: 32px;
    border: none;
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.06);
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    border-radius: 8px;
    cursor: pointer;
    font-size: 14px;
    transition: all 0.15s ease;

    &:hover {
      background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.12);
      color: var(--fd-text-primary, #fff);
    }
  }
}

.editor-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px;

  &::-webkit-scrollbar { width: 4px; }
  &::-webkit-scrollbar-thumb { background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1); border-radius: 2px; }
}

.editor-section {
  margin-bottom: 24px;

  .section-title {
    display: block;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);
    margin-bottom: 12px;
  }
}

.preset-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.preset-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.08);
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.03);
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  border-radius: 8px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.15s ease;

  .preset-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  &:hover {
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.06);
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.9);
  }

  &.active {
    border-color: var(--fd-accent, #00fff5);
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
    color: var(--fd-accent, #00fff5);
  }
}

.control-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;

  .control-label {
    flex: 1;
    font-size: 13px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  }

  .control-value {
    font-size: 11px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.35);
    min-width: 40px;
    text-align: right;
  }

  input[type="color"] {
    width: 32px;
    height: 32px;
    border: 2px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
    border-radius: 8px;
    background: none;
    cursor: pointer;
    padding: 2px;

    &::-webkit-color-swatch-wrapper { padding: 0; }
    &::-webkit-color-swatch { border: none; border-radius: 5px; }
  }

  input[type="range"] {
    width: 100px;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
    border-radius: 2px;
    outline: none;

    &::-webkit-slider-thumb {
      -webkit-appearance: none;
      width: 14px;
      height: 14px;
      border-radius: 50%;
      background: var(--fd-accent, #00fff5);
      cursor: pointer;
      box-shadow: 0 0 6px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.4);
    }
  }
}

// Toggle switch
.switch {
  position: relative;
  width: 40px;
  height: 22px;
  display: inline-block;

  input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    inset: 0;
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
    border-radius: 11px;
    cursor: pointer;
    transition: background 0.2s ease;

    &::before {
      content: '';
      position: absolute;
      left: 3px;
      top: 3px;
      width: 16px;
      height: 16px;
      border-radius: 50%;
      background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
      transition: transform 0.2s ease, background 0.2s ease;
    }
  }

  input:checked + .slider {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);

    &::before {
      transform: translateX(18px);
      background: var(--fd-accent, #00fff5);
    }
  }
}

.action-row {
  display: flex;
  gap: 8px;
}

.action-btn {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.08);
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.03);
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  border-radius: 8px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.15s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;

  &:hover {
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.06);
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.9);
  }
}

// 壁纸上传区
.wallpaper-drop {
  border: 2px dashed rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.12);
  border-radius: 12px;
  padding: 20px;
  text-align: center;
  cursor: pointer;
  transition: all 0.2s ease;
  position: relative;
  overflow: hidden;
  min-height: 80px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;

  &:hover, &.dragging {
    border-color: var(--fd-accent, #00fff5);
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.05);
  }

  &.has-wallpaper {
    padding: 0;
    border-style: solid;
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  }

  .drop-icon {
    margin-bottom: 6px;
    color: var(--fd-accent, #00fff5);
    display: flex;
    align-items: center;
    justify-content: center;

    :deep(svg) {
      width: 32px;
      height: 32px;
    }
  }

  .drop-text {
    font-size: 12px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);

    small {
      font-size: 11px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25);
    }
  }
}

.wallpaper-preview {
  width: 100%;
  height: 100px;
  object-fit: cover;
  display: block;
  border-radius: 10px;
}

.wallpaper-actions {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  gap: 4px;
  padding: 6px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.7));
  border-radius: 0 0 10px 10px;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.2s ease;

  .wallpaper-drop:hover & {
    opacity: 1;
  }
}

.wp-btn {
  padding: 3px 10px;
  border: none;
  border-radius: 4px;
  font-size: 11px;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.2);
  color: #fff;
  transition: background 0.15s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;

  :deep(svg) {
    width: 12px;
    height: 12px;
  }

  &:hover {
    background: rgba(255, 255, 255, 0.25);
  }

  &.danger:hover {
    background: rgba(255, 60, 60, 0.5);
  }
}

.color-palette {
  display: flex;
  gap: 6px;
  margin-top: 10px;
  justify-content: center;
}

.palette-dot {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 2px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.1);
  cursor: default;
  transition: transform 0.15s ease;

  &:hover {
    transform: scale(1.3);
  }
}

// 过渡动画
.fade-enter-active, .fade-leave-active { transition: opacity 0.25s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }

.slide-enter-active, .slide-leave-active { transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1); }
.slide-enter-from, .slide-leave-to { transform: translateX(100%); }
</style>
