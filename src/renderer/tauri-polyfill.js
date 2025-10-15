/**
 * Tauri 兼容性 Polyfill
 * 为了兼容原有的代码，提供与 Electron 类似的全局 API
 */

import { darkMode, openExternalUrl, vdd, sunshine, tools } from './tauri-adapter.js'

// 模拟 Electron 的 window.electron API（如果需要）
if (typeof window !== 'undefined') {
  // 为旧代码提供兼容性
  window.electron = {
    ipcRenderer: {
      invoke: async (channel, data) => {
        console.warn('请迁移到 Tauri API，不要使用 window.electron.ipcRenderer.invoke')

        // 提供基本的兼容性映射
        switch (channel) {
          case 'vdd:loadSettings':
            return await vdd.loadSettings()
          case 'vdd:saveSettings':
            return await vdd.saveSettings(data)
          case 'vdd:getGPUs':
            return await vdd.getGPUs()
          case 'vdd:execPipeCmd':
            return await vdd.execPipeCmd(data)
          case 'dark-mode:toggle':
            return await darkMode.toggle()
          case 'dark-mode:system':
            return await darkMode.system()
          case 'openExternalUrl':
            return await openExternalUrl(data)
          default:
            console.error(`未知的 IPC channel: ${channel}`)
            return { success: false, message: '未实现的功能' }
        }
      },
    },
    webUtils: {
      /**
       * 获取文件的本地路径（Electron 兼容 API）
       * 在 Electron 中，这会将 File 对象转换为文件系统路径
       * 在 Tauri 中，我们返回 Object URL 以便在 Web UI 中使用
       * @param {File} file - File 对象
       * @returns {string} 文件的 Object URL 或路径
       */
      getPathForFile: (file) => {
        if (!file) {
          console.error('❌ getPathForFile: file 参数为空')
          return ''
        }

        // 如果 File 对象有 path 属性（非标准，但某些环境支持）
        if (file.path) {
          console.log('✅ 使用 File.path:', file.path)
          return file.path
        }

        // 在 Tauri/Web 环境中，创建 Object URL
        // 这样可以在 img 元素中直接使用
        console.log('📄 为文件创建 Object URL:', file.name)

        const reader = new FileReader()
        const objectUrl = URL.createObjectURL(file)

        // 异步读取文件为 Data URL（更持久）
        reader.onload = (e) => {
          console.log('✅ 文件已转换为 Data URL')
          // 触发自定义事件通知应用
          window.dispatchEvent(
            new CustomEvent('file-converted', {
              detail: {
                name: file.name,
                dataUrl: e.target.result,
              },
            })
          )
        }
        reader.readAsDataURL(file)

        // 返回临时 Object URL（立即可用）
        return objectUrl
      },
    },
  }

  // 提供 darkMode 全局 API
  window.darkMode = darkMode
}

