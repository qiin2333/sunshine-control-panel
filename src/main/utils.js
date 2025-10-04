import { spawn } from 'node:child_process'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { BrowserWindow, nativeTheme, net } from 'electron'
import https from 'https'
import { parseSunshineConf } from './sunshineConfig.js'
import { SUNSHINE_PATH } from './paths.js'

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
      preload: join(__dirname, 'preload/tinyworld.mjs'),
    },
    ...options,
  })
}

export function runCmdAsAdmin(cmdStr = '') {
  return spawn('powershell', [`Start-Process powershell -WindowStyle Hidden -ArgumentList '${cmdStr}' -Verb RunAs`])
}

export async function loadURLByArgs(args = [], window) {
  let url = ''

  // 检查是否有URL参数
  const urlArg = args.find((item) => /--url=/.test(item))
  if (urlArg) {
    url = urlArg.replace('--url=', '')
  } else {
    // 如果没有URL参数，尝试从配置文件获取端口
    try {
      const config = await parseSunshineConf()
      const port = 1 + (Number(config.port) || 47989)
      url = `https://localhost:${port}/`
    } catch (error) {
      console.error('从配置文件获取端口失败:', error)
      url = 'https://localhost:47990/'
    }
  }

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

export async function sendHttpRequest({ hostname, port, path, method, headers, data }) {
  return new Promise((resolve, reject) => {
    const options = {
      hostname,
      port,
      path,
      method,
      headers,
      rejectUnauthorized: false,
    }

    const req = https.request(options, (res) => {
      let responseData = ''
      res.on('data', (chunk) => (responseData += chunk))
      res.on('end', () =>
        resolve({
          ok: res.statusCode >= 200 && res.statusCode < 300,
          status: res.statusCode,
          text: () => Promise.resolve(responseData),
        })
      )
    })

    req.on('error', reject)

    req.setTimeout(10000, () => {
      req.destroy(new Error('连接超时'))
    })

    // 仅在存在数据时写入请求体
    if (data) {
      req.write(JSON.stringify(data))
    }
    req.end() // 无论是否有数据都需要调用 end()
  })
}

/**
 * 获取Sunshine版本号
 * @returns {Promise<string>} Sunshine版本号
 */
export async function getSunshineVersion() {
  return new Promise((resolve) => {
    try {
      const sunshineExe = join(SUNSHINE_PATH, 'sunshine.exe')
      
      // 使用sunshine.exe --version命令获取版本
      const child = spawn(sunshineExe, ['--version'], {
        stdio: ['ignore', 'pipe', 'pipe'],
        shell: true
      })
      
      let stdout = ''
      let stderr = ''
      
      child.stdout.on('data', (data) => {
        stdout += data.toString()
      })
      
      child.stderr.on('data', (data) => {
        stderr += data.toString()
      })
      
      child.on('close', (code) => {
        const output = stdout + stderr
        
        // 解析版本号，支持多种格式
        const patterns = [
          /Sunshine\s+v?([\d.]+)/i,           // "Sunshine v0.21.0"
          /version\s*:?\s*v?([\d.]+)/i,       // "version: 0.21.0"
          /v?(\d+\.\d+\.\d+)/,                // "0.21.0" 或 "v0.21.0"
          /(\d+\.\d+)/                        // "0.21" (备用格式)
        ]
        
        for (const pattern of patterns) {
          const match = output.match(pattern)
          if (match) {
            resolve(match[1])
            return
          }
        }
        
        // 如果所有模式都失败，返回Unknown
        resolve('Unknown')
      })
      
      child.on('error', (error) => {
        console.warn('获取Sunshine版本失败:', error)
        resolve('Unknown')
      })
      
      // 设置超时
      setTimeout(() => {
        if (!child.killed) {
          child.kill()
        }
        resolve('Unknown')
      }, 5000)
      
    } catch (error) {
      console.warn('获取Sunshine版本时发生错误:', error)
      resolve('Unknown')
    }
  })
}
