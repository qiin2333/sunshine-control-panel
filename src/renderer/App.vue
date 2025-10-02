<script setup>
import { ref, onMounted } from 'vue'

// 检查是否在 Electron 环境中
const isElectron = ref(false)

// Star History 图表状态
const starHistoryLoaded = ref(false)
const starHistoryError = ref(false)

// 版本信息状态
const versionInfo = ref({
  current: null,
  latest: null,
  preRelease: null,
  loading: true,
  error: null,
})

// 检查最新版本
const checkLatestVersion = async () => {
  try {
    versionInfo.value.loading = true
    versionInfo.value.error = null

    // 获取最新稳定版
    const latestResponse = await fetch('https://api.github.com/repos/qiin2333/Sunshine/releases/latest')
    const latestRelease = await latestResponse.json()

    // 获取所有发布版本
    const allReleasesResponse = await fetch('https://api.github.com/repos/qiin2333/Sunshine/releases')
    const allReleases = await allReleasesResponse.json()

    // 查找预发布版本
    const preRelease = allReleases.find((release) => release.prerelease)

    versionInfo.value.latest = {
      version: latestRelease.tag_name,
      downloadUrl: latestRelease.assets.find((asset) => asset.name.includes('sunshine-windows-installer.exe'))
        ?.browser_download_url,
      releaseUrl: latestRelease.html_url,
      body: latestRelease.body,
    }

    if (preRelease) {
      versionInfo.value.preRelease = {
        version: preRelease.tag_name,
        downloadUrl: preRelease.assets.find((asset) => asset.name.includes('sunshine-windows-installer.exe'))
          ?.browser_download_url,
        releaseUrl: preRelease.html_url,
        body: preRelease.body,
      }
    }

    // 更新下载链接
    if (versionInfo.value.latest.downloadUrl) {
      downloadLinks.value.latest = versionInfo.value.latest.downloadUrl
      downloadLinks.value.windows = versionInfo.value.latest.downloadUrl
      downloadLinks.value.mirror = `https://ghfast.top/${versionInfo.value.latest.downloadUrl}`
    }
  } catch (error) {
    console.error('版本检查失败:', error)
    versionInfo.value.error = error.message
    // 使用默认下载地址
    downloadLinks.value.windows = 'https://vip.123pan.cn/1813496318/26878949'
    downloadLinks.value.mirror = 'https://vip.123pan.cn/1813496318/26878949'
  } finally {
    versionInfo.value.loading = false
  }
}

onMounted(() => {
  isElectron.value = typeof window !== 'undefined' && window.electronAPI
  if (isElectron.value) {
    window.electronAPI.sendMessage('Hello from App.vue!')
  }

  // 预加载 Star History 图表
  const img = new Image()
  img.onload = () => {
    starHistoryLoaded.value = true
  }
  img.onerror = () => {
    starHistoryError.value = true
  }
  img.src = 'https://api.star-history.com/svg?repos=qiin2333/Sunshine-Foundation&type=Date&width=800&height=400'

  // 检查最新版本
  checkLatestVersion()
})

// 下载链接
const downloadLinks = ref({
  windows:
    'https://ghfast.top/https://github.com/qiin2333/Sunshine/releases/download/foundation/sunshine-windows-installer.exe',
  github: 'https://github.com/qiin2333/Sunshine-Foundation/releases/',
  mirror:
    'https://ghfast.top/https://github.com/qiin2333/Sunshine/releases/download/foundation/sunshine-windows-installer.exe',
  latest: null,
})

