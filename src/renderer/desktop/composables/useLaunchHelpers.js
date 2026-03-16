import { ref, computed, watch } from 'vue'

const STORAGE_KEY = 'foundation-launch-helpers'

// 内置工具模板
const BUILTIN_TEMPLATES = [
  {
    id: 'controller-mapper',
    name: '手柄映射',
    icon: '🎮',
    description: '启动手柄按键映射工具（如 JoyToKey、antimicro）',
    type: 'prep',
    defaultDoCmd: '"{path}" "{profile}"',
    defaultUndoCmd: 'taskkill /im "{exe}" /f',
    params: [
      { key: 'path', label: '程序路径', placeholder: 'C:\\Tools\\JoyToKey.exe', required: true },
      { key: 'profile', label: '配置文件（可选）', placeholder: '默认配置' },
    ],
  },
  {
    id: 'locale-emulator',
    name: '区域模拟',
    icon: '🌐',
    description: '用 Locale Emulator 转区启动（日文游戏常用）',
    type: 'wrapper',
    wrapTemplate: '"{path}" /runas',
    params: [
      { key: 'path', label: 'LEProc 路径', placeholder: 'C:\\Tools\\Locale Emulator\\LEProc.exe', required: true },
    ],
  },
  {
    id: 'translator',
    name: '实时翻译',
    icon: '📝',
    description: '启动翻译工具（如 Textractor）并在游戏结束后关闭',
    type: 'prep',
    defaultDoCmd: '"{path}"',
    defaultUndoCmd: 'taskkill /im "{exe}" /f',
    params: [
      { key: 'path', label: '程序路径', placeholder: 'C:\\Tools\\Textractor\\x86\\Textractor.exe', required: true },
    ],
  },
  {
    id: 'game-trainer',
    name: '游戏修改器',
    icon: '🔧',
    description: '启动修改器/Trainer（如 WeMod、FLiNG）并在游戏结束后关闭',
    type: 'prep',
    defaultDoCmd: '"{path}"',
    defaultUndoCmd: 'taskkill /im "{exe}" /f',
    params: [
      { key: 'path', label: '修改器路径', placeholder: 'C:\\Tools\\WeMod\\WeMod.exe', required: true },
    ],
  },
  {
    id: 'custom',
    name: '自定义工具',
    icon: '⚙️',
    description: '自定义启动前/后执行的命令',
    type: 'prep',
    defaultDoCmd: '',
    defaultUndoCmd: '',
    params: [
      { key: 'doCmd', label: '启动命令', placeholder: '游戏启动前执行', required: true },
      { key: 'undoCmd', label: '关闭命令', placeholder: '游戏退出后执行' },
    ],
  },
]

/**
 * 从路径提取 exe 文件名
 */
function extractExeName(filePath) {
  if (!filePath) return ''
  return filePath.replace(/\\/g, '/').split('/').pop() || ''
}

/**
 * 构建实际的 do 命令
 */
function buildDoCmd(template, params) {
  if (template.id === 'custom') {
    return params.doCmd || ''
  }
  let cmd = template.defaultDoCmd || ''
  for (const p of template.params) {
    cmd = cmd.replaceAll(`{${p.key}}`, params[p.key] || '')
  }
  // 替换 exe 名
  cmd = cmd.replaceAll('{exe}', extractExeName(params.path))
  // 清理空引号对
  return cmd.replace(/""\s*/g, '').trim()
}

/**
 * 构建实际的 undo 命令
 */
function buildUndoCmd(template, params) {
  if (template.id === 'custom') {
    return params.undoCmd || ''
  }
  let cmd = template.defaultUndoCmd || ''
  for (const p of template.params) {
    cmd = cmd.replaceAll(`{${p.key}}`, params[p.key] || '')
  }
  cmd = cmd.replaceAll('{exe}', extractExeName(params.path))
  return cmd.replace(/""\s*/g, '').trim()
}

// 全局状态（跨组件共享）
const globalToolPaths = ref({})
const appHelpers = ref({})
const helperPanelOpen = ref(false)
let _loaded = false

function loadState() {
  if (_loaded) return
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const data = JSON.parse(raw)
      globalToolPaths.value = data.globalToolPaths || {}
      appHelpers.value = data.appHelpers || {}
    }
  } catch (e) {
    console.warn('Failed to load launch helpers state:', e)
  }
  _loaded = true
}

function saveState() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({
    globalToolPaths: globalToolPaths.value,
    appHelpers: appHelpers.value,
  }))
}

