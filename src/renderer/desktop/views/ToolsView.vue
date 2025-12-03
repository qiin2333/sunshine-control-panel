<template>
  <div class="tools-view">
    <div class="page-header fade-in">
      <h1 class="page-title">实用工具</h1>
      <p class="page-subtitle">系统诊断和优化工具集</p>
    </div>

    <!-- 工具网格 - 平铺所有工具 -->
    <DesktopGrid :cols="2" gap="lg" :responsive="true">
      <!-- 码率调节器 -->
      <DesktopCard title="实时码率调节" variant="success" hoverable class="tool-panel-card">
        <template #title>
          <span class="title-icon">📊</span>
          实时码率调节
        </template>

        <div class="tool-panel-content">
          <BitrateTool :embedded="true" />
        </div>
      </DesktopCard>

      <!-- DPI 调节器 -->
      <DesktopCard title="DPI 缩放调节" variant="secondary" hoverable class="tool-panel-card">
        <template #title>
          <span class="title-icon">🔍</span>
          DPI 缩放调节
        </template>

        <div class="tool-panel-content">
          <DpiAdjusterTool :embedded="true" />
        </div>
      </DesktopCard>

      <!-- 快捷键管理 -->
      <DesktopCard title="快捷键手册" variant="warning" hoverable class="tool-panel-card">
        <template #title>
          <span class="title-icon">⌨️</span>
          Moonlight 快捷键手册
        </template>

        <div class="tool-panel-content">
          <ShortcutsTool :embedded="true" />
        </div>
      </DesktopCard>
    </DesktopGrid>

    <!-- 系统诊断 -->
    <div class="section-title fade-in">
      <span class="title-icon">🔍</span>
      系统诊断
    </div>

    <DesktopCard class="diagnostics-card fade-in">
      <template #title>
        <span class="title-icon">💻</span>
        系统状态
      </template>

      <div class="diagnostics-grid">
        <div class="diagnostic-item">
          <div class="diagnostic-icon" :class="diagnostics.gpu.status">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="2" y="6" width="20" height="12" rx="2" />
              <path d="M6 10h.01M10 10h.01M14 10h.01" />
            </svg>
          </div>
          <div class="diagnostic-info">
            <div class="diagnostic-name">GPU 状态</div>
            <div class="diagnostic-value">{{ diagnostics.gpu.value }}</div>
          </div>
          <div class="diagnostic-status" :class="diagnostics.gpu.status">
            {{ diagnostics.gpu.statusText }}
          </div>
        </div>

        <div class="diagnostic-item">
          <div class="diagnostic-icon" :class="diagnostics.encoder.status">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polygon points="23 7 16 12 23 17 23 7" />
              <rect x="1" y="5" width="15" height="14" rx="2" />
            </svg>
          </div>
          <div class="diagnostic-info">
            <div class="diagnostic-name">编码器</div>
            <div class="diagnostic-value">{{ diagnostics.encoder.value }}</div>
          </div>
          <div class="diagnostic-status" :class="diagnostics.encoder.status">
            {{ diagnostics.encoder.statusText }}
          </div>
        </div>

        <div class="diagnostic-item">
          <div class="diagnostic-icon" :class="diagnostics.network.status">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M5 12.55a11 11 0 0 1 14.08 0" />
              <path d="M1.42 9a16 16 0 0 1 21.16 0" />
              <path d="M8.53 16.11a6 6 0 0 1 6.95 0" />
              <line x1="12" y1="20" x2="12.01" y2="20" />
            </svg>
          </div>
          <div class="diagnostic-info">
            <div class="diagnostic-name">网络</div>
            <div class="diagnostic-value">{{ diagnostics.network.value }}</div>
          </div>
          <div class="diagnostic-status" :class="diagnostics.network.status">
            {{ diagnostics.network.statusText }}
          </div>
        </div>

        <div class="diagnostic-item">
          <div class="diagnostic-icon" :class="diagnostics.firewall.status">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
            </svg>
          </div>
          <div class="diagnostic-info">
            <div class="diagnostic-name">防火墙</div>
            <div class="diagnostic-value">{{ diagnostics.firewall.value }}</div>
          </div>
          <div class="diagnostic-status" :class="diagnostics.firewall.status">
            {{ diagnostics.firewall.statusText }}
          </div>
        </div>
      </div>

      <template #footer>
        <button class="desktop-btn" @click="restartGraphicsDriver">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2" />
          </svg>
          重启显卡驱动
        </button>
        <button class="desktop-btn" @click="runDiagnostics">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2" />
          </svg>
          重新检测
        </button>
      </template>
    </DesktopCard>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'