// 功能特性
const features = [
  {
    icon: '🎮',
    title: 'HDR友好支持',
    description: '经过优化的HDR处理管线，提供真正的HDR游戏流媒体体验',
  },
  {
    icon: '🖥️',
    title: '虚拟显示器',
    description: '内置虚拟显示器管理，无需额外软件即可创建和管理虚拟显示器',
  },
  {
    icon: '🎤',
    title: '远程麦克风',
    description: '支持接收客户端麦克风，提供高音质的语音直通功能',
  },
  {
    icon: '⚡',
    title: '低延迟传输',
    description: '结合最新硬件能力优化的编码处理，提供流畅的游戏体验',
  },
  {
    icon: '🎛️',
    title: '高级控制面板',
    description: '直观的Web控制界面，提供实时监控和配置管理',
  },
  {
    icon: '🔗',
    title: '智能配对',
    description: '智能管理配对设备的对应配置文件',
  },
]

// 客户端推荐
const clients = [
  {
    name: 'Moonlight-PC',
    platform: 'Windows/macOS/Linux',
    link: 'https://github.com/qiin2333/moonlight-qt',
    color: 'red',
  },
  {
    name: '威力加强版 Moonlight-Android',
    platform: 'Android',
    link: 'https://github.com/qiin2333/moonlight-android/releases/tag/shortcut',
    color: 'green',
  },
  {
    name: '王冠版 Moonlight-Android',
    platform: 'Android',
    link: 'https://github.com/WACrown/moonlight-android',
    color: 'blue',
  },
  {
    name: 'VoidLink (Moonlight-iOS)',
    platform: 'iOS',
    link: 'https://github.com/The-Fried-Fish/VoidLink-previously-moonlight-zwm',
    color: 'grey',
  },
]
</script>