export function useLaunchHelpers() {
  loadState()

  const templates = computed(() => BUILTIN_TEMPLATES)

  /**
   * 获取全局工具路径
   */
  function getGlobalPath(templateId, paramKey) {
    return globalToolPaths.value[`${templateId}.${paramKey}`] || ''
  }

  /**
   * 设置全局工具路径
   */
  function setGlobalPath(templateId, paramKey, value) {
    globalToolPaths.value = {
      ...globalToolPaths.value,
      [`${templateId}.${paramKey}`]: value,
    }
    saveState()
  }

  /**
   * 获取某个应用的启动助手配置
   */
  function getAppHelpers(appName) {
    return appHelpers.value[appName] || []
  }

  /**
   * 设置某个应用的启动助手配置
   * helpers: [{ templateId, enabled, params: { key: value } }]
   */
  function setAppHelpers(appName, helpers) {
    appHelpers.value = {
      ...appHelpers.value,
      [appName]: helpers,
    }
    saveState()
  }

  /**
   * 检查某个应用是否有启用的启动助手
   */
  function hasActiveHelpers(appName) {
    const helpers = getAppHelpers(appName)
    return helpers.some(h => h.enabled)
  }

  /**
   * 获取某应用启用的助手图标列表
   */
  function getActiveHelperIcons(appName) {
    const helpers = getAppHelpers(appName)
    return helpers
      .filter(h => h.enabled)
      .map(h => {
        const tmpl = BUILTIN_TEMPLATES.find(t => t.id === h.templateId)
        return tmpl?.icon || '⚙️'
      })
  }

  /**
   * 为某个应用生成 prep-cmd 和 cmd 包装
   * 返回 { prepCmds: [{do, undo, elevated}], wrapCmd: fn(originalCmd) => wrappedCmd }
   */
  function generateAppCommands(appName) {
    const helpers = getAppHelpers(appName)
    const prepCmds = []
    let wrapCmd = null

    for (const helper of helpers) {
      if (!helper.enabled) continue
      const template = BUILTIN_TEMPLATES.find(t => t.id === helper.templateId)
      if (!template) continue

      // 合并全局路径和应用特定参数
      const mergedParams = {}
      for (const p of template.params) {
        mergedParams[p.key] = helper.params?.[p.key] || getGlobalPath(template.id, p.key) || ''
      }

      if (template.type === 'prep' || template.type === 'bg') {
        const doCmd = buildDoCmd(template, mergedParams)
        const undoCmd = buildUndoCmd(template, mergedParams)
        if (doCmd) {
          prepCmds.push({ do: doCmd, undo: undoCmd, elevated: false })
        }
      } else if (template.type === 'wrapper') {
        const wrapperPath = mergedParams.path
        if (wrapperPath) {
          wrapCmd = (originalCmd) => {
            let wrap = template.wrapTemplate || ''
            wrap = wrap.replaceAll('{path}', wrapperPath)
            return `${wrap} ${originalCmd}`
          }
        }
      }
    }

    return { prepCmds, wrapCmd }
  }

  /**
   * 将启动助手配置应用到 app 对象，生成可保存的 app 数据
   */
  function applyHelpersToApp(app) {
    const { prepCmds, wrapCmd } = generateAppCommands(app.name)
    const result = { ...app }

    // 标记由启动助手管理的 prep-cmd（用注释区分）
    const MARKER = '## launch-helper'
    const existingPrepCmds = (result['prep-cmd'] || []).filter(
      c => !c.do?.includes(MARKER) && !c.undo?.includes(MARKER)
    )

    const helperPrepCmds = prepCmds.map(c => ({
      do: `${c.do} ${MARKER}`,
      undo: c.undo ? `${c.undo} ${MARKER}` : '',
      elevated: c.elevated,
    }))

    result['prep-cmd'] = [...helperPrepCmds, ...existingPrepCmds]

    if (wrapCmd && result.cmd) {
      // 保存原始命令用于还原
      if (!result._originalCmd) {
        result._originalCmd = result.cmd
      }
      result.cmd = wrapCmd(result._originalCmd || result.cmd)
    }

    return result
  }

  return {
    templates,
    globalToolPaths,
    helperPanelOpen,
    getGlobalPath,
    setGlobalPath,
    getAppHelpers,
    setAppHelpers,
    hasActiveHelpers,
    getActiveHelperIcons,
    generateAppCommands,
    applyHelpersToApp,
  }
}
