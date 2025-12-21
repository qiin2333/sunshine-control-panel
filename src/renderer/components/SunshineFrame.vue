<template>
  <SidebarMenu ref="sidebarMenuRef">
    <div class="iframe-container">
      <transition name="fade-loading">
        <div v-if="loading" class="loading-overlay">
          <div class="loading-container">
            <img src="../public/gura-pix.png" class="loading-image" alt="Loading" />
            <div class="loading-text">
              <p>正在准备 {{ currentPath }} ...</p>
            </div>
          </div>
        </div>
      </transition>

      <iframe
        ref="sunshineIframe"
        v-show="!loading"
        :src="sunshineUrl"
        class="sunshine-iframe"
        @load="onLoad"
        frameborder="0"
        allow="autoplay; clipboard-read; clipboard-write; fullscreen"
      ></iframe>
    </div>
  </SidebarMenu>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { ElMessage } from 'element-plus'
import { sunshine } from '@/tauri-adapter.js'
import SidebarMenu from './SidebarMenu.vue'

// Refs
const loading = ref(true)
const sunshineUrl = ref('')
const currentPath = ref('/')
const sunshineIframe = ref(null)
const sidebarMenuRef = ref(null)

// State
let pollTimer = null
let unlistenVddSettings = null
let unlistenDragDrop = null
let messageHandler = null

// Constants
const ALLOWED_IMAGE_EXTENSIONS = ['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp']
const POLL_INTERVAL = 3000
const LOAD_DELAY = 100

// Utility functions
const extractPathFromUrl = (url) => {
  try {
    const { pathname, search, hash } = new URL(url)
    return pathname + search + hash
  } catch {
    return '/'
  }
}

const isImageFile = (path) => {
  const ext = path.toLowerCase().slice(path.lastIndexOf('.'))
  return ALLOWED_IMAGE_EXTENSIONS.includes(ext)
}

const setAnimationsPaused = (paused) => {
  document.body?.classList.toggle('paused-animations', paused)
}

const isWelcomePath = (url) => {
  if (!url) return false
  try {
    const path = new URL(url).pathname.toLowerCase()
    return path === '/welcome' || path.startsWith('/welcome/') || path === '/welcome.html'
  } catch {
    return url.includes('/welcome') || url.includes('welcome.html')
  }
}

const openWelcome = () => sidebarMenuRef.value?.openWelcome?.()

// Navigation handler
const handleNavigateFrame = (event) => {
  const url = event.detail?.url
  if (!url) return

  if (isWelcomePath(url)) {
    console.log('🔄 拦截 welcome 页面加载，打开 Vue welcome 组件')
    openWelcome()
    return
  }

  sunshineUrl.value = url
  loading.value = true
}

// Background image handling
const loadAndSetBackground = async (imagePath) => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const dataUrl = await invoke('read_image_as_data_url', { path: imagePath })

    sunshineIframe.value?.contentWindow?.postMessage(
      {
        type: 'set-background',
        dataUrl,
        filePath: imagePath,
      },
      '*'
    )
  } catch (error) {
    console.error('❌ 读取背景图片失败:', error)
  }
}

const handleTauriFileDrop = async (paths) => {
  const imagePath = paths?.find(isImageFile)
  if (imagePath) await loadAndSetBackground(imagePath)
}