<template>
  <div class="website">
    <!-- 头部导航 -->
    <header class="header">
      <div class="container">
        <div class="nav">
          <div class="logo">
            <h1>Sunshine 基地版</h1>
          </div>
          <nav class="nav-links">
            <a href="#features">特性</a>
            <a href="#download">下载</a>
            <a href="#clients">客户端</a>
            <a href="#stats">Star History</a>
            <a href="#docs">文档</a>
          </nav>
        </div>
      </div>
    </header>

    <!-- 主横幅 -->
    <section class="hero">
      <div class="container">
        <div class="hero-content">
          <h1 class="hero-title">让游戏串流更优雅</h1>
          <p class="hero-subtitle">基于LizardByte/Sunshine的分支，提供完整的文档支持和增强功能</p>
          <div class="hero-actions">
            <a :href="downloadLinks.windows" class="btn btn-primary"> 🚀 立即下载 </a>
            <a :href="downloadLinks.github" class="btn btn-secondary"> 📦 GitHub Releases </a>
            <a :href="downloadLinks.mirror" class="btn btn-secondary"> 🌐 镜像下载 </a>
          </div>
        </div>
      </div>
    </section>

    <!-- 核心特性 -->
    <section id="features" class="features">
      <div class="container">
        <h2 class="section-title">🌟 核心特性</h2>
        <div class="features-grid">
          <div v-for="feature in features" :key="feature.title" class="feature-card">
            <div class="feature-icon">{{ feature.icon }}</div>
            <h3 class="feature-title">{{ feature.title }}</h3>
            <p class="feature-description">{{ feature.description }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- 下载区域 -->
    <section id="download" class="download">
      <div class="container">
        <h2 class="section-title">📥 下载 Sunshine 基地版</h2>

        <!-- 版本信息 -->
        <div class="version-info" v-if="versionInfo.latest">
          <div class="version-badge">
            <span class="version-label">最新版本</span>
            <span class="version-number">{{ versionInfo.latest.version }}</span>
          </div>
          <div class="version-actions">
            <button @click="checkLatestVersion" class="btn-refresh" :disabled="versionInfo.loading">
              <span v-if="versionInfo.loading">🔄</span>
              <span v-else>🔄</span>
              检查更新
            </button>
          </div>
        </div>

        <!-- 加载状态 -->
        <div v-if="versionInfo.loading" class="loading-state">
          <div class="loading-spinner"></div>
          <p>正在检查最新版本...</p>
        </div>

        <!-- 错误状态 -->
        <div v-if="versionInfo.error" class="error-state">
          <p>⚠️ 无法检查版本信息，使用默认下载地址</p>
          <button @click="checkLatestVersion" class="btn btn-secondary">重试</button>
        </div>

        <div class="download-content">
          <div class="download-info">
            <h3>系统要求</h3>
            <ul>
              <li>系统: Windows10 22H2+</li>
              <li>CPU: Intel Core i3 / AMD Ryzen 3 以上</li>
              <li>GPU: 支持硬件编码的显卡, 支持VCE 1.0或更高版本, Intel VAAPI / AMD VCE / <a target="_blank" href="https://developer.nvidia.com/video-encode-and-decode-gpu-support-matrix-new">Nvidia NVENC</a></li>
              <li>RAM: 4GB 或更多</li>
              <li>网络: 5GHz, 802.11ac</li>
            </ul>
          </div>
          <div class="download-actions">
            <a :href="downloadLinks.windows" class="download-btn">
              <span class="download-icon">🪟</span>
              <span class="download-text">
                <strong>Windows 最新版</strong>
                <small v-if="versionInfo.latest">{{ versionInfo.latest.version }}</small>
                <small v-else>推荐使用</small>
              </span>
            </a>
            <a :href="downloadLinks.github" class="download-btn secondary">
              <span class="download-icon">📦</span>
              <span class="download-text">
                <strong>所有版本</strong>
                <small>GitHub Releases</small>
              </span>
            </a>
            <a :href="downloadLinks.mirror" class="download-btn secondary">
              <span class="download-icon">🌐</span>
              <span class="download-text">
                <strong>镜像下载</strong>
                <small>国内加速</small>
              </span>
            </a>
          </div>
        </div>

        <!-- 预发布版本提示 -->
        <div v-if="versionInfo.preRelease" class="prerelease-alert">
          <div class="alert-content">
            <h4>🚀 预发布版本可用</h4>
            <p>
              发现新的预发布版本 <strong>{{ versionInfo.preRelease.version }}</strong>
            </p>
            <a :href="versionInfo.preRelease.releaseUrl" class="btn btn-warning" target="_blank"> 查看预发布版本 </a>
          </div>
        </div>
      </div>
    </section>

    <!-- 推荐客户端 -->
    <section id="clients" class="clients">
      <div class="container">
        <h2 class="section-title">📱 推荐的 Moonlight 客户端</h2>
        <p class="section-subtitle">建议使用以下经过优化的客户端获得最佳的串流体验</p>
        <div class="clients-grid">
          <div v-for="client in clients" :key="client.name" class="client-card">
            <div class="client-info">
              <h3 class="client-name">{{ client.name }}</h3>
              <p class="client-platform">{{ client.platform }}</p>
            </div>
            <a :href="client.link" class="client-link" target="_blank" rel="noopener"> 下载 → </a>
          </div>
        </div>
      </div>
    </section>

    <!-- Star History -->
    <section id="stats" class="stats">
      <div class="container">
        <h2 class="section-title">⭐ Star History</h2>
        <p class="section-subtitle">查看项目的 GitHub Star 增长趋势</p>
        <div class="star-history-container">
          <div v-if="!starHistoryLoaded && !starHistoryError" class="loading-state">
            <div class="loading-spinner"></div>
            <p>正在加载 Star History...</p>
          </div>
          <div v-else-if="starHistoryError" class="error-state">
            <p>⚠️ 无法加载 Star History 图表</p>
            <a
              href="https://star-history.com/#qiin2333/Sunshine-Foundation&Date"
              target="_blank"
              class="btn btn-secondary"
            >
              手动查看
            </a>
          </div>
          <img
            v-else
            src="https://api.star-history.com/svg?repos=qiin2333/Sunshine-Foundation&type=Date&width=800&height=400"
            alt="Sunshine 基地版 Star History"
            class="star-history-chart"
            loading="lazy"
          />
        </div>
        <div class="stats-actions">
          <a href="https://github.com/qiin2333/Sunshine-Foundation" class="btn btn-primary" target="_blank">
            ⭐ 给个 Star
          </a>
          <a
            href="https://star-history.com/#qiin2333/Sunshine-Foundation&Date"
            class="btn btn-secondary"
            target="_blank"
          >
            📊 查看详细统计
          </a>
        </div>
      </div>
    </section>

    <!-- 文档链接 -->
    <section id="docs" class="docs">
      <div class="container">
        <h2 class="section-title">📚 文档与支持</h2>
        <div class="docs-grid">
          <a href="https://docs.qq.com/aio/DSGdQc3htbFJjSFdO?p=YTpMj5JNNdB5hEKJhhqlSB" class="doc-card" target="_blank">
            <h3>📖 使用文档</h3>
            <p>详细的使用指南和配置说明</p>
          </a>
          <a href="https://docs.lizardbyte.dev/projects/sunshine/latest/" class="doc-card" target="_blank">
            <h3>📋 官方文档</h3>
            <p>LizardByte 官方文档参考</p>
          </a>
          <a
            href="https://qm.qq.com/cgi-bin/qm/qr?k=5qnkzSaLIrIaU4FvumftZH_6Hg7fUuLD&jump_from=webapi"
            class="doc-card"
            target="_blank"
          >
            <h3>💬 QQ 交流群</h3>
            <p>加入社区获取帮助</p>
    </a>
  </div>
      </div>
    </section>

    <!-- 页脚 -->
    <footer class="footer">
      <div class="container">
        <div class="footer-content">
          <div class="footer-section">
            <h4>Sunshine 基地版</h4>
            <p>让游戏串流更优雅</p>
          </div>
          <div class="footer-section">
            <h4>相关链接</h4>
            <ul>
              <li><a href="https://github.com/qiin2333/Sunshine" target="_blank">GitHub</a></li>
              <li><a href="https://github.com/LizardByte/awesome-sunshine" target="_blank">awesome-sunshine</a></li>
            </ul>
          </div>
        </div>
        <div class="footer-bottom">
          <p>&copy; 2024 Sunshine 基地版. 基于 LizardByte/Sunshine 修改.</p>
        </div>
      </div>
    </footer>
  </div>
</template>

<style lang="less" scoped>
.website {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  line-height: @line-height-normal;
  color: @text-primary;
}

.container {
  .container();
}

/* 头部导航 */
.header {
  background: @background-primary;
  box-shadow: @shadow-md;
  position: sticky;
  top: 0;
  z-index: @z-sticky;
}

.nav {
  .flex-between();
  padding: @spacing-sm 0;
}

.logo h1 {
  margin: 0;
  color: @primary-color;
  font-size: @font-size-2xl;
}

.nav-links {
  display: flex;
  gap: @spacing-lg;

  a {
    text-decoration: none;
    color: @text-secondary;
    font-weight: @font-weight-medium;
    transition: color @transition-normal;

    &:hover {
      color: @primary-color;
    }
  }
}

/* 主横幅 */
.hero {
  background: @gradient-primary;
  color: white;
  padding: @spacing-2xl 0;
  text-align: center;
  .fade-in();
  position: relative;
  overflow: hidden;

  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: @gradient-accent;
    opacity: 0.1;
    z-index: 1;
  }

  .container {
    position: relative;
    z-index: 2;
  }
}

