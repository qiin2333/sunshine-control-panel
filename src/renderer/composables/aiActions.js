/**
 * AI 操作解析与执行
 * 负责解析 AI 返回的 JSON 操作指令，并执行对 Sunshine 的修改
 */

import { ElMessage } from 'element-plus'
import { useI18n } from '../desktop/i18n/index.js'

function strings() {
  const { locale } = useI18n()
  if (locale.value === 'zh') {
    return {
      fetchAppsFailed: '获取应用列表失败: {status}',
      saveFailed: '保存失败: {status}',
      logsHeader: '最近的 Sunshine 日志（最后 {count} 行）：',
      appsHeader: '当前已配置的应用列表：',
      existingMenuCommands: '现有菜单命令:',
      invalidAction: '无效操作',
      unknownAction: '未知操作类型: {action}',
      appNotFound: '未找到应用 "{name}"',
      menuAddedToast: '已添加菜单命令：{names}',
      menuAdded: '✅ 已成功添加 {count} 条菜单命令到 "{name}"：{names}',
      prepAddedToast: '已添加 {count} 条预处理命令',
      prepAdded: '✅ 已成功添加 {count} 条预处理命令到 "{name}"',
      noConfigChanges: '无配置修改',
      fetchConfigFailed: '获取配置失败: {status}',
      saveConfigFailed: '保存配置失败: {status}',
      configUpdated: '配置已更新',
      configApplied: '✅ 已应用修改：{explanation}',
      noAppsToEnhance: '无应用需要增强',
      appsEnhancedToast: '已增强 {count} 个应用的配置',
      appsEnhanced: '✅ 已成功增强 {updated}/{total} 个应用的配置',
      join: '、',
    }
  }
  return {
    fetchAppsFailed: 'Failed to fetch apps: {status}',
    saveFailed: 'Failed to save: {status}',
    logsHeader: 'Recent Sunshine logs (last {count} lines):',
    appsHeader: 'Currently configured apps:',
    existingMenuCommands: 'Existing menu commands:',
    invalidAction: 'Invalid action',
    unknownAction: 'Unknown action type: {action}',
    appNotFound: 'App "{name}" was not found',
    menuAddedToast: 'Added menu commands: {names}',
    menuAdded: '✅ Added {count} menu commands to "{name}": {names}',
    prepAddedToast: 'Added {count} pre-processing commands',
    prepAdded: '✅ Added {count} pre-processing commands to "{name}"',
    noConfigChanges: 'No configuration changes',
    fetchConfigFailed: 'Failed to fetch configuration: {status}',
    saveConfigFailed: 'Failed to save configuration: {status}',
    configUpdated: 'Configuration updated',
    configApplied: '✅ Applied changes: {explanation}',
    noAppsToEnhance: 'No apps to enhance',
    appsEnhancedToast: 'Enhanced {count} app configurations',
    appsEnhanced: '✅ Enhanced {updated}/{total} app configurations',
    join: ', ',
  }
}

function format(template, values = {}) {
  return template.replace(/\{(\w+)\}/g, (_, key) => values[key] ?? '')
}

/**
 * 获取 Sunshine API 代理地址
 */
async function getProxyUrl() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    return await invoke('get_proxy_url_command')
  } catch {
    return 'https://localhost:47990'
  }
}

/**
 * 获取当前应用列表
 */
async function fetchApps() {
  const proxyUrl = await getProxyUrl()
  const resp = await fetch(`${proxyUrl}/api/apps`)
  if (!resp.ok) throw new Error(format(strings().fetchAppsFailed, { status: resp.status }))
  const data = await resp.json()
  return { apps: data.apps || data || [], proxyUrl }
}

/**
 * 保存单个应用（匹配 Sunshine API 格式）
 */
async function saveApp(proxyUrl, apps, appIndex, app) {
  const editApp = { index: appIndex, ...app }
  const resp = await fetch(`${proxyUrl}/api/apps`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ apps, editApp }),
  })
  if (!resp.ok) throw new Error(format(strings().saveFailed, { status: resp.status }))
}

