import { ipcMain } from 'electron'
import fs from 'fs'
import path from 'path'
import { parseStringPromise, Parser, Builder } from 'xml2js'
import sudo from 'sudo-prompt'
import { connect } from 'net'
import { sendHttpRequest } from './utils.js'

const VDD_SETTINGS_PATH = path.join(process.env.SystemDrive, 'VirtualDisplayDriver', 'vdd_settings.xml')
// "C:\Program Files\Sunshine\config\sunshine.conf"
const SUNSHINE_CONF_PATH = path.join(process.env.PROGRAMFILES, 'Sunshine', 'config', 'sunshine.conf')

// XML 格式示例
const DEFAULT_SETTINGS = {
  settings: {
    monitors: [{ count: 1 }],
    gpu: [{ friendlyname: [''] }],
    resolutions: [],
  },
}

// 读取设置
async function loadVddSettings() {
  try {
    if (!fs.existsSync(VDD_SETTINGS_PATH)) {
      return { success: false, data: DEFAULT_SETTINGS.VirtualDisplaySettings }
    }

    const xmlData = await fs.promises.readFile(VDD_SETTINGS_PATH, 'utf-8')
    const parser = new Parser({ explicitArray: false })
    const result = await parseStringPromise(xmlData, { parser })

    return {
      success: true,
      data: {
        monitors: result.vdd_settings.monitors,
        gpu: result.vdd_settings.gpu,
        resolutions: result.vdd_settings.resolutions,
      },
    }
  } catch (error) {
    console.error('读取设置文件失败:', error)
    return { success: false, data: DEFAULT_SETTINGS.settings }
  }
}

// 保存设置
async function saveVddSettings(settings) {
  try {
    const builder = new Builder()
    const xmlObj = {
      vdd_settings: {
        monitors: settings.monitors,
        gpu: settings.gpu,
        resolutions: settings.resolutions,
      },
    }

    const xml = builder.buildObject(xmlObj)
    console.log('生成的XML内容:', xml)

    // 确保目录存在（使用同步方法）
    const dir = path.dirname(VDD_SETTINGS_PATH)
    if (!fs.existsSync(dir)) {
      console.log('正在创建目录:', dir)
      fs.mkdirSync(dir, { recursive: true, mode: 0o755 })
    }

    const options = {
      name: 'Sunshine Control Panel',
      icns: './build/icon.icns',
    }

    // 使用临时文件并正确转义特殊字符
    const tempPath = path.join(dir, 'vdd_temp.xml')
    fs.writeFileSync(tempPath, xml, 'utf8')

    const command = `powershell -ExecutionPolicy Bypass -Command "$content = Get-Content '${tempPath}' -Raw; Set-Content -Path '${VDD_SETTINGS_PATH}' -Value $content -Encoding UTF8"`
    console.log('执行命令:', command)

    await new Promise((resolve, reject) => {
      sudo.exec(command, options, (error, stdout, stderr) => {
        // 增加调试日志
        console.log('标准输出:', stdout)
        console.log('标准错误:', stderr)
        if (error || stderr) {
          console.error('执行错误:', error || stderr)
          reject(error || new Error(stderr))
          return
        }
        console.log('保存成功，标准输出:', stdout)
        resolve()
      })
    })

    // 执行配置更新流程
    await updateSunshineConfig(settings)

    // 验证文件是否写入
    if (fs.existsSync(VDD_SETTINGS_PATH)) {
      console.log('文件验证成功，文件大小:', fs.statSync(VDD_SETTINGS_PATH).size + '字节')

      // 通过命名管道通知驱动重启
      try {
        console.log('正在通过Node.js连接命名管道通知驱动重启')

        const client = connect('\\\\.\\pipe\\ZakoVDDPipe', () => {
          console.log('已连接到管道')
          client.write('RELOAD_DRIVER')
          client.end()
        })

        client.on('error', (err) => {
          console.error('管道连接错误:', err)
        })

        // 设置连接超时
        setTimeout(() => {
          client.destroy()
        }, 5000)

        console.log('成功通知驱动重启')
      } catch (error) {
        console.error('通知驱动重启失败:', error)
        // 继续处理，不要影响整体流程
      }
    }

    return { success: true }
  } catch (error) {
    console.error('保存过程中发生错误:', error)
    return {
      success: false,
      message: `保存失败: ${error.message}`,
      stack: error.stack,
    }
  }
}

