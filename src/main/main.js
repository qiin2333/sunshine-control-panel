// Modules to control application life and create native browser window
const { app, Menu, session, BrowserWindow } = require('electron')
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

function createWindow() {
  // Create the browser window.
  win = new BrowserWindow({
    width: 1080,
    height: 800,
    icon: './assets/sunshine.ico',
    // autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  })

  loadURLByArgs(process.argv)

  // Open the DevTools.
  // mainWindow.webContents.openDevTools()

  win.setBackgroundMaterial('auto')
}

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
          await shell.openExternal(
            'https://qm.qq.com/q/MOwm7fe68q'
          )
        },
      },
      {
        label: '加入moonlight游戏串流XX群',
        click: async () => {
          const { shell } = require('electron')
          await shell.openExternal(
            'https://qm.qq.com/q/RyiWpIRBYK'
          )
        },
      },
      {
        label: '新手入门',
        click: async () => {
          const { shell } = require('electron')
          await shell.openExternal(
            'https://flowus.cn/share/3a591f93-f48b-4164-9028-bade2c35ef58'
          )
        },
      },
    ],
  },
]

const menu = Menu.buildFromTemplate(template)
Menu.setApplicationMenu(menu)