.hero-title {
  font-size: @font-size-5xl;
  margin-bottom: @spacing-sm;
  font-weight: @font-weight-bold;
}

.hero-subtitle {
  font-size: @font-size-xl;
  margin-bottom: @spacing-lg;
  opacity: 0.9;
}

.hero-actions {
  .flex-center();
  gap: @spacing-sm;
  flex-wrap: wrap;
}

.btn {
  .btn-base();

  &.btn-primary {
    background: @primary-color;
    color: white;
    border: 2px solid @primary-color;

    &:hover {
      background: @primary-hover;
      border-color: @primary-hover;
      transform: translateY(-2px);
      box-shadow: 0 4px 12px rgba(255, 107, 107, 0.3);
    }
  }

  &.btn-secondary {
    background: transparent;
    color: white;
    border: 2px solid white;

    &:hover {
      background: white;
      color: @primary-color;
      border-color: @primary-color;
    }
  }
}

/* 特性区域 */
.features {
  padding: @spacing-2xl 0;
  background: @background-secondary;
}

.section-title {
  text-align: center;
  font-size: @font-size-4xl;
  margin-bottom: @spacing-xl;
  color: @text-primary;
}

.features-grid {
  .grid-auto-fit(300px);
}

.feature-card {
  .card();
  padding: @spacing-xl;
  text-align: center;
  position: relative;

  &::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 0;
    height: 0;
    background: radial-gradient(circle, rgba(255, 107, 107, 0.1) 0%, transparent 70%);
    border-radius: 50%;
    transform: translate(-50%, -50%);
    transition: all @transition-slow;
  }

  &:hover {
    &::after {
      width: 200px;
      height: 200px;
    }
  }
}

