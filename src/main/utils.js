import { spawn } from 'node:child_process'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { BrowserWindow, nativeTheme, net } from 'electron'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

export function createSubBrowserWin(options = {}, parent) {
  return new BrowserWindow({
    parent,
    icon: join(__dirname, 'static', 'gura.ico'),
    autoHideMenuBar: true,
    useContentSize: true,
    webPreferences: {
      sandbox: false,
      webSecurity: false,
      allowRunningInsecureContent: true,
      enablePreferredSizeMode: true,
      preload: join(__dirname, 'preload.mjs'),
    },
    ...options,
  })
}

export function runCmdAsAdmin(cmdStr = '') {
  return spawn('powershell', [`Start-Process powershell -WindowStyle Hidden -ArgumentList '${cmdStr}' -Verb RunAs`])
}

export function loadURLByArgs(args = [], window) {
  const urlArg = args.find((item) => /--url=/.test(item))
  const url = urlArg?.replace('--url=', '') || 'https://localhost:47990/'

  // 创建隐藏的测试窗口
  const testWindow = new BrowserWindow({
    show: false,
    webPreferences: {
      sandbox: false,
      webSecurity: false,
    },
  })

  // 先尝试在隐藏窗口加载
  testWindow
    .loadURL(url)
    .then(() => {
      // 加载成功后才加载真实窗口
      window && window.loadURL(url)
      testWindow.close()
    })
    .catch((err) => {
      console.error('URL加载失败:', err)
      testWindow.close()
    })
}

export function setThemeColor(window) {
  return window.webContents.postMessage('theme', {
    shouldUseDarkColors: nativeTheme.shouldUseDarkColors,
  })
}
