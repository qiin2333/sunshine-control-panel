import { ipcMain } from 'electron'
import fs from 'fs'
import path from 'path'
import { parseStringPromise, Parser, Builder } from 'xml2js'
import sudo from 'sudo-prompt'

const VDD_SETTINGS_PATH = path.join(process.env.SystemDrive, 'VirtualDisplayDriver', 'vdd_settings.xml')

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

    // 验证文件是否写入
    if (fs.existsSync(VDD_SETTINGS_PATH)) {
      console.log('文件验证成功，文件大小:', fs.statSync(VDD_SETTINGS_PATH).size + '字节')

      // 通过命名管道通知驱动重启
      const reloadCommand = `powershell -Command "$pipe = New-Object System.IO.Pipes.NamedPipeClientStream('.', 'ZakoVDDPipe', [System.IO.Pipes.PipeDirection]::Out); $pipe.Connect(5000); $writer = New-Object System.IO.StreamWriter($pipe); $writer.WriteLine('RELOAD_DRIVER'); $writer.Flush(); $pipe.Dispose();"`

      console.log('执行驱动重启命令:', reloadCommand)
      await new Promise((resolve, reject) => {
        sudo.exec(reloadCommand, options, (error, stdout, stderr) => {
          console.log('驱动重启命令输出:', stdout)
          console.log('驱动重启命令错误:', stderr)
          error ? reject(error) : resolve()
        })
      })
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
