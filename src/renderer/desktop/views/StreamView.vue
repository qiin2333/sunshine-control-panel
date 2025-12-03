<template>
  <div class="stream-view">
    <div class="page-header fade-in">
      <h1 class="page-title">串流设置</h1>
      <p class="page-subtitle">配置视频编码和串流质量参数</p>
    </div>

    <!-- 编码器选择 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">🎬</span>
          视频编码器
        </div>
      </div>
      <div class="card-content">
        <div class="encoder-grid">
          <div 
            v-for="encoder in encoders" 
            :key="encoder.id"
            class="encoder-option"
            :class="{ active: selectedEncoder === encoder.id, disabled: !encoder.available }"
            @click="encoder.available && (selectedEncoder = encoder.id)"
          >
            <div class="encoder-icon">{{ encoder.icon }}</div>
            <div class="encoder-info">
              <div class="encoder-name">{{ encoder.name }}</div>
              <div class="encoder-desc">{{ encoder.description }}</div>
            </div>
            <div v-if="selectedEncoder === encoder.id" class="encoder-check">✓</div>
            <div v-if="!encoder.available" class="encoder-unavailable">不可用</div>
          </div>
        </div>
      </div>
    </div>

    <!-- 分辨率和帧率 -->
    <div class="desktop-grid cols-2 fade-in">
      <div class="desktop-card">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon">📐</span>
            分辨率
          </div>
        </div>
        <div class="card-content">
          <div class="resolution-options">
            <div 
              v-for="res in resolutions" 
              :key="res.value"
              class="resolution-option"
              :class="{ active: selectedResolution === res.value }"
              @click="selectedResolution = res.value"
            >
              <span class="res-label">{{ res.label }}</span>
              <span class="res-badge" v-if="res.badge">{{ res.badge }}</span>
            </div>
          </div>
        </div>
      </div>

      <div class="desktop-card">
        <div class="card-header">
          <div class="card-title">
            <span class="title-icon">🎯</span>
            帧率
          </div>
        </div>
        <div class="card-content">
          <div class="fps-options">
            <div 
              v-for="fps in frameRates" 
              :key="fps"
              class="fps-option"
              :class="{ active: selectedFps === fps }"
              @click="selectedFps = fps"
            >
              {{ fps }} FPS
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 码率设置 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">📊</span>
          码率设置
        </div>
        <div class="card-actions">
          <span class="bitrate-value">{{ bitrate }} Mbps</span>
        </div>
      </div>
      <div class="card-content">
        <div class="bitrate-slider">
          <input 
            type="range" 
            v-model="bitrate" 
            min="5" 
            max="150" 
            step="5"
            class="slider"
          />
          <div class="slider-labels">
            <span>5 Mbps</span>
            <span>低延迟</span>
            <span>平衡</span>
            <span>高画质</span>
            <span>150 Mbps</span>
          </div>
        </div>
        <div class="bitrate-presets">
          <button class="preset-btn" @click="bitrate = 20">低延迟 (20)</button>
          <button class="preset-btn" @click="bitrate = 50">平衡 (50)</button>
          <button class="preset-btn" @click="bitrate = 100">高画质 (100)</button>
        </div>
      </div>
    </div>

    <!-- 高级选项 -->
    <div class="desktop-card fade-in">
      <div class="card-header">
        <div class="card-title">
          <span class="title-icon">⚙️</span>
          高级选项
        </div>
      </div>
      <div class="card-content">
        <div class="options-grid">
          <label class="option-item">
            <input type="checkbox" v-model="options.hdr" />
            <span class="option-label">启用 HDR</span>
            <span class="option-desc">需要支持 HDR 的显示器和客户端</span>
          </label>
          <label class="option-item">
            <input type="checkbox" v-model="options.hevc" />
            <span class="option-label">使用 HEVC (H.265)</span>
            <span class="option-desc">更高效的编码，但兼容性略低</span>
          </label>
          <label class="option-item">
            <input type="checkbox" v-model="options.av1" />
            <span class="option-label">使用 AV1</span>
            <span class="option-desc">最新编码格式，需要较新硬件支持</span>
          </label>
          <label class="option-item">
            <input type="checkbox" v-model="options.virtualDisplay" />
            <span class="option-label">虚拟显示器</span>
            <span class="option-desc">使用虚拟显示器进行串流</span>
          </label>
        </div>
      </div>
    </div>

    <!-- 保存按钮 -->
    <div class="actions-bar fade-in">
      <button class="desktop-btn" @click="resetSettings">重置默认</button>
      <button class="desktop-btn primary" @click="saveSettings">保存设置</button>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'

const encoders = ref([
  { id: 'nvenc', name: 'NVIDIA NVENC', description: '硬件加速编码', icon: '🟢', available: true },
  { id: 'amf', name: 'AMD AMF', description: '硬件加速编码', icon: '🔴', available: false },
  { id: 'qsv', name: 'Intel QuickSync', description: '硬件加速编码', icon: '🔵', available: false },
  { id: 'software', name: '软件编码', description: 'CPU 编码（高负载）', icon: '⚪', available: true },
])

