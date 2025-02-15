import { ipcMain, app, session, nativeTheme, BrowserWindow, dialog, shell } from 'electron'
import fs from 'node:fs'
import { download, CancelError } from 'electron-dl'
import { fileURLToPath } from 'node:url'
import { join, dirname } from 'node:path'
import { setupApplicationMenu } from './menu.js'
import { loadURLByArgs, setThemeColor, runCmdAsAdmin, createSubBrowserWin } from './utils.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
let win

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
  // Create the browser window.
  win = new BrowserWindow({
    width: 1080,
    height: 800,
    icon: './assets/sunshine.ico',
    webPreferences: {
      sandbox: false,
      webSecurity: false,
      allowRunningInsecureContent: true,
      preload: join(__dirname, 'preload.mjs'),
    },
  })

  // 先加载占位页面
  win.loadFile('./static/placeholder.html').catch(console.error)

  // 添加初始化标志
  let isInitialLoad = true

  // 正常加载完成后加载实际内容
  win.webContents.on('did-finish-load', () => {
    if (isInitialLoad) {
      loadURLByArgs(process.argv, win)  // 将原加载逻辑移到这里
      win.webContents.send('page-loaded')
      isInitialLoad = false  // 执行后标记为已完成
    }
  })

  // 保留原有的加载失败处理
  win.webContents.on('did-fail-load', () => {
    win.loadFile('./static/placeholder.html').catch(console.error)
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

ipcMain.on('read-directory', (event, directoryPath) => {
  try {
    fs.readdir(directoryPath, (err, files) => {
      if (err) {
        event.reply('directory-read-error', err)
      } else {
        event.reply('directory-read-success', files)
      }
    })
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
