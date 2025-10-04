import { Menu, dialog, shell, app } from 'electron'
import openAboutWindow from 'about-window'
import { fileURLToPath } from 'node:url'
import { join, dirname, parse } from 'node:path'
import { createSubBrowserWin, runCmdAsAdmin, getSunshineVersion } from './utils.js'
import { SUNSHINE_PATH, SUNSHINE_TOOLS_PATH, VIRTUAL_DRIVER_PATH } from './paths.js'
import sudo from 'sudo-prompt'
import { t, initI18n } from './i18n/index.js'
import { addLanguageSwitcherToMenu } from './i18n/languageSwitcher.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

export function createMenuTemplate(mainWindow) {
  const isMac = process.platform === 'darwin'

  const menuTemplate = [
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
      label: t('menu.window'),
      submenu: [
        { role: 'minimize', label: t('submenu.minimize') },
        { role: 'zoom', label: t('submenu.zoom') },
        { role: 'reload', label: t('submenu.reload') },
        ...(isMac
          ? [
              { type: 'separator' }, 
              { role: 'front', label: t('submenu.front') }, 
              { type: 'separator' }, 
              { role: 'window', label: t('submenu.window') }
            ]
          : [{ role: 'close', label: t('submenu.close') }]),
      ],
    },
    {
      label: t('menu.management'),
      submenu: [
        {
          label: t('submenu.editVirtualDisplay'),
          click: () => {
            const subWin = createSubBrowserWin(null, mainWindow)
            subWin.loadFile(join(__dirname, '../renderer/vdd/index.html'))
          },
        },
        {
          label: t('submenu.uninstallVirtualDisplay'),
          click: async () => {
            const prompt = await dialog.showMessageBox(mainWindow, {
              type: 'question',
              message: t('dialog.confirmUninstall'),
              buttons: [t('dialog.cancel'), t('dialog.confirm')],
            })
            if (prompt.response) {
              const uninstallCmd = [
                `"${join(VIRTUAL_DRIVER_PATH, 'nefconw.exe')}"`,
                '--remove-device-node',
                '--hardware-id ROOT\\iddsampledriver',
                '--class-guid 4d36e968-e325-11ce-bfc1-08002be10318',
              ].join(' ')

              runCmdAsAdmin(uninstallCmd).on('close', (code) => {
                dialog.showMessageBox(mainWindow, {
                  message: t('dialog.uninstallComplete', { code }),
                })
              })
            }
          },
        },
        { type: 'separator' },
        {
          label: t('submenu.restartGraphicsDriver'),
          click: () => {
            const restartExe = join(SUNSHINE_TOOLS_PATH, 'restart64.exe')
            sudo.exec(`"${restartExe}"`, {
              name: 'Sunshine Control Panel',
            })
          },
        },
        {
          label: t('submenu.restartSunshineAsAdmin'),
          click: () => {
            const command = [
              'net stop sunshineservice',
              'taskkill /IM sunshine.exe /F',
              `cd "${SUNSHINE_PATH}"`,
              './sunshine.exe',
            ].join(' && ')

            runCmdAsAdmin(command).on('close', () => mainWindow.close())
          },
        },
      ],
    },
    {
      label: t('menu.tutorial'),
      submenu: [
        {
          label: t('submenu.joinStreamingGroup'),
          click: async () => {
            const subWin = createSubBrowserWin(null, mainWindow)
            subWin.loadURL('https://qm.qq.com/q/s3QnqbxvFK')
            setTimeout(() => {
              subWin.close()
            }, 3000)
          },
        },
        {
          label: t('submenu.userGuide'),
          click: async () => {
            await shell.openExternal('https://docs.qq.com/aio/DSGdQc3htbFJjSFdO')
          },
        },
      ],
    },
    {
      label: t('menu.tools'),
      submenu: [
        {
          label: t('submenu.clipboardSync'),
          click: async () => {
            const subWin = createSubBrowserWin(null, mainWindow)
            subWin.loadURL('https://gcopy.rutron.net/zh')
          },
        },
        {
          label: t('submenu.streamingTimer'),
          click: () => {
            const subWin = createSubBrowserWin({ width: 1080, height: 600 }, mainWindow)
            subWin.loadFile(join(__dirname, '../renderer/stop-clock-canvas/index.html'))
          },
        },
        {
          label: t('submenu.delayTestClock'),
          click: async () => {
            const subWin = createSubBrowserWin(null, mainWindow)
            subWin.loadURL('https://yangkile.github.io/D-lay/')
          },
        },
        {
          label: t('submenu.gamepadTest'),
          click: async () => {
            await shell.openExternal('https://hardwaretester.com/gamepad')
          },
        },
      ],
    },
    {
      label: t('menu.about'),
      click: () =>
        openAboutWindow.default({
          icon_path: 'https://raw.gitmirror.com/qiin2333/qiin.github.io/assets/img/109527119_p1.png',
          product_name: 'Sunshine Control Panel',
          homepage: 'https://sunshine-foundation.vercel.app',
          copyright: t('about.copyright'),
          use_version_info: false,
          package_json_dir: __dirname,
        }),
    },
  ]

  // 添加语言切换功能
  return addLanguageSwitcherToMenu(menuTemplate, mainWindow)
}

export function setupApplicationMenu(mainWindow) {
  // 初始化国际化
  initI18n()
  
  const menu = Menu.buildFromTemplate(createMenuTemplate(mainWindow))
  Menu.setApplicationMenu(menu)
}