// Message handling
const createMessageHandler = () => {
  const handlers = {
    'path-update': (data) => {
      currentPath.value = data.path
    },
    'navigation-start': (data) => {
      if (data.path) {
        if (isWelcomePath(data.path) || data.path.toLowerCase().includes('welcome')) {
          console.log('🔄 拦截导航到 welcome 页面，打开 Vue welcome 组件')
          openWelcome()
          return
        }
        currentPath.value = data.path
      }
      loading.value = true
    },
    'restore-background': (data) => loadAndSetBackground(data.path),
    'tauri-invoke': (data) => handleTauriInvoke(data),
    'show-message': (data) => {
      // 处理来自 Web UI 的消息显示请求
      if (data.source === 'sunshine-webui' && data.message) {
        const messageType = data.messageType || 'info'
        switch (messageType) {
          case 'success':
            ElMessage.success(data.message)
            break
          case 'error':
            ElMessage.error(data.message)
            break
          case 'warning':
            ElMessage.warning(data.message)
            break
          default:
            ElMessage.info(data.message)
        }
      }
    },
  }

  return async (event) => {
    const { data } = event
    if (data?.type && handlers[data.type]) {
      await handlers[data.type](data)
    }
  }
}

const handleTauriInvoke = async ({ id, command, args }) => {
  const contentWindow = sunshineIframe.value?.contentWindow
  if (!contentWindow) return

  const sendResponse = (payload) => {
    contentWindow.postMessage({ type: 'api-response', id, ...payload }, '*')
  }

  try {
    let result
    if (command === 'request_file_path') {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        title: '选择图片文件',
        filters: [{ name: '图片文件', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp'] }],
        multiple: false,
        directory: false,
      })
      result = { path: selected || null }
    } else {
      const { invoke } = await import('@tauri-apps/api/core')
      result = await invoke(command, args)
    }
    sendResponse({ result })
  } catch (error) {
    sendResponse({ error: error.message || String(error) })
  }
}

// Window state monitoring
const setupWindowStateMonitor = async (currentWindow) => {
  let lastMinimized = false
  let lastHidden = false

  const checkWindowState = async () => {
    try {
      const [isMinimized, isVisible] = await Promise.all([currentWindow.isMinimized(), currentWindow.isVisible()])

      if (isMinimized !== lastMinimized || !isVisible !== lastHidden) {
        lastMinimized = isMinimized
        lastHidden = !isVisible
        setAnimationsPaused(isMinimized || !isVisible)
      }
    } catch (e) {
      console.warn('⚠️ 检测窗口状态失败:', e)
    }
  }

  pollTimer = setInterval(checkWindowState, POLL_INTERVAL)
  await checkWindowState()

  const visibilityHandler = () => setAnimationsPaused(document.hidden)
  document.addEventListener('visibilitychange', visibilityHandler)

  return visibilityHandler
}

// Lifecycle
onUnmounted(() => {
  window.removeEventListener('navigate-frame', handleNavigateFrame)
  if (messageHandler) window.removeEventListener('message', messageHandler)
  if (pollTimer) clearInterval(pollTimer)
  unlistenVddSettings?.()
  unlistenDragDrop?.()
})

onMounted(async () => {
  window.addEventListener('navigate-frame', handleNavigateFrame)

  try {
    const proxyBaseUrl = await sunshine.getProxyUrl()
    const cmdLineUrl = await sunshine.getCommandLineUrl()

    if (cmdLineUrl) {
      const targetPath = extractPathFromUrl(cmdLineUrl)
      const fullUrl = proxyBaseUrl + targetPath

      if (isWelcomePath(fullUrl)) {
        console.log('🔄 启动时检测到 welcome 页面，打开 Vue welcome 组件')
        openWelcome()
        sunshineUrl.value = 'about:blank'
        currentPath.value = '/'
        loading.value = false
        return
      }

      sunshineUrl.value = fullUrl
      currentPath.value = targetPath
    } else {
      sunshineUrl.value = proxyBaseUrl + '/'
      currentPath.value = '/'
    }

    messageHandler = createMessageHandler()
    window.addEventListener('message', messageHandler)

    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const currentWindow = getCurrentWebviewWindow()

    unlistenDragDrop = await currentWindow.onDragDropEvent((event) => {
      if (event.payload.type === 'drop') {
        handleTauriFileDrop(event.payload.paths)
      }
    })

    await setupWindowStateMonitor(currentWindow)

    unlistenVddSettings = await currentWindow.listen('open-vdd-settings', () => {
      sidebarMenuRef.value?.openVddSettings?.()
    })

    await currentWindow.listen('open-welcome', openWelcome)
  } catch (error) {
    console.error('初始化失败:', error)
    try {
      sunshineUrl.value = (await sunshine.getProxyUrl()) + '/'
    } catch {
      sunshineUrl.value = 'http://localhost:48081/'
    }
  }
})