const selectedEncoder = ref('nvenc')

const resolutions = ref([
  { value: '1280x720', label: '720p', badge: null },
  { value: '1920x1080', label: '1080p', badge: '推荐' },
  { value: '2560x1440', label: '1440p', badge: null },
  { value: '3840x2160', label: '4K', badge: null },
])

const selectedResolution = ref('1920x1080')

const frameRates = [30, 60, 90, 120, 144]
const selectedFps = ref(60)

const bitrate = ref(50)

const options = ref({
  hdr: false,
  hevc: true,
  av1: false,
  virtualDisplay: false,
})

function resetSettings() {
  selectedEncoder.value = 'nvenc'
  selectedResolution.value = '1920x1080'
  selectedFps.value = 60
  bitrate.value = 50
  options.value = {
    hdr: false,
    hevc: true,
    av1: false,
    virtualDisplay: false,
  }
}

function saveSettings() {
  // TODO: 保存设置到后端
  console.log('Saving settings...', {
    encoder: selectedEncoder.value,
    resolution: selectedResolution.value,
    fps: selectedFps.value,
    bitrate: bitrate.value,
    options: options.value,
  })
}
</script>

<style lang="less" scoped>
.stream-view {
  max-width: 1000px;
  margin: 0 auto;
}

.page-header {
  margin-bottom: 32px;

  .page-title {
    font-size: 32px;
    font-weight: 700;
    color: white;
    margin: 0 0 8px 0;
  }

  .page-subtitle {
    font-size: 16px;
    color: rgba(255, 255, 255, 0.5);
    margin: 0;
  }
}

.desktop-card {
  margin-bottom: 24px;
}

.encoder-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;

  @media (max-width: 700px) {
    grid-template-columns: 1fr;
  }
}

.encoder-option {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px;
  border: 1px solid rgba(0, 255, 245, 0.2);
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
  position: relative;

  &:hover:not(.disabled) {
    border-color: rgba(0, 255, 245, 0.4);
    background: rgba(0, 255, 245, 0.05);
  }

  &.active {
    border-color: #00fff5;
    background: rgba(0, 255, 245, 0.1);
  }

  &.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .encoder-icon {
    font-size: 24px;
  }

  .encoder-info {
    flex: 1;

    .encoder-name {
      font-weight: 600;
      color: white;
      margin-bottom: 2px;
    }

    .encoder-desc {
      font-size: 13px;
      color: rgba(255, 255, 255, 0.5);
    }
  }

  .encoder-check {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #00fff5;
    color: #0f0f23;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: bold;
  }

  .encoder-unavailable {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.4);
    padding: 4px 8px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
  }
}

.resolution-options, .fps-options {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

.resolution-option, .fps-option {
  padding: 12px 20px;
  border: 1px solid rgba(0, 255, 245, 0.2);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  gap: 8px;

  &:hover {
    border-color: rgba(0, 255, 245, 0.4);
  }

  &.active {
    border-color: #00fff5;
    background: rgba(0, 255, 245, 0.1);
    color: #00fff5;
  }

  .res-badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);
    color: #0f0f23;
    font-weight: 600;
  }
}

.bitrate-value {
  font-size: 24px;
  font-weight: 700;
  background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.bitrate-slider {
  margin-bottom: 20px;

  .slider {
    width: 100%;
    height: 8px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
    appearance: none;
    outline: none;

    &::-webkit-slider-thumb {
      appearance: none;
      width: 20px;
      height: 20px;
      border-radius: 50%;
      background: linear-gradient(135deg, #00fff5 0%, #ff00ff 100%);
      cursor: pointer;
      box-shadow: 0 0 10px rgba(0, 255, 245, 0.5);
    }
  }

  .slider-labels {
    display: flex;
    justify-content: space-between;
    margin-top: 8px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.4);
  }
}

.bitrate-presets {
  display: flex;
  gap: 12px;

  .preset-btn {
    padding: 8px 16px;
    border: 1px solid rgba(0, 255, 245, 0.2);
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s ease;

    &:hover {
      border-color: #00fff5;
      color: #00fff5;
    }
  }
}

.options-grid {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.option-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  cursor: pointer;

  input[type="checkbox"] {
    width: 20px;
    height: 20px;
    margin-top: 2px;
    accent-color: #00fff5;
  }

  .option-label {
    font-weight: 500;
    color: white;
  }

  .option-desc {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    margin-left: auto;
  }
}

.actions-bar {
  display: flex;
  justify-content: flex-end;
  gap: 16px;
  margin-top: 32px;
  padding-top: 24px;
  border-top: 1px solid rgba(0, 255, 245, 0.1);
}
</style>