/**
 * 生成随机 ID（10位字母数字）
 */
function generateId() {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
  return Array.from({ length: 10 }, () => chars[Math.floor(Math.random() * chars.length)]).join('')
}

/**
 * 获取 Sunshine 最近日志，供 AI 分析使用
 * 只取最后若干行，避免 token 过多
 */
export async function getLogsContext(maxLines = 150) {
  try {
    const proxyUrl = await getProxyUrl()
    const resp = await fetch(`${proxyUrl}/api/logs`, { headers: { 'X-Log-Offset': '0' } })
    if (!resp.ok) return ''
    const text = await resp.text()
    const lines = text.split('\n')
    const recent = lines.slice(-maxLines).join('\n')
    const msg = strings()
    return `\n\n${format(msg.logsHeader, { count: Math.min(lines.length, maxLines) })}\n\`\`\`\n${recent}\n\`\`\``
  } catch {
    return ''
  }
}

/**
 * 获取当前应用列表摘要，供 AI 上下文使用
 */
export async function getAppsContext() {
  try {
    const { apps } = await fetchApps()
    const summary = apps
      .map((a) => {
        const menuCmds = (a['menu-cmd'] || []).map((c) => `  - ${c.name}: ${c.cmd}`).join('\n')
        const msg = strings()
        return `- ${a.name}${a.cmd ? ` (cmd: ${a.cmd})` : ''}${menuCmds ? `\n  ${msg.existingMenuCommands}\n${menuCmds}` : ''}`
      })
      .join('\n')
    return `\n\n${strings().appsHeader}\n${summary}`
  } catch {
    return ''
  }
}

/**
 * 从 AI 回复中解析 JSON 操作指令
 * @returns {object|null} 解析出的操作对象，或 null
 */
export function parseAction(message) {
  try {
    const jsonMatch = message.match(/```json\s*([\s\S]*?)\s*```/) || message.match(/(\{[\s\S]*"action"[\s\S]*\})/)
    if (!jsonMatch) return null

    const action = JSON.parse(jsonMatch[1])
    const validActions = ['add_menu_cmd', 'add_prep_cmd', 'modify_config', 'enhance_apps']
    if (validActions.includes(action.action)) return action
    return null
  } catch {
    return null
  }
}

/**
 * 执行 AI 建议的操作
 * @returns {string} 操作结果描述
 */
export async function executeAction(action) {
  if (!action) throw new Error(strings().invalidAction)

  switch (action.action) {
    case 'add_menu_cmd':
      return applyMenuCmd(action)
    case 'add_prep_cmd':
      return applyPrepCmd(action)
    case 'enhance_apps':
      return applyEnhanceApps(action)
    case 'modify_config':
      return applyConfigChange(action)
    default:
      throw new Error(format(strings().unknownAction, { action: action.action }))
  }
}

/**
 * 添加菜单命令
 */
async function applyMenuCmd(action) {
  const { apps, proxyUrl } = await fetchApps()
  const msg = strings()
  const targetName = action.app_name || 'Desktop'
  const appIndex = apps.findIndex((a) => a.name === targetName)
  if (appIndex === -1) throw new Error(format(msg.appNotFound, { name: targetName }))

  const app = apps[appIndex]
  if (!app['menu-cmd']) app['menu-cmd'] = []

  for (const cmd of action.commands) {
    const existIdx = app['menu-cmd'].findIndex((c) => c.name === cmd.name)
    const newCmd = {
      id: generateId(),
      name: cmd.name,
      cmd: cmd.cmd,
      elevated: cmd.elevated || 'false',
    }
    if (existIdx >= 0) {
      app['menu-cmd'][existIdx] = { ...app['menu-cmd'][existIdx], ...newCmd, id: app['menu-cmd'][existIdx].id }
    } else {
      app['menu-cmd'].push(newCmd)
    }
  }

  await saveApp(proxyUrl, apps, appIndex, app)

  const cmdNames = action.commands.map((c) => c.name).join(msg.join)
  ElMessage.success(format(msg.menuAddedToast, { names: cmdNames }))
  return `${format(msg.menuAdded, { count: action.commands.length, name: targetName, names: cmdNames })}\n\n${action.explanation || ''}`
}

