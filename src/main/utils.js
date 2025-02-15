import { spawn } from 'node:child_process'
import { join } from 'node:path'
import { BrowserWindow, nativeTheme } from 'electron'

export function createSubBrowserWin(options = {}, parent) {
  return new BrowserWindow({
    parent,
    icon: './assets/sunshine.ico',
    autoHideMenuBar: true,
    useContentSize: true,
    webPreferences: {
      enablePreferredSizeMode: true,
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
  window && window.loadURL(url)
}

export function setThemeColor(window) {
  return window.webContents.postMessage('theme', {
    shouldUseDarkColors: nativeTheme.shouldUseDarkColors,
  })
}
