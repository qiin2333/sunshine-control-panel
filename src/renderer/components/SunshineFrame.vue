<template>
  <SidebarMenu ref="sidebarMenuRef">
    <!-- Sunshine iframe -->
    <div class="iframe-container">
      <transition name="fade-loading">
        <div v-if="loading" class="loading-overlay">
          <div class="loading-container">
            <img src="../public/gura-pix.png" class="loading-image" alt="Loading" />
            <div class="loading-text">
              <p>正在准备 {{ currentPath }} ...</p>
              <!-- <p class="url-hint">{{ displayUrl }}{{ currentPath }}</p> -->
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
import { ref, onMounted } from 'vue'
import { sunshine } from '@/tauri-adapter.js'
import SidebarMenu from './SidebarMenu.vue'

const loading = ref(true)
const sunshineUrl = ref('') // 代理 URL
const displayUrl = ref('') // 显示的实际 URL
const currentPath = ref('/') // 当前页面路径
const sunshineIframe = ref(null)
const sidebarMenuRef = ref(null)

onMounted(async () => {
  try {
    // 检查是否有 URL 参数（来自命令行参数）
    const urlParams = new URLSearchParams(window.location.search)
    const cmdLineUrl = urlParams.get('url')
    
    if (cmdLineUrl) {
      // 使用命令行参数指定的 URL（通过代理）
      console.log('✅ 使用命令行参数 URL:', cmdLineUrl)
      // 保持使用代理，但告诉用户是从命令行来的
      sunshineUrl.value = 'http://localhost:48081/'
      displayUrl.value = cmdLineUrl
      console.log('📡 通过本地代理访问:', cmdLineUrl)
    } else {
      // 获取代理服务器 URL（支持主题同步）
      const proxyUrl = await sunshine.getUrl()
      sunshineUrl.value = 'http://localhost:48081/'
      displayUrl.value = proxyUrl // 显示实际的 Sunshine URL
      console.log('✅ 使用本地代理服务器（支持主题同步）')
      console.log('📡 代理 URL:', sunshineUrl.value)
      console.log('🎯 目标 Sunshine:', proxyUrl)
      console.log('💡 代理将自动注入主题同步脚本和 API')
    }

    // 监听 Tauri 文件拖放事件
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const currentWindow = getCurrentWebviewWindow()

    // 监听文件拖放事件
    const unlisten = await currentWindow.onDragDropEvent((event) => {
      console.log('🎯 Tauri 拖放事件:', event)

      if (event.payload.type === 'drop') {
        console.log('📂 拖放的文件路径:', event.payload.paths)
        handleTauriFileDrop(event.payload.paths)
      } else if (event.payload.type === 'over') {
        console.log('🟢 文件悬停中')
      }
    })

    console.log('✅ Tauri 文件拖放监听器已启用')

    // 监听来自 iframe 的消息
    window.addEventListener('message', async (event) => {
      // 处理路径更新消息
      if (event.data && event.data.type === 'path-update') {
        currentPath.value = event.data.path
        return
      }

      // 处理导航开始消息
      if (event.data && event.data.type === 'navigation-start') {
        console.log('🔄 收到导航开始通知')
        loading.value = true
        // 如果消息中包含目标路径，立即更新显示
        if (event.data.path) {
          currentPath.value = event.data.path
        }
        return
      }

      // 处理恢复背景图片请求
      if (event.data && event.data.type === 'restore-background') {
        const path = event.data.path
        console.log('🔄 恢复背景图片请求:', path)
        await loadAndSetBackground(path)
        return
      }

      // 处理 API 调用请求
      if (event.data && event.data.type === 'tauri-invoke') {
        const { id, command, args } = event.data

        try {
          let result

          // 特殊处理：请求文件路径（打开文件对话框）
          if (command === 'request_file_path') {
            const { open } = await import('@tauri-apps/plugin-dialog')
            const selected = await open({
              title: '选择图片文件',
              filters: [
                {
                  name: '图片文件',
                  extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp'],
                },
              ],
              multiple: false,
              directory: false,
            })

            if (selected) {
              result = { path: selected }
              console.log('✅ 用户选择文件:', selected)
            } else {
              result = { path: null }
              console.log('⚠️  用户取消选择文件')
            }
          } else {
            // 普通 API 调用
            const { invoke } = await import('@tauri-apps/api/core')
            result = await invoke(command, args)
          }

          // 返回结果给 iframe
          const iframe = sunshineIframe.value
          if (iframe && iframe.contentWindow) {
            iframe.contentWindow.postMessage(
              {
                type: 'api-response',
                id,
                result,
              },
              '*'
            )
          }
        } catch (error) {
          // 返回错误给 iframe
          const iframe = sunshineIframe.value
          if (iframe && iframe.contentWindow) {
            iframe.contentWindow.postMessage(
              {
                type: 'api-response',
                id,
                error: error.message || String(error),
              },
              '*'
            )
          }
        }
      }
    })

    // 监听来自托盘的VDD设置打开事件
    const unlistenVddSettings = await currentWindow.listen('open-vdd-settings', () => {
      console.log('📱 收到托盘VDD设置事件')
      // 通过ref调用SidebarMenu的方法打开VDD设置
      if (sidebarMenuRef.value && sidebarMenuRef.value.openVddSettings) {
        sidebarMenuRef.value.openVddSettings()
      } else {
        console.warn('⚠️  无法访问SidebarMenu的openVddSettings方法')
      }
    })

    console.log('✅ Tauri VDD设置事件监听器已启用')
  } catch (error) {
    console.error('获取配置失败:', error)
    sunshineUrl.value = 'http://localhost:48081/'
  }
})