.feature-icon {
  font-size: @font-size-5xl;
  margin-bottom: @spacing-md;
  position: relative;
  z-index: 2;
  display: inline-block;
  transition: transform @transition-normal;

  .feature-card:hover & {
    transform: scale(1.1) rotate(5deg);
  }
}

.feature-title {
  font-size: @font-size-2xl;
  margin-bottom: @spacing-md;
  color: @text-primary;
  font-weight: @font-weight-bold;
  position: relative;
  z-index: 2;
  transition: color @transition-normal;

  .feature-card:hover & {
    color: @primary-color;
  }
}

.feature-description {
  color: @text-secondary;
  line-height: @line-height-relaxed;
  position: relative;
  z-index: 2;
  transition: color @transition-normal;

  .feature-card:hover & {
    color: @text-primary;
  }
}

/* 下载区域 */
.download {
  padding: @spacing-2xl 0;
}

.version-info {
  .flex-between();
  align-items: center;
  background: linear-gradient(135deg, rgba(255, 107, 107, 0.05) 0%, rgba(78, 205, 196, 0.05) 100%);
  border-radius: @border-radius-lg;
  padding: @spacing-lg;
  margin-bottom: @spacing-lg;
  box-shadow: 0 8px 32px rgba(255, 107, 107, 0.15);
  border: 2px solid @primary-color;
  position: relative;
  overflow: hidden;

  &::after {
    content: '';
    position: absolute;
    top: -50%;
    right: -50%;
    width: 100%;
    height: 200%;
    background: radial-gradient(circle, rgba(255, 107, 107, 0.1) 0%, transparent 70%);
    animation: pulse 3s ease-in-out infinite;
    pointer-events: none;
  }
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.3;
    transform: scale(1);
  }
  50% {
    opacity: 0.6;
    transform: scale(1.1);
  }
}

.version-badge {
  display: flex;
  align-items: center;
  gap: @spacing-sm;
  position: relative;
  z-index: 2;

  .version-label {
    color: @text-secondary;
    font-size: @font-size-sm;
    font-weight: @font-weight-medium;
  }

  .version-number {
    background: @gradient-primary;
    color: white;
    padding: @spacing-sm @spacing-md;
    border-radius: @border-radius-md;
    font-weight: @font-weight-bold;
    font-size: @font-size-sm;
    box-shadow: 0 4px 12px rgba(255, 107, 107, 0.3);
    position: relative;
    overflow: hidden;

    &::before {
      content: '';
      position: absolute;
      top: 0;
      left: -100%;
      width: 100%;
      height: 100%;
      background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
      transition: left @transition-normal;
    }

    &:hover::before {
      left: 100%;
    }
  }
}

