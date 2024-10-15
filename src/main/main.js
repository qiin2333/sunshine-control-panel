const { ipcMain, app, Menu, session, nativeTheme, BrowserWindow, dialog } = require('electron')
const path = require('node:path')
const openAboutWindow = require('about-window').default
const sudo = require('sudo-prompt')
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
      sandbox: false,
      webSecurity: false,
      allowRunningInsecureContent: true,
      preload: path.join(__dirname, 'preload.js'),
    },
  })

  loadURLByArgs(process.argv)

  // Open the DevTools.
  // win.webContents.openDevTools()
  win.webContents.on('dom-ready', setThemeColor)
  nativeTheme.on('updated', setThemeColor)
}

function createSubBrowserWin(options = {}) {
  return new BrowserWindow({
    parent: win,
    icon: './assets/sunshine.ico',
    autoHideMenuBar: true,
    useContentSize: true,
    webPreferences: {
      enablePreferredSizeMode: true,
    },
    ...options,
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

const menuTmpl = [
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
  {
    label: 'Window',
    submenu: [
      { role: 'minimize' },
      { role: 'zoom' },
      { role: 'reload' },
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
        click: () => {
          const cp = require('child_process')
          dialog.showMessageBox(win, {
            message: `虚拟显示器的设置已转移到【设置-视频/音频】页面底部, 编辑后在【windows设备管理器】中禁用再启用 Virtual Display 即可生效`,
          })
          cp.spawn('powershell', [`start devmgmt.msc`])
        },
      },
      // {
      //   label: '指定虚拟显示器调用的GPU',
      //   click: async () => {
      //     const cp = require('child_process')
      //     sudo.exec('start C:\\iddSampleDriver\\adapter.txt', { name: '212333' })
      //     dialog.showMessageBox(win, {
      //       message: `编辑后在【windows设备管理器】中禁用再启用 iddSampleDriver 即可生效`,
      //     })
      //     cp.spawn('powershell', [`start devmgmt.msc`])
      //   },
      // },
      {
        label: '卸载虚拟显示器',
        click: async () => {
          const prompt = await dialog.showMessageBox(win, {
            type: 'question',
            message: '确认卸载? 卸载后可通过重新安装基地版sunshine恢复。',
            buttons: ['取消', '确认'],
          })
          const cp = require('child_process')
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
        label: '重启显卡驱动',
        click: () => {
          sudo.exec('C:\\Program` Files\\Sunshine\\tools\\restart64.exe', { name: '212333' })
        },
      },
      {
        label: '以管理员身份重启sunshine',
        click: () => {
          runCmdAsAdmin(
            'net stop sunshineservice; taskkill /IM sunshine.exe /F; cd "C:\\Program` Files\\Sunshine"; ./sunshine.exe'
          ).on('close', () => win.close())
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
          const subWin = createSubBrowserWin()
          subWin.loadURL('https://qm.qq.com/q/s3QnqbxvFK')
          setTimeout(() => {
            subWin.close()
          }, 3000)
        },
      },
      {
        label: '食用指南',
        click: async () => {
          const { shell } = require('electron')
          await shell.openExternal('https://docs.qq.com/aio/DSGdQc3htbFJjSFdO')
        },
      },
    ],
  },
  {
    label: '小工具',
    submenu: [
      {
        label: '剪贴板同步',
        click: async () => {
          const subWin = createSubBrowserWin()
          subWin.loadURL('https://gcopy.rutron.net/zh')
        },
      },
      {
        label: '串流屏摄专用计时器',
        click: () => {
          const subWin = createSubBrowserWin({ width: 1080, height: 600 })
          subWin.loadFile(path.join(__dirname, '../renderer/stop-clock-canvas/index.html'))
        },
      },
      {
        label: '新一代延迟测试钟 by Kile',
        click: async () => {
          const subWin = createSubBrowserWin()
          subWin.loadURL('https://yangkile.github.io/D-lay/')
        },
      },
      {
        label: '手柄测试',
        click: async () => {
          const { shell } = require('electron')
          await shell.openExternal('https://hardwaretester.com/gamepad')
        },
      },
    ],
  },
  {
    label: '关于',
    click: () =>
      openAboutWindow({
        icon_path: 'https://raw.gitmirror.com/qiin2333/qiin.github.io/assets/img/109527119_p1.png',
        product_name: 'Sunshine 基地版',
        copyright: 'Copyright (c) 2023 Qiin',
        use_version_info: false,
        package_json_dir: __dirname,
      }),
  },
]

const menu = Menu.buildFromTemplate(menuTmpl)
Menu.setApplicationMenu(menu)
