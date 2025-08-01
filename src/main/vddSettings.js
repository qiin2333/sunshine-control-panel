import { ipcMain } from 'electron'
import os from 'os'
import fs from 'fs'
import path from 'path'
import { parseStringPromise, Parser, Builder } from 'xml2js'
import sudo from 'sudo-prompt'
import { connect } from 'net'
import { sendHttpRequest } from './utils.js'
import { VDD_SETTINGS_PATH, SUNSHINE_CONF_PATH } from './paths.js'

// XML 格式示例
const DEFAULT_SETTINGS = {
  settings: {
    monitors: [{ count: 1 }],
    gpu: [{ friendlyname: [''] }],
    global: {
      g_refresh_rate: [60, 120, 240],
    },
    resolutions: [],
    colour: [
      {
        SDR10bit: [false],
        HDRPlus: [false],
        ColourFormat: ['RGB'],
      },
    ],
  },
}

// 读取设置
async function loadVddSettings() {
  try {
    if (!fs.existsSync(VDD_SETTINGS_PATH)) {
      return { success: false, data: DEFAULT_SETTINGS.settings }
    }

    const xmlData = await fs.promises.readFile(VDD_SETTINGS_PATH, 'utf-8')
    const parser = new Parser({ explicitArray: false })
    const result = await parseStringPromise(xmlData, { parser })

    return {
      success: true,
      data: {
        ...result.vdd_settings,
        monitors: result.vdd_settings.monitors ?? [{ count: 1 }],
        gpu: result.vdd_settings.gpu ?? [{ friendlyname: [''] }],
        global: result.vdd_settings.global ?? DEFAULT_SETTINGS.settings.global,
        resolutions: result.vdd_settings.resolutions ?? [],
        colour: (result.vdd_settings.colour || DEFAULT_SETTINGS.settings.colour).map((item) => ({
          SDR10bit: item.SDR10bit[0] === 'true' ? true : false,
          HDRPlus: item.HDRPlus[0] === 'true' ? true : false,
          ColourFormat: item.ColourFormat[0],
        })),
        logging: result.vdd_settings.logging.map((item) => ({
          logging: item.logging[0] === 'true' ? true : false,
          debuglogging: item.debuglogging[0] === 'true' ? true : false,
        })),
      },
    }
  } catch (error) {
    console.error('读取设置文件失败:', error)
    return { success: false, data: DEFAULT_SETTINGS.settings }
  }
}

// 发送pipe命令
async function execPipeCmd(command) {
  return new Promise((resolve, reject) => {
    try {
      console.log(`正在通过命名管道发送命令: ${command}`)

      const client = connect('\\\\.\\pipe\\ZakoVDDPipe', () => {
        console.log('已连接到管道')

        // 将命令转换为 UTF-16LE 编码（wchar_t）
        const utf16Buffer = Buffer.from(command + '\0', 'utf16le') // 添加空终止符
        console.log('发送的UTF-16LE缓冲区大小:', utf16Buffer.length, '字节')

        client.write(utf16Buffer)
        client.end()
        resolve(true)
      })

      client.on('error', (err) => {
        console.error('管道连接错误:', err)
        reject(err)
      })

      client.on('close', () => {
        console.log('管道连接关闭')
      })

      // 设置连接超时
      const timeout = setTimeout(() => {
        client.destroy()
        reject(new Error('管道连接超时'))
      }, 5000)

      // 清除超时
      client.on('connect', () => {
        clearTimeout(timeout)
      })
    } catch (error) {
      console.error('通知驱动失败:', error)
      reject(error)
    }
  })
}

// 保存设置
async function saveVddSettings(settings) {
  try {
    const builder = new Builder()
    const xmlObj = {
      vdd_settings: {
        monitors: settings.monitors,
        gpu: settings.gpu,
        global: settings.global,
        resolutions: settings.resolutions,
        colour: settings.colour,
        logging: settings.logging,
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

    // 使用Windows临时目录
    const winTempDir = os.tmpdir()
    let tempPath = path.join(winTempDir, `vdd_temp_${Date.now()}.xml`)
    try {
      fs.writeFileSync(tempPath, xml, 'utf8')
      console.log('已将临时文件写入Windows临时目录:', tempPath)
    } catch (error) {
      console.error('写入Windows临时目录失败:', error)
      // 尝试写入用户目录
      tempPath = path.join(os.homedir(), 'vdd_temp.xml')
      try {
        fs.writeFileSync(tempPath, xml, 'utf8')
        console.log('已将临时文件写入用户目录:', tempPath)
      } catch (innerError) {
        console.error('写入用户目录也失败:', innerError)
        throw new Error('无法创建临时文件: ' + error.message)
      }
    }

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

      // 使用优化后的管道命令通知驱动重启
      try {
        await execPipeCmd('RELOAD_DRIVER')
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
      resolutions: JSON.stringify(settings.resolutions[0].resolution.map((res) => `${res.width[0]}x${res.height[0]}`))
        .replace(/","/g, ',\n    ') // 替换逗号并添加换行和缩进
        .replace(/\["/, '[\n    ') // 去除开头双引号
        .replace(/"\]/, '\n  ]'), // 去除结尾双引号
      fps: JSON.stringify(settings.global.g_refresh_rate || [60]),
    }

    const urlPort = 1 + (Number(mergedConfig.port) || 47989)

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

  ipcMain.handle('vdd:execPipeCmd', async (_, command) => {
    return await execPipeCmd(command)
  })
}

export { registerVddHandlers, execPipeCmd }