.btn-refresh {
  .btn-base();
  background: @background-secondary;
  color: @text-primary;
  font-size: @font-size-sm;
  padding: @spacing-xs @spacing-sm;

  &:hover:not(:disabled) {
    background: @border-color;
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.prerelease-alert {
  background: linear-gradient(135deg, #ff6b6b, #ff8e8e);
  border: 2px solid @primary-color;
  border-radius: @border-radius-md;
  padding: @spacing-md;
  margin-top: @spacing-lg;
  color: white;

  .alert-content {
    .flex-between();
    align-items: center;

    h4 {
      margin: 0 0 @spacing-xs 0;
      color: white;
    }

    p {
      margin: 0;
      color: white;
    }

    .btn-warning {
      background: white;
      color: @primary-color;
      border: 2px solid white;

      &:hover {
        background: @primary-color;
        color: white;
        border-color: @primary-color;
      }
    }
  }
}

.download-content {
  .grid-responsive(2, @spacing-xl);
  align-items: start;
}

.download-info {
  h3 {
    margin-bottom: @spacing-sm;
    color: @text-primary;
  }

  ul {
    list-style: none;
    padding: 0;
  }

  li {
    padding: @spacing-xs 0;
    border-bottom: 1px solid @border-color;
  }
}

.download-actions {
  .flex-column();
  gap: @spacing-sm;
}

.download-btn {
  .flex-center();
  padding: @spacing-md;
  background: @gradient-primary;
  color: white;
  text-decoration: none;
  border-radius: @border-radius-md;
  transition: all @transition-normal;

  &:hover {
    background: @gradient-accent;
    transform: translateY(-2px);
    box-shadow: 0 8px 20px rgba(255, 107, 107, 0.3);
  }

  &.secondary {
    background: @gradient-secondary;

    &:hover {
      background: linear-gradient(135deg, @secondary-hover 0%, #26a69a 100%);
      box-shadow: 0 8px 20px rgba(78, 205, 196, 0.3);
    }
  }
}

.download-icon {
  font-size: @font-size-3xl;
  margin-right: @spacing-sm;
}

.download-text {
  .flex-column();
  align-items: flex-start;

  strong {
    font-size: @font-size-lg;
  }

  small {
    opacity: 0.8;
  }
}

/* 客户端区域 */
.clients {
  padding: @spacing-2xl 0;
  background: @background-secondary;
}

.section-subtitle {
  text-align: center;
  color: @text-secondary;
  margin-bottom: @spacing-xl;
}

.clients-grid {
  .grid-auto-fit(250px);
}

.client-card {
  .card();
  padding: @spacing-lg;
  .flex-between();
  position: relative;
  overflow: hidden;

  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(78, 205, 196, 0.1), transparent);
    transition: left @transition-slow;
  }

  &:hover {
    &::before {
      left: 100%;
    }
  }
}

.client-info {
  position: relative;
  z-index: 2;
}

.client-name {
  margin: 0 0 @spacing-xs 0;
  font-size: @font-size-lg;
  color: @text-primary;
  font-weight: @font-weight-semibold;
  transition: color @transition-normal;

  .client-card:hover & {
    color: @secondary-color;
  }
}

.client-platform {
  margin: 0;
  color: @text-secondary;
  font-size: @font-size-sm;
  transition: color @transition-normal;

  .client-card:hover & {
    color: @text-primary;
  }
}

.client-link {
  color: @secondary-color;
  text-decoration: none;
  font-weight: @font-weight-semibold;
  padding: @spacing-xs @spacing-sm;
  border-radius: @border-radius-sm;
  transition: all @transition-normal;
  position: relative;
  z-index: 2;

  &:hover {
    background: @secondary-color;
    color: white;
    transform: scale(1.05);
  }
}

/* Star History 区域 */
.stats {
  padding: @spacing-2xl 0;
  background: @background-secondary;
}

.star-history-container {
  background: white;
  border-radius: @border-radius-lg;
  padding: @spacing-xl;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
  margin-bottom: @spacing-lg;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 400px;
  position: relative;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.2);

  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: @gradient-primary;
  }

  &::after {
    content: '';
    position: absolute;
    top: -50%;
    left: -50%;
    width: 200%;
    height: 200%;
    background: radial-gradient(circle, rgba(255, 107, 107, 0.05) 0%, transparent 70%);
    animation: rotate 20s linear infinite;
    pointer-events: none;
  }
}

