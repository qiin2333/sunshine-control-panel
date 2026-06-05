import { ref, computed, watch } from 'vue'

const STORAGE_KEY = 'foundation-launch-helpers'

// Template ID 到 i18n key 的映射
const TEMPLATE_I18N_MAP = {
  'controller-mapper': 'controllerMapper',
  'locale-emulator': 'localeEmulator',
  'translator': 'translator',
  'game-trainer': 'gameTrainer',
  'rtss-fps-limiter': 'rtssLimiter',
  'custom': 'custom',
}

// 内置工具模板（仅保留结构性数据，用户可见文字通过 i18n 覆盖）
const BUILTIN_TEMPLATES = [
  {
    id: 'controller-mapper',
    type: 'prep',
    defaultDoCmd: '"{path}" "{profile}"',
    defaultUndoCmd: 'taskkill /im "{exe}" /f',
    params: [
      { key: 'path', placeholder: 'C:\\Tools\\JoyToKey.exe', required: true },
      { key: 'profile', placeholder: '' },
      { key: 'extraArgs', placeholder: '-minimized --profile xxx' },
    ],
  },
  {
    id: 'locale-emulator',
    type: 'wrapper',
    wrapTemplate: '"{path}" /runas',
    params: [
      { key: 'path', placeholder: 'C:\\Tools\\Locale Emulator\\LEProc.exe', required: true },
      { key: 'extraArgs', placeholder: '/runas /profile xxx' },
    ],
  },
  {
    id: 'translator',
    type: 'prep',
    defaultDoCmd: '"{path}"',
    defaultUndoCmd: 'taskkill /im "{exe}" /f',
    params: [
      { key: 'path', placeholder: 'C:\\Tools\\Textractor\\x86\\Textractor.exe', required: true },
      { key: 'extraArgs', placeholder: '--lang=ja' },
    ],
  },
  {
    id: 'game-trainer',
    type: 'prep',
    defaultDoCmd: '"{path}"',
    defaultUndoCmd: 'taskkill /im "{exe}" /f',
    params: [
      { key: 'path', placeholder: 'C:\\Tools\\WeMod\\WeMod.exe', required: true },
      { key: 'extraArgs', placeholder: '--minimized' },
    ],
  },
  {
    id: 'rtss-fps-limiter',
    type: 'prep',
    defaultDoCmd: '"{path}" limit:set {fps}',
    defaultUndoCmd: '"{path}" limit:set 0',
    elevated: true,
    params: [
      { key: 'path', placeholder: 'C:\\Program Files (x86)\\RivaTuner Statistics Server\\rtss-cli.exe', required: true },
      { key: 'fps', placeholder: '60', required: true },
      { key: 'extraArgs', placeholder: '' },
    ],
  },
  {
    id: 'custom',
    type: 'prep',
    defaultDoCmd: '',
    defaultUndoCmd: '',
    params: [
      { key: 'doCmd', placeholder: '', required: true },
      { key: 'undoCmd', placeholder: '' },
    ],
  },
]

/**
 * 用 i18n 文本填充模板的用户可见字段
 * @param {object} t - i18n translation object (t.launchHelper.templates.*)
 */
function resolveTemplateI18n(templates, t) {
  if (!t?.launchHelper?.templates) return templates
  return templates.map(tmpl => {
    const i18nKey = TEMPLATE_I18N_MAP[tmpl.id]
    const tr = t.launchHelper.templates[i18nKey]
    if (!tr) return tmpl
    const resolved = { ...tmpl, name: tr.name, description: tr.description }
    if (tr.typeLabel) resolved.typeLabel = tr.typeLabel
    if (tr.typeDesc) resolved.typeDesc = tr.typeDesc
    // 解析 params 的 label/placeholder
    resolved.params = tmpl.params.map(p => {
      const paramTr = tr[`${p.key}Label`]
      const placeholderTr = tr[`${p.key}Placeholder`]
      return {
        ...p,
        label: paramTr || p.label || p.key,
        ...(placeholderTr ? { placeholder: placeholderTr } : {}),
      }
    })
    return resolved
  })
}

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
    if (p.key === 'extraArgs') continue // extraArgs 单独处理
    // 用 split/join 替代 replaceAll，避免 $ 在替换串中被当作特殊模式
    cmd = cmd.split(`{${p.key}}`).join(params[p.key] || '')
  }
  // 替换 exe 名
  cmd = cmd.split('{exe}').join(extractExeName(params.path))
  // 清理空引号对（仅匹配独立的 "" ，避免误伤正常内容）
  cmd = cmd.replace(/(?<=\s|^)""(?=\s|$)/g, '').trim()
  // 附加额外参数
  if (params.extraArgs) {
    cmd = `${cmd} ${params.extraArgs}`
  }
  return cmd.trim()
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
    // 用 split/join 避免 $ 特殊模式解释
    cmd = cmd.split(`{${p.key}}`).join(params[p.key] || '')
  }
  cmd = cmd.split('{exe}').join(extractExeName(params.path))
  return cmd.replace(/(?<=\s|^)""(?=\s|$)/g, '').trim()
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

