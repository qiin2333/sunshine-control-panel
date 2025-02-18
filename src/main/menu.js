import { Menu, dialog, shell } from 'electron'
import openAboutWindow from 'about-window'
import { fileURLToPath } from 'node:url'
import { join, dirname } from 'node:path'
import { spawn } from 'node:child_process'
import { createSubBrowserWin, runCmdAsAdmin } from './utils.js'
import sudo from 'sudo-prompt'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

export function createMenuTemplate(mainWindow) {
  const isMac = process.platform === 'darwin'
  
  return [
    // { role: 'appMenu' }
    ...(isMac ? [{
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
    }] : []),
    {
      label: 'Window',
      submenu: [
        { role: 'minimize' },
        { role: 'zoom' },
        { role: 'reload' },
        ...(isMac ? [
          { type: 'separator' },
          { role: 'front' },
          { type: 'separator' },
          { role: 'window' }
        ] : [{ role: 'close' }]),
      ],
    },
    {
      label: '管理',
      submenu: [
        {
          label: '编辑虚拟显示器分辨率',
          click: () => {
            dialog.showMessageBox(mainWindow, {
              message: `虚拟显示器的设置已转移到【设置-视频/音频】页面底部, 编辑后保存生效`,
            })
            spawn('powershell', [`start devmgmt.msc`])
          },
        },
        {
          label: '卸载虚拟显示器',
          click: async () => {
            const prompt = await dialog.showMessageBox(mainWindow, {
              type: 'question',
              message: '确认卸载? 卸载后可通过重新安装基地版sunshine恢复。',
              buttons: ['取消', '确认'],
            })
            if (prompt.response) {
              runCmdAsAdmin(
                'C:\\VirtualDisplayDriver\\nefconw.exe --remove-device-node --hardware-id ROOT\\iddsampledriver --class-guid 4d36e968-e325-11ce-bfc1-08002be10318'
              ).on('close', (code) => {
                dialog.showMessageBox(mainWindow, {
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
            sudo.exec('C:\\Program` Files\\Sunshine\\tools\\restart64.exe', {
              name: '212333',
            })
          },
        },
        {
          label: '以管理员身份重启sunshine',
          click: () => {
            runCmdAsAdmin(
              'net stop sunshineservice; taskkill /IM sunshine.exe /F; cd "C:\\Program` Files\\Sunshine"; ./sunshine.exe'
            ).on('close', () => mainWindow.close())
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
            await shell.openExternal('https://github.com/qiin2333/Sunshine/releases/tag/alpha')
          },
        },
        {
          label: '加入串流基地裙',
          click: async () => {
            const subWin = createSubBrowserWin(null, mainWindow)
            subWin.loadURL('https://qm.qq.com/q/s3QnqbxvFK')
            setTimeout(() => {
              subWin.close()
            }, 3000)
          },
        },
        {
          label: '食用指南',
          click: async () => {
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
            const subWin = createSubBrowserWin(null, mainWindow)
            subWin.loadURL('https://gcopy.rutron.net/zh')
          },
        },
        {
          label: '串流屏摄专用计时器',
          click: () => {
            const subWin = createSubBrowserWin({ width: 1080, height: 600 }, mainWindow)
            subWin.loadFile(join(__dirname, '../renderer/stop-clock-canvas/index.html'))
          },
        },
        {
          label: '新一代延迟测试钟 by Kile',
          click: async () => {
            const subWin = createSubBrowserWin(null, mainWindow)
            subWin.loadURL('https://yangkile.github.io/D-lay/')
          },
        },
        {
          label: '手柄测试',
          click: async () => {
            await shell.openExternal('https://hardwaretester.com/gamepad')
          },
        },
      ],
    },
    {
      label: '关于',
      click: () =>
        openAboutWindow.default({
          icon_path: 'https://raw.gitmirror.com/qiin2333/qiin.github.io/assets/img/109527119_p1.png',
          product_name: 'Sunshine 基地版',
          copyright: 'Copyright (c) 2023 Qiin',
          use_version_info: false,
          package_json_dir: __dirname,
        }),
    },
  ]
}

export function setupApplicationMenu(mainWindow) {
  const menu = Menu.buildFromTemplate(createMenuTemplate(mainWindow))
  Menu.setApplicationMenu(menu)
}