@keyframes rotate {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.star-history-chart {
  width: 100%;
  max-width: 800px;
  height: auto;
  min-height: 300px;
  border-radius: @border-radius-sm;
  object-fit: contain;
}

.stats-actions {
  .flex-center();
  gap: @spacing-sm;
  flex-wrap: wrap;
}

.loading-state,
.error-state {
  .flex-center();
  .flex-column();
  gap: @spacing-sm;
  padding: @spacing-xl 0;
  color: @text-secondary;
}

.loading-spinner {
  width: 40px;
  height: 40px;
  border: 3px solid @border-color;
  border-top: 3px solid @primary-color;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  0% {
    transform: rotate(0deg);
  }
  100% {
    transform: rotate(360deg);
  }
}

.error-state {
  color: #dc2626;

  p {
    margin: 0 0 @spacing-sm 0;
  }
}

/* 文档区域 */
.docs {
  padding: @spacing-2xl 0;
}

.docs-grid {
  .grid-auto-fit(250px);
}

.doc-card {
  .card();
  padding: @spacing-xl;
  text-decoration: none;
  color: inherit;
  position: relative;
  overflow: hidden;

  &::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    width: 0;
    height: 3px;
    background: @gradient-primary;
    transition: width @transition-normal;
  }

  &:hover {
    &::after {
      width: 100%;
    }
  }

  h3 {
    margin: 0 0 @spacing-md 0;
    color: @text-primary;
    font-weight: @font-weight-bold;
    transition: color @transition-normal;
    position: relative;
    z-index: 2;

    .doc-card:hover & {
      color: @primary-color;
    }
  }

  p {
    margin: 0;
    color: @text-secondary;
    transition: color @transition-normal;
    position: relative;
    z-index: 2;

    .doc-card:hover & {
      color: @text-primary;
    }
  }
}

/* 页脚 */
.footer {
  background: @background-dark;
  color: white;
  padding: @spacing-xl 0 @spacing-sm;
}

.footer-content {
  .grid-auto-fit(250px);
  margin-bottom: @spacing-lg;
}

.footer-section {
  h4 {
    margin: 0 0 @spacing-sm 0;
    color: lighten(@background-dark, 20%);
  }

  p {
    margin: 0;
    color: @text-muted;
  }

  ul {
    list-style: none;
    padding: 0;
  }

  li {
    margin-bottom: @spacing-xs;
  }

  a {
    color: @text-muted;
    text-decoration: none;

    &:hover {
      color: white;
    }
  }
}

.footer-bottom {
  border-top: 1px solid @border-dark;
  padding-top: @spacing-sm;
  text-align: center;
  color: @text-muted;
}

/* 响应式设计 */
@media (max-width: @breakpoint-md) {
  .hero-title {
    font-size: @font-size-4xl;
  }

  .hero-actions {
    flex-direction: column;
    align-items: center;
  }

  .download-content {
    grid-template-columns: 1fr;
  }

  .nav-links {
    .hide-on-mobile();
  }

  .star-history-container {
    padding: @spacing-sm;
    min-height: 300px;
  }

  .star-history-chart {
    min-height: 200px;
    max-width: 100%;
  }

  .stats-actions {
    flex-direction: column;
    align-items: center;
  }

  .version-info {
    flex-direction: column;
    gap: @spacing-sm;
    text-align: center;
  }

  .prerelease-alert .alert-content {
    flex-direction: column;
    gap: @spacing-sm;
    text-align: center;
  }
}
</style>
