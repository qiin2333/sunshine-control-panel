import fs from 'fs'
import { SUNSHINE_CONF_PATH } from './paths.js'

// 解析sunshine.conf文件
export async function parseSunshineConf() {
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