// 桌面 UI 组件
import DesktopCard from '../components/DesktopCard.vue'
import DesktopGrid from '../components/DesktopGrid.vue'

// 工具组件 - 复用工具栏的工具
import BitrateTool from '../../tool-window/tools/BitrateTool.vue'
import DpiAdjusterTool from '../../tool-window/tools/DpiAdjusterTool.vue'
import ShortcutsTool from '../../tool-window/tools/ShortcutsTool.vue'

// Tauri 命令
const invoke = ref(null)

const diagnostics = ref({
  gpu: { value: 'NVIDIA RTX 4090', status: 'good', statusText: '正常' },
  encoder: { value: 'NVENC H.265', status: 'good', statusText: '可用' },
  network: { value: '千兆以太网', status: 'good', statusText: '良好' },
  firewall: { value: '端口已开放', status: 'good', statusText: '已配置' },
})

async function restartGraphicsDriver() {
  if (confirm('确定要重启显卡驱动吗？屏幕可能会短暂闪烁。')) {
    if (invoke.value) {
      try {
        await invoke.value('restart_graphics_driver')
      } catch (e) {
        console.error('Failed to restart graphics driver:', e)
      }
    }
  }
}

async function runDiagnostics() {
  // TODO: 运行系统诊断
  console.log('Running diagnostics...')
}

onMounted(async () => {
  // 动态导入 Tauri API
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }
})
</script>

<style lang="less" scoped>
.tools-view {
  max-width: 1600px;
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

.tool-panel-card {
  min-height: 400px;

  .tool-panel-content {
    max-height: 600px;
    overflow-y: auto;

    &::-webkit-scrollbar {
      width: 6px;
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(0, 255, 245, 0.2);
      border-radius: 3px;

      &:hover {
        background: rgba(0, 255, 245, 0.3);
      }
    }

    // 隐藏嵌入组件的标题栏
    :deep(.tool-header) {
      display: none;
    }
  }

  .title-icon {
    font-size: 20px;
    margin-right: 8px;
  }
}

.section-title {
  font-size: 20px;
  font-weight: 600;
  color: white;
  margin: 48px 0 16px 0;
  display: flex;
  align-items: center;
  gap: 8px;

  .title-icon {
    font-size: 24px;
  }
}

.diagnostics-card {
  .diagnostics-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 24px;
    margin-bottom: 24px;

    @media (max-width: 1200px) {
      grid-template-columns: repeat(2, 1fr);
    }

    @media (max-width: 600px) {
      grid-template-columns: 1fr;
    }
  }

  .diagnostic-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 12px;
  }

  .diagnostic-icon {
    width: 64px;
    height: 64px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;

    &.good {
      background: rgba(0, 255, 136, 0.1);
      color: #00ff88;
    }

    &.warning {
      background: rgba(255, 215, 0, 0.1);
      color: #ffd700;
    }

    &.error {
      background: rgba(255, 107, 53, 0.1);
      color: #ff6b35;
    }

    svg {
      width: 32px;
      height: 32px;
    }
  }

  .diagnostic-info {
    .diagnostic-name {
      font-size: 14px;
      color: rgba(255, 255, 255, 0.5);
      margin-bottom: 4px;
    }

    .diagnostic-value {
      font-size: 14px;
      font-weight: 500;
      color: white;
    }
  }

  .diagnostic-status {
    font-size: 12px;
    padding: 4px 12px;
    border-radius: 12px;

    &.good {
      background: rgba(0, 255, 136, 0.1);
      color: #00ff88;
    }

    &.warning {
      background: rgba(255, 215, 0, 0.1);
      color: #ffd700;
    }

    &.error {
      background: rgba(255, 107, 53, 0.1);
      color: #ff6b35;
    }
  }
}
</style>
