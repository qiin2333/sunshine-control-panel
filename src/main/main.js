import { Tray, nativeImage, ipcMain, app, session, nativeTheme, BrowserWindow, dialog, shell, Menu } from 'electron'
import fs from 'node:fs'
import { fileURLToPath } from 'node:url'
import { join, dirname } from 'node:path'
import si from 'systeminformation'
import { setupApplicationMenu } from './menu.js'
import { loadURLByArgs, setThemeColor, runCmdAsAdmin, createSubBrowserWin } from './utils.js'
import { registerVddHandlers } from './vddSettings.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
let win

// 提取常量
const DEFAULT_WINDOW_WIDTH = 1080
const DEFAULT_WINDOW_HEIGHT = 800
const TRAY_ICON_PATH = join(__dirname, 'static', 'gura.ico')

app.commandLine.appendSwitch('ignore-certificate-errors')

const gotTheLock = app.requestSingleInstanceLock()
if (!gotTheLock) {
  app.quit()
} else {
  app.on('second-instance', (event, argv, workingDirectory) => {
    if (win) {
      if (win.isMinimized()) win.restore()
      win.show()
      win.focus()
      loadURLByArgs(argv, win)
    }
  })
}

function createWindow() {
  // 创建浏览器窗口
  win = new BrowserWindow({
    width: DEFAULT_WINDOW_WIDTH,
    height: DEFAULT_WINDOW_HEIGHT,
    icon: TRAY_ICON_PATH,
    webPreferences: {
      sandbox: false,
      webSecurity: false,
      allowRunningInsecureContent: true,
      preload: join(__dirname, 'preload.mjs'),
      userAgent:
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 SunshineControlPanel',
    },
  })

  // 初始化托盘
  createTray()

  // 设置窗口事件
  setupWindowEvents()

  // 先加载占位页面
  if (process.env.NODE_ENV === 'development') {
    const rendererPort = process.argv[2]
    win.loadURL(`http://localhost:${rendererPort}/vdd/`)
  } else {
    win.loadFile(join(app.getAppPath(), 'renderer', 'placeholder.html')).catch(console.error)
  }

  win.webContents.once('did-finish-load', async () => {
    if (win.isDestroyed()) return
    await loadURLByArgs(process.argv, win)
    win.webContents.send('page-loaded')
  })

  win.webContents.on('did-fail-load', async (event, errorCode, errorDescription, validatedURL, isMainFrame) => {
    if (!isMainFrame) return
    win.loadFile(join(app.getAppPath(), 'renderer', 'placeholder.html')).catch(console.error)
    await new Promise((resolve) => setTimeout(resolve, 3000)) // 3秒后重试
    win.loadURL(validatedURL)
  })

  // Open the DevTools.
  // win.webContents.openDevTools()
  win.webContents.on('dom-ready', () => setThemeColor(win))
  nativeTheme.on('updated', () => setThemeColor(win))
  // 监听will-download事件, 使用外部浏览器下载资源
  win.webContents.session.on('will-download', async (event, item, webContents) => {
    // 阻止默认下载行为
    event.preventDefault()
    let downloadUrl = item.getURL()
    if (['github.com', 'raw.githubusercontent.com'].includes(new URL(downloadUrl).hostname)) {
      downloadUrl = `https://github.moeyy.xyz/${downloadUrl}`
    }
    shell.openExternal(downloadUrl)
  })
}

// 创建托盘独立函数
function createTray() {
  try {
    // 添加不同尺寸的图标适配
    const trayIcon = nativeImage.createFromPath(TRAY_ICON_PATH).resize({ width: 32, height: 32 }) // 添加尺寸适配

    const tray = new Tray(trayIcon)
    tray.setToolTip('Sunshine Control Panel')

    const contextMenu = Menu.buildFromTemplate([
      {
        label: '设置虚拟显示器（vdd)',
        click: () => {
          const subWin = createSubBrowserWin(null, win)
          subWin.loadFile(join(__dirname, '../renderer/vdd/index.html'))
        },
      },
      {
        label: '退出程序',
        click: () => {
          app.isQuiting = true
          app.quit()
        },
      },
    ])
    tray.setContextMenu(contextMenu)

    tray.on('click', () => toggleWindowVisibility())
  } catch (error) {
    console.error('托盘创建失败:', error)
  }
}

// 窗口事件处理独立函数
function setupWindowEvents() {
  // 窗口关闭时隐藏到托盘
  win.on('close', (event) => {
    if (!app.isQuiting) {
      event.preventDefault()
      win.hide()
    }
  })

  // 窗口最小化时隐藏
  win.on('minimize', (event) => {
    event.preventDefault()
    win.hide()
  })

  // 主题变化监听
  const updateThemeColor = () => setThemeColor(win)
  win.webContents.on('dom-ready', updateThemeColor)
  nativeTheme.on('updated', updateThemeColor)

  // 窗口关闭时移除监听
  win.on('closed', () => {
    nativeTheme.off('updated', updateThemeColor)
  })
}

// 窗口可见性切换函数
function toggleWindowVisibility() {
  if (win.isVisible()) {
    win.hide()
  } else {
    win.show()
    win.focus()
  }
}

// GPU信息处理函数
async function getGPUFriendlyNames() {
  const gpuInfo = await si.graphics()
  return gpuInfo.controllers.filter((controller) => controller.vram > 0).map((controller) => controller.model)
}

ipcMain.handle('dark-mode:toggle', () => {
  if (nativeTheme.shouldUseDarkColors) {
    nativeTheme.themeSource = 'light'
  } else {
    nativeTheme.themeSource = 'dark'
  }
  return nativeTheme.shouldUseDarkColors
})

ipcMain.handle('dark-mode:system', () => {
  nativeTheme.themeSource = 'system'
})

ipcMain.handle('netpierce:toggle', () => {
  // TODO:
})

ipcMain.handle('vdd:getGPUs', async () => {
  try {
    // 获取GPU信息并转换为友好名称
    const gpus = await getGPUFriendlyNames()
    return { success: true, data: gpus }
  } catch (error) {
    console.error('获取GPU信息失败:', error)
    return { success: false, message: error.message }
  }
})

ipcMain.on('read-directory', async (event, directoryPath) => {
  try {
    const files = await fs.promises.readdir(directoryPath)
    event.reply('directory-read-success', files)
  } catch (error) {
    event.reply('directory-read-error', error)
  }
})

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.whenReady().then(async () => {
  createWindow()
  setupApplicationMenu(win)

  registerVddHandlers()

  app.on('activate', function () {
    // On macOS it's common to re-create a window in the app when the
    // dock icon is clicked and there are no other windows open.
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })

  const filter = {
    urls: ['https://*.googleapis.com/*'],
  }

  session.defaultSession.webRequest.onBeforeRequest(filter, (details, cb) => {
    if (details.url.startsWith('https://translate-pa.googleapis.com/v1/supportedLanguages')) {
      cb({
        redirectURL: `https://qiin2333.github.io/sunshine-control-panel/src/main/static/supportedLanguages.js`,
      })
    } else {
      cb({ requestHeaders: details.requestHeaders })
    }
  })
})

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on('window-all-closed', function () {
  if (process.platform !== 'darwin') app.quit()
})

// 在退出应用时清理托盘
app.on('before-quit', () => {
  app.isQuiting = true
})