const onLoad = () => {
  setTimeout(() => {
    try {
      const iframe = sunshineIframe.value
      const newUrl = iframe?.contentWindow?.location?.href

      if (newUrl && newUrl !== 'about:blank') {
        const path = extractPathFromUrl(newUrl)

        if (isWelcomePath(newUrl) || path.toLowerCase().includes('welcome')) {
          console.log('🔄 检测到 welcome 页面加载，拦截并打开 Vue welcome 组件')
          openWelcome()
          if (iframe) iframe.src = 'about:blank'
          loading.value = true
          return
        }

        currentPath.value = path
      }
    } catch {
      // 跨域时无法读取，保持当前路径
    }

    loading.value = false
  }, LOAD_DELAY)
}
</script>

<style scoped lang="less">
@import '../styles/theme.less';

.iframe-container {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.sunshine-iframe {
  width: 100%;
  height: 100%;
  border: none;
  transition: opacity 0.3s ease;
}

.loading-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(135deg, @morandi-dark-bg 0%, @morandi-mid-bg 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10;
  backdrop-filter: blur(10px);
}

.loading-container {
  text-align: center;
  padding: 20px;
  max-width: 400px;
}

.loading-image {
  width: 60%;
  max-width: 180px;
  opacity: 0.85;
  margin-bottom: 24px;
  animation: gura 2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  position: relative;
  left: -20%;
  transform-style: preserve-3d;
  filter: drop-shadow(0 4px 12px rgba(212, 165, 165, 0.3));
}

@keyframes gura {
  0% {
    transform: translateX(-100%) rotate(-5deg) translateY(-5px) scale(0.9);
  }
  40% {
    transform: translateX(0%) rotate(0deg) translateY(2px) scale(1.1);
  }
  50% {
    transform: translateX(10%) rotate(3deg) translateY(-10px) scale(0.95);
  }
  60% {
    transform: translateX(20%) rotate(-3deg) translateY(5px) scale(1.05);
  }
  100% {
    transform: translateX(100%) rotate(5deg) translateY(-5px) scale(0.9);
  }
}

.loading-text {
  color: @morandi-yellow;
  font-size: 18px;
  line-height: 1.6;
  font-family: 'PixelMplus12', 'YouYuan', cursive, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  text-shadow: 1px 1px 3px rgba(0, 0, 0, 0.4);
  letter-spacing: 0.5px;
  font-weight: 500;
  transform: skew(-3deg);

  p {
    margin: 12px 0;
    animation: pulse 2s ease-in-out infinite;
  }
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
    transform: translateY(0);
  }
  50% {
    opacity: 0.7;
    transform: translateY(-2px);
  }
}

.fade-loading-enter-active,
.fade-loading-leave-active {
  transition: opacity 0.3s ease;
}

.fade-loading-enter-from,
.fade-loading-leave-to {
  opacity: 0;
}

body[data-bs-theme='light'] {
  .loading-overlay {
    background: linear-gradient(135deg, @gura-bg-light 0%, @gura-bg-mid 100%);
  }

  .loading-image {
    filter: drop-shadow(0 4px 12px rgba(74, 158, 255, 0.3));
  }

  .loading-text {
    color: @gura-blue;
    text-shadow: 1px 1px 3px rgba(74, 158, 255, 0.2);
  }
}
</style>

<style>
.paused-animations *,
.paused-animations *::before,
.paused-animations *::after {
  animation: none !important;
  transition: none !important;
}
</style>