// 主题切换功能（替代 Electron preload 中的功能）
export function initTheme() {
  if (typeof document === 'undefined') return

  const body = document.querySelector('body')

  // 检测系统主题
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  body.setAttribute('data-bs-theme', prefersDark ? 'dark' : 'light')

  // 监听系统主题变化
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    body.setAttribute('data-bs-theme', e.matches ? 'dark' : 'light')
  })

  // // 添加主题切换按钮
  // if (!document.querySelector('#theme_ctrl')) {
  //   const btn = document.createElement('button')
  //   btn.setAttribute('id', 'theme_ctrl')
  //   btn.setAttribute(
  //     'style',
  //     'position: fixed; width: 56px; height: 56px; border-radius: 48px; right: 18px; bottom: 18px; z-index: 9999; cursor: pointer; border: none; background: rgba(255, 255, 255, 0.1); backdrop-filter: blur(10px);'
  //   )
  //   btn.innerHTML =
  //     '<svg t="1711372397603" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg" p-id="11874" width="100%" height="100%"><path d="M561.15624 705.345669c-13.779647-17.489552-19.013263-35.376593-25.439348-55.118587-31.732937-97.318755-51.143689-197.949926-74.330595-297.454875L367.247461 378.079058c25.439348 98.511225 84.532833 230.014104 122.09562 319.051821 9.274762 21.994436 17.688297 41.140195 20.404477 45.181342 11.725949 17.290807 78.371741 66.513295 99.968687 66.513295 11.725949 0.331242 25.306851-8.413534 21.729443-21.729443 0-12.785922-59.424727-65.718315-70.289448-81.750404z" p-id="11875"></path><path d="M652.313903 557.678204c-17.224558-114.145824-42.13392-233.194022-67.904509-342.50372l-95.132561 25.704341c33.521641 100.564922 71.018179 201.063596 96.32503 304.079705 3.974898 16.363331 8.877272 29.877984 6.028596 46.638805l-16.230834 108.249725c-0.066248 6.889823 9.407259 11.129715 16.297082 11.129714h3.577408c21.530698 0 61.544672-83.605357 61.544673-104.93731v-3.577408c0-10.665977-1.722456-26.499321-4.504885-44.783852zM578.31455 279.369088c-4.041146 1.391214-9.009769 0-14.309633-2.252442l13.64715 20.338229-3.444912-0.132497-1.324966 3.246167-13.845895-20.603222 1.788704 24.511872-3.047421-1.788704-2.583684 2.252442-1.854953-25.240603c-2.981174 5.564857-6.22734 10.202238-10.599728 11.725949-3.246167 1.126221-6.823575-0.662483-7.949796-3.908649-0.993725-2.914925 0-4.902374 2.451187-7.41981-3.444912-0.463738-5.432361-1.457463-6.426085-4.438636-1.126221-3.246167 0.662483-6.823575 3.908649-7.949797 5.299864-1.788704 12.255936 1.126221 19.344505 4.571133 0.132497 0 0.198745-0.066248 0.331241-0.132496 1.258718-0.861228 2.583684-2.186194 3.90865-2.119946 3.378663-6.62483 6.956072-12.653426 11.990943-14.375882 3.246167-1.126221 6.823575 0.662483 7.949796 3.90865 0.993725 2.981174 0 4.902374-2.451187 7.41981 3.444912 0.463738 5.432361 1.457463 6.426085 4.438636 1.126221 3.246167-0.662483 6.823575-3.90865 7.949796z" p-id="11876"></path></svg>'

  //   btn.onclick = async () => {
  //     const current = body.getAttribute('data-bs-theme')
  //     const newTheme = current === 'dark' ? 'light' : 'dark'
  //     body.setAttribute('data-bs-theme', newTheme)
  //   }

  //   body.appendChild(btn)
  // }
}

// 背景图片拖放功能
export function initBgImg() {
  if (typeof document === 'undefined') return

  const localBgPath = localStorage.getItem('WEBUI-BGSRC')
  if (localBgPath) {
    document.body.style.backgroundImage = `url('${localBgPath}')`
  }

  document.addEventListener('drop', async (e) => {
    e.preventDefault()
    e.stopPropagation()

    const allowedFileTypes = ['image/png', 'image/jpeg', 'image/gif', 'image/webp']
    let bgFile

    for (const f of e.dataTransfer.files) {
      console.log('File(s) you dragged here: ', f)
      if (allowedFileTypes.includes(f.type)) {
        bgFile = f
        break
      }
    }

    if (bgFile) {
      // 在 Tauri 中使用 FileReader 读取文件
      const reader = new FileReader()
      reader.onload = (event) => {
        const bgPath = event.target.result
        document.body.style.backgroundImage = `url('${bgPath}')`
        localStorage.setItem('WEBUI-BGSRC', bgPath)
      }
      reader.readAsDataURL(bgFile)
    }
  })

  document.addEventListener('dragover', (e) => {
    e.preventDefault()
    e.stopPropagation()
  })
}

// 自动初始化
if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
      initTheme()
      initBgImg()
    })
  } else {
    initTheme()
    initBgImg()
  }
}

export default {
  initTheme,
  initBgImg,
  darkMode,
  vdd,
  sunshine,
  tools,
  openExternalUrl,
}