// 解析sunshine.conf文件
async function parseSunshineConf() {
  try {
    if (!fs.existsSync(SUNSHINE_CONF_PATH)) {
      return {}
    }

    const data = await fs.promises.readFile(SUNSHINE_CONF_PATH, 'utf-8')
    const lines = data.split('\n')
    const config = {}

    let currentKey = ''
    let currentValue = ''

    for (const line of lines) {
      const rawLine = line.trim() === '' ? '' : line // 保留原始行内容

      // 跳过注释和空行（只检查trim后的内容）
      if (rawLine.trim().startsWith('#') || rawLine.trim() === '') continue

      // 处理多行数组值
      if (currentKey) {
        currentValue += rawLine // 保留原始换行和缩进
        if (rawLine.trim().endsWith(']')) {
          config[currentKey] = currentValue
            .replace(/^\[/, '') // 去除开头的[
            .replace(/\]$/, '') // 去除结尾的]
          currentKey = ''
          currentValue = ''
        }
        continue
      }

      const eqIndex = rawLine.indexOf('=')
      if (eqIndex === -1) continue

      const key = rawLine.slice(0, eqIndex).trim()
      const value = rawLine.slice(eqIndex + 1) // 保留等号后的原始内容

      if (value.trim().startsWith('[') && !value.trim().endsWith(']')) {
        currentKey = key
        currentValue = value
      } else {
        config[key] = value.trim() // 保留原始换行内容
      }
    }

    return config
  } catch (error) {
    console.error('解析配置文件失败:', error)
    return {}
  }
}

// 修改后的配置更新逻辑
async function updateSunshineConfig(settings) {
  try {
    // 读取本地配置文件
    const fileConfig = await parseSunshineConf()

    console.log('fileConfig', JSON.stringify(fileConfig, null, 2))

    // 构建合并配置
    const mergedConfig = {
      ...fileConfig, // 保留文件中的配置
      adapter_name: settings.gpu[0].friendlyname[0],
      resolutions: JSON.stringify(
        settings.resolutions.map((res) => `${res.resolution[0].width[0]}x${res.resolution[0].height[0]}`)
      )
        .replace(/","/g, ',\n    ') // 替换逗号并添加换行和缩进
        .replace(/\["/, '[\n    ') // 去除开头双引号
        .replace(/"\]/, '\n  ]'), // 去除结尾双引号
      fps: JSON.stringify(settings.resolutions[0].resolution[0].refresh_rate.map((fps) => fps || 60)),
    }

    const urlPort = mergedConfig.port || 47990

    await sendHttpRequest({
      hostname: '127.0.0.1',
      port: urlPort,
      path: '/api/reset-display-device-persistence',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
    }).catch((err) => {
      console.error('重置显示设备持久化失败:', err)
    })

    const response = await sendHttpRequest({
      hostname: '127.0.0.1',
      port: urlPort,
      path: '/api/config',
      method: 'POST',
      data: mergedConfig,
    })

    if (!response.ok) {
      const errorText = await response.text()
      console.error('完整请求体:', JSON.stringify(mergedConfig, null, 2))
      throw new Error(`配置更新失败: ${response.status} ${errorText}`)
    }

    console.log('配置合并更新成功')
  } catch (error) {
    console.error('配置更新流程错误:', error)
    throw error
  }
}

// 注册 IPC 处理程序
function registerVddHandlers() {
  ipcMain.handle('vdd:loadSettings', async () => {
    return await loadVddSettings()
  })

  ipcMain.handle('vdd:saveSettings', async (_, settings) => {
    return await saveVddSettings(settings)
  })
}

export { registerVddHandlers }