export function useLaunchHelpers(t) {
  loadState()

  const templates = computed(() => {
    const tVal = t?.value || t
    return resolveTemplateI18n(BUILTIN_TEMPLATES, tVal)
  })

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
   * 验证助手配置，返回错误列表
   * @param {Array} helpers - editingHelpers 数组
   * @returns {{ templateId: string, paramKey: string, message: string }[]}
   */
  function validateHelpers(helpers) {
    const tVal = t?.value || t
    const requiredMsg = tVal?.launchHelper?.fieldRequired || '不能为空'
    const resolvedTemplates = templates.value
    const errors = []
    for (const helper of helpers) {
      if (!helper.enabled) continue
      const tmpl = resolvedTemplates.find(tt => tt.id === helper.templateId)
      if (!tmpl) continue
      for (const p of tmpl.params) {
        if (!p.required) continue
        const val = helper.params?.[p.key] || getGlobalPath(tmpl.id, p.key)
        if (!val || !val.trim()) {
          errors.push({
            templateId: helper.templateId,
            paramKey: p.key,
            message: `${tmpl.name}: "${p.label}" ${requiredMsg}`,
          })
        }
      }
    }
    return errors
  }

  function getActiveHelperIds(appName) {
    return getAppHelpers(appName)
      .filter(h => h.enabled)
      .map(h => h.templateId)
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
          prepCmds.push({ do: doCmd, undo: undoCmd, elevated: !!template.elevated })
        }
      } else if (template.type === 'wrapper') {
        const wrapperPath = mergedParams.path
        if (wrapperPath) {
          const extra = mergedParams.extraArgs || ''
          wrapCmd = (originalCmd) => {
            let wrap = template.wrapTemplate || ''
            // 用 split/join 避免 $ 特殊模式解释
            wrap = wrap.split('{path}').join(wrapperPath)
            if (extra) wrap = `${wrap} ${extra}`
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

    // 标记由启动助手管理的 prep-cmd（用 REM 注释区分）
    const MARKER = '& REM launch-helper'
    const existingPrepCmds = (result['prep-cmd'] || []).filter(
      c => !c.do?.includes('REM launch-helper') && !c.undo?.includes('REM launch-helper')
    )

    const helperPrepCmds = prepCmds.map(c => ({
      do: `${c.do} ${MARKER}`,
      undo: c.undo ? `${c.undo} ${MARKER}` : '',
      elevated: c.elevated ? 'true' : 'false',
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

  /**
   * 构建预览命令列表（供面板显示）
   */
  function buildPreviewCommands(helpers) {
    const tVal = t?.value || t
    const lh = tVal?.launchHelper || {}
    const resolvedTemplates = templates.value
    const cmds = []
    for (const helper of helpers) {
      if (!helper.enabled) continue
      const tmpl = resolvedTemplates.find(tt => tt.id === helper.templateId)
      if (!tmpl) continue

      const mergedParams = {}
      const rawTmpl = BUILTIN_TEMPLATES.find(tt => tt.id === helper.templateId)
      for (const p of (rawTmpl || tmpl).params) {
        mergedParams[p.key] = helper.params?.[p.key] || getGlobalPath(tmpl.id, p.key) || ''
      }

      if (tmpl.type === 'wrapper') {
        const wrapPath = mergedParams.path
        if (wrapPath) {
          let wrapCmd = (rawTmpl || tmpl).wrapTemplate || ''
          wrapCmd = wrapCmd.split('{path}').join(wrapPath)
          if (mergedParams.extraArgs) wrapCmd = `${wrapCmd} ${mergedParams.extraArgs}`
          cmds.push({ label: `${tmpl.name} ${lh.previewWrapper || '(包装启动命令)'}`, value: `${wrapCmd} <游戏命令>` })
        }
      } else if (tmpl.id === 'custom') {
        if (mergedParams.doCmd) cmds.push({ label: lh.previewBefore || '(启动前)', value: mergedParams.doCmd })
        if (mergedParams.undoCmd) cmds.push({ label: lh.previewAfter || '(退出后)', value: mergedParams.undoCmd })
      } else {
        const doCmd = buildDoCmd(rawTmpl || tmpl, mergedParams)
        const undoCmd = buildUndoCmd(rawTmpl || tmpl, mergedParams)
        const elevatedTag = tmpl.elevated ? ' 🛡️' : ''
        if (doCmd) cmds.push({ label: `${tmpl.name} ${lh.previewBefore || '(启动前)'}${elevatedTag}`, value: doCmd })
        if (undoCmd) cmds.push({ label: `${tmpl.name} ${lh.previewAfter || '(退出后)'}${elevatedTag}`, value: undoCmd })
      }
    }
    return cmds
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
    validateHelpers,
    getActiveHelperIds,
    generateAppCommands,
    applyHelpersToApp,
    buildPreviewCommands,
  }
}