/**
 * 添加预处理命令
 */
async function applyPrepCmd(action) {
  const { apps, proxyUrl } = await fetchApps()
  const msg = strings()
  const targetName = action.app_name || 'Desktop'
  const appIndex = apps.findIndex((a) => a.name === targetName)
  if (appIndex === -1) throw new Error(format(msg.appNotFound, { name: targetName }))

  const app = apps[appIndex]
  if (!app['prep-cmd']) app['prep-cmd'] = []

  for (const cmd of action.commands) {
    app['prep-cmd'].push({
      do: cmd.do || '',
      undo: cmd.undo || '',
      elevated: cmd.elevated || 'false',
    })
  }

  await saveApp(proxyUrl, apps, appIndex, app)

  ElMessage.success(format(msg.prepAddedToast, { count: action.commands.length }))
  return `${format(msg.prepAdded, { count: action.commands.length, name: targetName })}\n\n${action.explanation || ''}`
}

/**
 * 修改 Sunshine 配置
 */
async function applyConfigChange(action) {
  const msg = strings()
  if (!action?.changes) throw new Error(msg.noConfigChanges)

  const proxyUrl = await getProxyUrl()

  // 获取当前配置
  const getResp = await fetch(`${proxyUrl}/api/config`)
  if (!getResp.ok) throw new Error(format(msg.fetchConfigFailed, { status: getResp.status }))
  const currentConfig = await getResp.json()

  // 合并修改
  const updates = {}
  for (const change of action.changes) {
    updates[change.key] = change.value
  }

  // 保存配置
  const saveResp = await fetch(`${proxyUrl}/api/config`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ...currentConfig, ...updates }),
  })
  if (!saveResp.ok) throw new Error(format(msg.saveConfigFailed, { status: saveResp.status }))

  ElMessage.success(action.explanation || msg.configUpdated)
  return format(msg.configApplied, { explanation: action.explanation || msg.configUpdated })
}

/**
 * 批量增强应用配置
 */
async function applyEnhanceApps(action) {
  const msg = strings()
  if (!action?.apps?.length) throw new Error(msg.noAppsToEnhance)

  const { apps, proxyUrl } = await fetchApps()
  let updatedCount = 0

  for (const enhancedApp of action.apps) {
    const appIndex = apps.findIndex((a) => a.name === enhancedApp.name)
    if (appIndex === -1) continue

    const app = apps[appIndex]

    if (enhancedApp['prep-cmd']?.length) {
      if (!app['prep-cmd']) app['prep-cmd'] = []
      for (const cmd of enhancedApp['prep-cmd']) {
        app['prep-cmd'].push({
          do: cmd.do || '',
          undo: cmd.undo || '',
          elevated: cmd.elevated || 'false',
        })
      }
    }

    if (enhancedApp['menu-cmd']?.length) {
      if (!app['menu-cmd']) app['menu-cmd'] = []
      for (const cmd of enhancedApp['menu-cmd']) {
        const existIdx = app['menu-cmd'].findIndex((c) => c.name === cmd.name)
        const newCmd = {
          id: generateId(),
          name: cmd.name,
          cmd: cmd.cmd,
          elevated: cmd.elevated || 'false',
        }
        if (existIdx >= 0) {
          app['menu-cmd'][existIdx] = { ...app['menu-cmd'][existIdx], ...newCmd, id: app['menu-cmd'][existIdx].id }
        } else {
          app['menu-cmd'].push(newCmd)
        }
      }
    }

    const editApp = { index: appIndex, ...app }
    const saveResp = await fetch(`${proxyUrl}/api/apps`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ apps, editApp }),
    })
    if (saveResp.ok) updatedCount++
  }

  ElMessage.success(format(msg.appsEnhancedToast, { count: updatedCount }))
  return `${format(msg.appsEnhanced, { updated: updatedCount, total: action.apps.length })}\n\n${action.explanation || ''}`
}
