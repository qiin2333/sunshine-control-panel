// Modules to control application life and create native browser window
const { ipcMain, app, Menu, session, nativeTheme, BrowserWindow, dialog } = require('electron')
const path = require('node:path')
let win = null

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
      loadURLByArgs(argv)
    }
  })
}

function loadURLByArgs(args = []) {
  const urlArg = args.find((item) => /--url=/.test(item))
  const url = urlArg?.replace('--url=', '') || 'https://localhost:47990/'
  win && win.loadURL(url)
}

function setThemeColor() {
  return win.webContents.postMessage('theme', { shouldUseDarkColors: nativeTheme.shouldUseDarkColors })
}

function runCmdAsAdmin(cmdStr = '') {
  const cp = require('child_process')
  return cp.spawn('powershell', [`Start-Process powershell -WindowStyle Hidden -ArgumentList '${cmdStr}' -Verb RunAs`])
}

function createWindow() {
  // Create the browser window.
  win = new BrowserWindow({
    width: 1080,
    height: 800,
    icon: './assets/sunshine.ico',
    // titleBarStyle: 'hidden',
    // titleBarOverlay: {
    //   color: 'rgba(0,0,0,0)',
    //   symbolColor: '#74b1be',
    //   height: 20,
    // },
    // autoHideMenuBar: true,
    webPreferences: {
      webSecurity: false,
      allowRunningInsecureContent: true,
      preload: path.join(__dirname, 'preload.js'),
    },
  })

  loadURLByArgs(process.argv)

  // Open the DevTools.
  // mainWindow.webContents.openDevTools()
  win.webContents.on('dom-ready', setThemeColor)
  nativeTheme.on('updated', setThemeColor)
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

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.whenReady().then(async () => {
  createWindow()
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

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and require them here.

const isMac = process.platform === 'darwin'

const template = [
  // { role: 'appMenu' }
  ...(isMac
    ? [
        {
          label: app.name,
          submenu: [
            { role: 'about' },
            { type: 'separator' },
            { role: 'services' },
            { type: 'separator' },
            { role: 'hide' },
            { role: 'hideOthers' },
            { role: 'unhide' },
            { type: 'separator' },
            { role: 'quit' },
          ],
        },
      ]
    : []),
  // { role: 'viewMenu' }
  {
    label: 'View',
    submenu: [
      { role: 'reload' },
      { role: 'forceReload' },
      { role: 'toggleDevTools' },
      { type: 'separator' },
      { role: 'resetZoom' },
      { role: 'zoomIn' },
      { role: 'zoomOut' },
      { type: 'separator' },
      { role: 'togglefullscreen' },
    ],
  },
  // { role: 'windowMenu' }
  {
    label: 'Window',
    submenu: [
      { role: 'minimize' },
      { role: 'zoom' },
      ...(isMac
        ? [{ type: 'separator' }, { role: 'front' }, { type: 'separator' }, { role: 'window' }]
        : [{ role: 'close' }]),
    ],
  },
  {
    label: '管理',
    submenu: [
      {
        label: '编辑虚拟显示器分辨率',
        click: async () => {
          const cp = require('child_process')
          runCmdAsAdmin('start C:\\iddSampleDriver\\option.txt')
          dialog.showMessageBox(win, {
            message: `编辑后在【windows设备管理器】中禁用再启用 iddSampleDriver 即可生效`,
          })
          cp.spawn('powershell', [`start devmgmt.msc`])
        },
      },
      {
        label: '卸载虚拟显示器',
        click: async () => {
          const prompt = await dialog.showMessageBox(win, {
            type: 'question',
            message: '确认卸载? 卸载后可通过重新安装基地版sunshine恢复。',
            buttons: ['取消', '确认'],
          })
          if (prompt.response) {
            runCmdAsAdmin(
              'C:\\IddSampleDriver\\nefconw.exe --remove-device-node --hardware-id ROOT\\iddsampledriver --class-guid 4d36e968-e325-11ce-bfc1-08002be10318'
            ).on('close', (code) => {
              dialog.showMessageBox(win, {
                message: `虚拟显示器卸载完成: ${code}`,
              })
            })
          }
        },
      },
      { type: 'separator' },
      {
        label: '以管理员身份重启sunshine',
        click: () => {
          runCmdAsAdmin('net stop sunshineservice; taskkill /IM sunshine.exe /F; cd "C:\\Program` Files\\Sunshine"; ./sunshine.exe').on('close', () => win.close());
        },
      },
    ],
  },
  {
    label: '使用教程',
    submenu: [
      {
        label: '下载最新基地版sunshine',
        click: async () => {
          const { shell } = require('electron')
          await shell.openExternal('https://github.com/qiin2333/Sunshine/releases/tag/alpha')
        },
      },
      {
        label: '加入串流基地裙',
        click: async () => {
          const { shell } = require('electron')
          await shell.openExternal('https://qm.qq.com/q/s3QnqbxvFK')
        },
      },
      {
        label: '加入moonlight游戏串流XX群',
        click: async () => {
          const { shell } = require('electron')
          await shell.openExternal('https://qm.qq.com/q/RyiWpIRBYK')
        },
      },
    ],
  },
]

const menu = Menu.buildFromTemplate(template)
Menu.setApplicationMenu(menu)
