/**
 * 占位页面加载器
 * 在占位页面加载后，自动获取 Sunshine URL 并跳转
 */

import { invoke } from '@tauri-apps/api/core'

// 等待 DOM 加载完成
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initLoader)
} else {
  initLoader()
}

async function initLoader() {
  try {
    // 等待 1 秒显示占位动画
    await new Promise((resolve) => setTimeout(resolve, 1000))

    // 检查是否有命令行参数指定的 URL
    let sunshineUrl = await invoke('get_command_line_url')

    if (sunshineUrl) {
      console.log('✅ 检测到命令行参数 URL:', sunshineUrl)
      updateLoadingText('正在加载指定的 URL...')
      // 将 URL 传递给 sunshine-frame.html（保留菜单）
      window.location.href = `./sunshine-frame.html?url=${encodeURIComponent(sunshineUrl)}`
    } else {
      console.log('✅ 正在加载 Sunshine 带自定义菜单...')
      updateLoadingText('正在连接服务器...')
      // 加载带自定义菜单的 Sunshine Frame
      window.location.href = './sunshine-frame.html'
    }
  } catch (error) {
    console.error('加载 Sunshine 失败:', error)

    // 失败后显示错误信息
    const container = document.querySelector('.placeholder-container')
    if (container) {
      const errorText = document.createElement('p')
      errorText.style.color = '#ff4d4f'
      errorText.style.marginTop = '20px'
      errorText.textContent = '无法连接到 Sunshine，请确保 Sunshine 正在运行'
      container.appendChild(errorText)

      // 5秒后尝试默认URL
      setTimeout(() => {
        window.location.href = 'https://localhost:47990/'
      }, 5000)
    }
  }
}

// 更新加载提示文本
function updateLoadingText(text) {
  const textElement = document.querySelector('.placeholder-text p')
  if (textElement) {
    textElement.textContent = text
  }
}

// 检测 Sunshine URL 是否可访问
async function checkSunshineAvailability(url) {
  try {
    const response = await fetch(url, {
      method: 'HEAD',
      mode: 'no-cors',
    })
    return true
  } catch (error) {
    return false
  }
}