let currentUrl = ''

const onLoad = () => {
  // 延迟隐藏 loading，确保页面渲染完成
  setTimeout(() => {
    loading.value = false
    console.log('✅ Sunshine 页面加载完成')
    console.log('🎨 主题同步脚本已注入，可以同步主题了')

    // 发送当前主题
    const currentTheme = document.body.getAttribute('data-bs-theme') || 'dark'
    const iframe = sunshineIframe.value
    if (iframe && iframe.contentWindow) {
      iframe.contentWindow.postMessage(
        {
          type: 'theme-sync',
          theme: currentTheme,
        },
        '*'
      )
      console.log('📤 已发送初始主题:', currentTheme)
    }

    // 更新当前 URL 和路径（用于导航检测和显示）
    try {
      const newUrl = iframe?.contentWindow?.location?.href
      if (newUrl && newUrl !== 'about:blank') {
        currentUrl = newUrl
        // 提取路径部分
        const urlObj = new URL(newUrl)
        currentPath.value = urlObj.pathname + urlObj.search + urlObj.hash
      }
    } catch (e) {
      // 跨域时无法读取，保持显示上次的路径
    }
  }, 300)
}

// 监听 iframe 内部导航
onMounted(() => {
  const iframe = sunshineIframe.value
  if (!iframe) return

  // 监听所有可能导致导航的事件
  iframe.addEventListener('load', () => {
    console.log('📄 iframe load 事件')
  })

  // 多页应用不需要轮询检测，每次都是完整的页面加载
  // iframe 的 load 事件会自然触发
})

// ========== 加载并设置背景图片（通用函数） ==========
const loadAndSetBackground = async (imagePath) => {
  console.log('📖 正在加载背景图片:', imagePath)

  try {
    // 使用 Rust 命令读取图片并转换为 Data URL
    const { invoke } = await import('@tauri-apps/api/core')
    const dataUrl = await invoke('read_image_as_data_url', { path: imagePath })

    console.log('✅ Data URL 生成成功，长度:', dataUrl.length)

    // 通知 iframe 设置背景
    const iframe = sunshineIframe.value
    if (iframe && iframe.contentWindow) {
      iframe.contentWindow.postMessage(
        {
          type: 'set-background',
          dataUrl: dataUrl,
          filePath: imagePath, // 传递文件路径用于保存
        },
        '*'
      )
      console.log('✅ 已发送背景图片到 iframe')
    }
  } catch (error) {
    console.error('❌ 读取文件失败:', error)
  }
}

// ========== Tauri 文件拖放处理 ==========
const handleTauriFileDrop = async (paths) => {
  if (!paths || paths.length === 0) {
    console.warn('⚠️  没有拖放文件')
    return
  }

  console.log('📂 处理拖放的文件:', paths)

  // 查找图片文件
  const allowedExtensions = ['.png', '.jpg', '.jpeg', '.gif', '.webp']
  let imagePath = null

  for (const path of paths) {
    const ext = path.toLowerCase().substring(path.lastIndexOf('.'))
    console.log('📄 文件:', path, '扩展名:', ext)
    if (allowedExtensions.includes(ext)) {
      imagePath = path
      break
    }
  }

  if (!imagePath) {
    console.warn('⚠️  没有找到支持的图片格式')
    return
  }

  console.log('✅ 找到图片文件:', imagePath)
  await loadAndSetBackground(imagePath)
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
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
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

// 复用 placeholder 的 Gura 动画
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

.url-hint {
  font-size: 12px;
  color: @morandi-yellow;
  opacity: 0.5;
  margin-top: 12px;
  font-family: 'Courier New', monospace;
  transform: skew(0deg);
  letter-spacing: normal;
}

// 淡入淡出过渡
.fade-loading-enter-active,
.fade-loading-leave-active {
  transition: opacity 0.3s ease;
}

.fade-loading-enter-from,
.fade-loading-leave-to {
  opacity: 0;
}

// ========== 亮色模式适配（Gura 蓝色主题）==========
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

  .url-hint {
    color: @gura-light-blue;
  }
}
</style>
