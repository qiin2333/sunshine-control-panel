<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="open" class="helper-overlay" @click="$emit('close')"></div>
    </Transition>

    <Transition name="slide">
      <div v-if="open" class="helper-panel" @click.stop @keydown.escape="$emit('close')">
        <div class="panel-header">
          <h2>{{ t.launchHelper.title }}</h2>
          <span class="app-name-tag">{{ appName }}</span>
          <button class="close-btn" @click="$emit('close')">✕</button>
        </div>

        <div class="panel-body">
          <p class="panel-desc">{{ t.launchHelper.description }}<br/>
            <small>{{ t.launchHelper.orderHint }}</small>
          </p>

          <div
            v-for="(helper, idx) in editingHelpers"
            :key="helper.templateId"
            class="helper-card"
            :class="{ active: helper.enabled }"
          >
            <div class="helper-header">
              <!-- 排序按钮（仅启用时显示） -->
              <div v-if="helper.enabled" class="order-buttons">
                <button
                  class="order-btn"
                  :disabled="idx === 0"
                  @click.stop="moveHelper(idx, -1)"
                  :title="t.launchHelper.moveUp"
                  data-focusable
                >▲</button>
                <button
                  class="order-btn"
                  :disabled="idx === editingHelpers.length - 1"
                  @click.stop="moveHelper(idx, 1)"
                  :title="t.launchHelper.moveDown"
                  data-focusable
                >▼</button>
              </div>
              <div v-else class="order-placeholder"></div>

              <span class="helper-icon" @click="toggleHelper(helper.templateId)">{{ getTemplate(helper.templateId)?.icon }}</span>
              <div class="helper-info" @click="toggleHelper(helper.templateId)">
                <span class="helper-name">
                  {{ getTemplate(helper.templateId)?.name }}
                  <span v-if="getTemplate(helper.templateId)?.typeLabel" class="type-badge" :class="{ elevated: getTemplate(helper.templateId)?.elevated }" :title="getTemplate(helper.templateId)?.typeDesc">
                    {{ getTemplate(helper.templateId)?.typeLabel }}
                  </span>
                </span>
                <span class="helper-desc">{{ getTemplate(helper.templateId)?.description }}</span>
              </div>
              <div class="helper-switch" :class="{ on: helper.enabled }" @click="toggleHelper(helper.templateId)">
                <div class="switch-thumb"></div>
              </div>
            </div>

            <Transition name="expand">
              <div v-if="helper.enabled" class="helper-config">
                <div
                  v-for="param in getTemplate(helper.templateId)?.params"
                  :key="param.key"
                  class="config-field"
                >
                  <label class="field-label">
                    {{ param.label }}
                    <span v-if="param.required" class="required">*</span>
                  </label>
                  <div class="field-input-row">
                    <input
                      type="text"
                      class="field-input"
                      :class="{
                        'field-error': hasFieldError(helper.templateId, param.key),
                        'field-warn': pathWarnings[`${helper.templateId}.${param.key}`]
                      }"
                      :placeholder="param.placeholder || ''"
                      :value="getParamValue(helper.templateId, param.key)"
                      @input="setParamValue(helper.templateId, param.key, $event.target.value)"
                      data-focusable
                    />
                    <button
                      v-if="param.key === 'path' && hasTauri"
                      class="browse-btn"
                      @click="browseFile(helper.templateId, param.key)"
                      :title="t.launchHelper.browse"
                      data-focusable
                    ><FolderOpened /></button>
                  </div>
                  <div v-if="hasFieldError(helper.templateId, param.key)" class="field-error-msg">
                    ⚠ {{ getFieldError(helper.templateId, param.key) }}
                  </div>
                  <div v-else-if="pathWarnings[`${helper.templateId}.${param.key}`]" class="field-path-warn">
                    ⚠ {{ pathWarnings[`${helper.templateId}.${param.key}`] }}
                  </div>
                  <div v-else-if="getGlobalHint(helper.templateId, param.key)" class="field-hint">
                    {{ t.launchHelper.globalDefault }}{{ getGlobalHint(helper.templateId, param.key) }}
                  </div>
                </div>
              </div>
            </Transition>
          </div>

          <!-- 预览生成的命令 -->
          <div v-if="hasAnyEnabled" class="preview-section">

          <!-- 全局路径保存通知 -->
          <Transition name="fade">
            <div v-if="globalSaveNotice" class="global-save-notice">
              ✓ {{ globalSaveNotice }}
            </div>
          </Transition>
            <div class="preview-header" @click="showPreview = !showPreview">
              <span>{{ t.launchHelper.preview }}</span>
              <span class="preview-toggle">{{ showPreview ? '▼' : '▶' }}</span>
            </div>
            <div v-if="showPreview" class="preview-content">
              <div v-for="(cmd, i) in previewCommands" :key="i" class="preview-cmd">
                <div class="cmd-label">{{ cmd.label }}</div>
                <code class="cmd-code">{{ cmd.value }}</code>
              </div>
            </div>
          </div>
        </div>

        <div class="panel-footer">
          <button class="btn-secondary" @click="$emit('close')">{{ t.launchHelper.cancel }}</button>
          <button class="btn-primary" @click="handleSave" :disabled="saving">
            {{ saving ? t.launchHelper.saving : t.launchHelper.save }}
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { FolderOpened } from '@element-plus/icons-vue'
import { useLaunchHelpers } from '../composables/useLaunchHelpers.js'
import { useI18n } from '../i18n/index.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  appName: { type: String, default: '' },
  app: { type: Object, default: () => ({}) },
  proxyUrl: { type: String, default: '' },
})

const emit = defineEmits(['close', 'saved'])

const { t } = useI18n()

const {
  templates,
  getGlobalPath,
  setGlobalPath,
  getAppHelpers,
  setAppHelpers,
  validateHelpers,
  generateAppCommands,
  buildPreviewCommands,
} = useLaunchHelpers(t)

const hasTauri = ref(false)
const saving = ref(false)
const showPreview = ref(false)
const validationErrors = ref([]) // { templateId, paramKey, message }[]
const pathWarnings = ref({}) // { "templateId.paramKey": "message" }
const globalSaveNotice = ref('') // 全局路径自动保存通知

// 编辑中的助手列表（本地副本）
const editingHelpers = ref([])

// 当面板打开/应用变化时，加载配置
watch(() => [props.open, props.appName], ([isOpen]) => {
  if (isOpen && props.appName) {
    const stored = getAppHelpers(props.appName)
    // 为每个模板创建一个条目
    editingHelpers.value = templates.value.map(tmpl => {
      const existing = stored.find(h => h.templateId === tmpl.id)
      return {
        templateId: tmpl.id,
        enabled: existing?.enabled || false,
        params: { ...(existing?.params || {}) },
      }
    })
  }
}, { immediate: true })

function isEnabled(templateId) {
  const h = editingHelpers.value.find(h => h.templateId === templateId)
  return h?.enabled || false
}

function getTemplate(templateId) {
  return templates.value.find(t => t.id === templateId)
}

function toggleHelper(templateId) {
  const h = editingHelpers.value.find(h => h.templateId === templateId)
  if (h) h.enabled = !h.enabled
}

function moveHelper(index, direction) {
  const target = index + direction
  if (target < 0 || target >= editingHelpers.value.length) return
  const arr = [...editingHelpers.value]
  ;[arr[index], arr[target]] = [arr[target], arr[index]]
  editingHelpers.value = arr
}

function getParamValue(templateId, paramKey) {
  const h = editingHelpers.value.find(h => h.templateId === templateId)
  return h?.params?.[paramKey] || getGlobalPath(templateId, paramKey)
}

function setParamValue(templateId, paramKey, value) {
  const h = editingHelpers.value.find(h => h.templateId === templateId)
  if (h) {
    h.params = { ...h.params, [paramKey]: value }
    // 仅当全局路径为空时才自动回写，避免 app 级修改污染其他应用
    if (paramKey === 'path' && value && !getGlobalPath(templateId, paramKey)) {
      setGlobalPath(templateId, paramKey, value)
      const tmpl = getTemplate(templateId)
      globalSaveNotice.value = (t.launchHelper.globalSaved || '').replace('{name}', tmpl?.name || templateId)
      setTimeout(() => { globalSaveNotice.value = '' }, 3000)
    }
    // 清除该字段的验证错误
    validationErrors.value = validationErrors.value.filter(
      e => !(e.templateId === templateId && e.paramKey === paramKey)
    )
    // 异步校验路径是否存在
    if (paramKey === 'path' && value) {
      checkPathExists(templateId, paramKey, value)
    } else if (paramKey === 'path') {
      delete pathWarnings.value[`${templateId}.${paramKey}`]
    }
  }
}

let _pathCheckTimer = null
async function checkPathExists(templateId, paramKey, filePath) {
  clearTimeout(_pathCheckTimer)
  const key = `${templateId}.${paramKey}`
  _pathCheckTimer = setTimeout(async () => {
    if (!hasTauri.value) return
    try {
      const { exists } = await import('@tauri-apps/plugin-fs')
      const found = await exists(filePath)
      if (!found) {
        pathWarnings.value = { ...pathWarnings.value, [key]: t.launchHelper.fileNotFound || '文件不存在' }
      } else {
        const { [key]: _, ...rest } = pathWarnings.value
        pathWarnings.value = rest
      }
    } catch {
      // fs plugin 不可用，忽略
    }
  }, 500) // 延迟 500ms 防抖
}

function getGlobalHint(templateId, paramKey) {
  const h = editingHelpers.value.find(h => h.templateId === templateId)
  const localVal = h?.params?.[paramKey]
  const globalVal = getGlobalPath(templateId, paramKey)
  // 只在有全局默认且本地没有覆盖时显示
  if (globalVal && !localVal) return globalVal
  return ''
}

function hasFieldError(templateId, paramKey) {
  return validationErrors.value.some(e => e.templateId === templateId && e.paramKey === paramKey)
}

function getFieldError(templateId, paramKey) {
  return validationErrors.value.find(e => e.templateId === templateId && e.paramKey === paramKey)?.message || ''
}

async function browseFile(templateId, paramKey) {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const path = await open({
      filters: [
        { name: t.launchHelper.executableFiles || 'Executables', extensions: ['exe', 'bat', 'cmd', 'lnk', 'com', 'scr'] },
        { name: t.launchHelper.allFiles || 'All Files', extensions: ['*'] },
      ],
    })
    if (path) {
      setParamValue(templateId, paramKey, path)
    }
  } catch (e) {
    console.warn('File dialog not available:', e)
  }
}

const hasAnyEnabled = computed(() => editingHelpers.value.some(h => h.enabled))

const previewCommands = computed(() => {
  return buildPreviewCommands(editingHelpers.value)
})

async function handleSave() {
  // 验证必填字段
  const errors = validateHelpers(editingHelpers.value)
  validationErrors.value = errors
  if (errors.length > 0) {
    return
  }

  saving.value = true
  try {
    // 1. 保存到本地
    setAppHelpers(props.appName, editingHelpers.value)

    // 2. 生成 prep-cmd 并保存到服务器
    const { prepCmds, wrapCmd } = generateAppCommands(props.appName)

    // 获取当前所有应用数据
    const resp = await fetch(`${props.proxyUrl}/api/apps`)
    if (!resp.ok) throw new Error('Failed to fetch apps')
    const data = await resp.json()
    const allApps = data.apps || data || []

    const appIndex = allApps.findIndex(a => a.name === props.appName)
    if (appIndex === -1) throw new Error('App not found')

    const app = { ...allApps[appIndex] }
    const MARKER = '& REM launch-helper'

    // 移除旧的 helper prep-cmd
    app['prep-cmd'] = (app['prep-cmd'] || []).filter(
      c => !c.do?.includes('REM launch-helper') && !c.undo?.includes('REM launch-helper')
    )

    // 添加新的 helper prep-cmd
    const helperPrepCmds = prepCmds.map(c => ({
      do: `${c.do} ${MARKER}`,
      undo: c.undo ? `${c.undo} ${MARKER}` : '',
      elevated: c.elevated ? 'true' : 'false',
    }))
    app['prep-cmd'] = [...helperPrepCmds, ...app['prep-cmd']]

    // 应用命令包装器
    if (wrapCmd) {
      const originalCmd = app._originalCmd || app.cmd
      app.cmd = wrapCmd(originalCmd)
    }

    // 删除内部字段，不发给服务器
    delete app._originalCmd

    // 按 Sunshine API 格式保存：{ apps: [...], editApp: { ...app, index } }
    const editApp = { ...app, index: appIndex }
    const saveResp = await fetch(`${props.proxyUrl}/api/apps`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ apps: allApps, editApp }),
    })

    if (!saveResp.ok) throw new Error('Failed to save app')

    emit('saved')
    emit('close')
  } catch (e) {
    console.error('Failed to save launch helpers:', e)
    alert(`${t.launchHelper.saveFailed || 'Save failed: '}${e.message}`)
  } finally {
    saving.value = false
  }
}

onMounted(async () => {
  try {
    await import('@tauri-apps/api/core')
    hasTauri.value = true
  } catch { hasTauri.value = false }
})

// Escape 键全局监听（面板打开时）
function onKeydown(e) {
  if (e.key === 'Escape' && props.open) {
    emit('close')
  }
}
watch(() => props.open, (open) => {
  if (open) {
    document.addEventListener('keydown', onKeydown)
  } else {
    document.removeEventListener('keydown', onKeydown)
  }
})
onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown)
})
</script>

<style lang="less" scoped>
.helper-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 9998;
}

.helper-panel {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  width: 400px;
  max-width: 90vw;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.97);
  backdrop-filter: blur(20px);
  box-shadow: -8px 0 40px rgba(0, 0, 0, 0.5);
  z-index: 9999;
  display: flex;
  flex-direction: column;
  border-left: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
}

.panel-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);

  h2 {
    font-size: 18px;
    margin: 0;
    color: var(--fd-text-primary, #fff);
  }

  .app-name-tag {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 4px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
    color: var(--fd-accent, #00fff5);
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close-btn {
    margin-left: auto;
    background: none;
    border: none;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    cursor: pointer;
    font-size: 18px;
    padding: 4px 8px;
    border-radius: 6px;
    transition: all 0.2s;

    &:hover {
      background: rgba(255, 100, 100, 0.15);
      color: #ff6b6b;
    }
  }
}

.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 20px 24px;

  &::-webkit-scrollbar { width: 6px; }
  &::-webkit-scrollbar-thumb {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
    border-radius: 3px;
  }
}

.panel-desc {
  font-size: 13px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  margin: 0 0 20px 0;
  line-height: 1.5;
}

.helper-card {
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
  border-radius: 12px;
  margin-bottom: 12px;
  overflow: hidden;
  transition: all 0.2s ease;

  &.active {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.25);
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.03);
  }

  &:hover {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  }
}

.helper-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  cursor: pointer;
  user-select: none;

  .order-buttons {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-shrink: 0;
  }

  .order-btn {
    width: 22px;
    height: 18px;
    border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
    border-radius: 4px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.05);
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    cursor: pointer;
    font-size: 10px;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
    padding: 0;

    &:hover:not(:disabled) {
      background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
      color: var(--fd-accent, #00fff5);
      border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
    }

    &:disabled {
      opacity: 0.2;
      cursor: not-allowed;
    }
  }

  .order-placeholder {
    width: 22px;
    flex-shrink: 0;
  }

  .helper-icon {
    font-size: 24px;
    width: 36px;
    text-align: center;
  }

  .helper-info {
    flex: 1;
    min-width: 0;

    .helper-name {
      display: block;
      font-size: 14px;
      font-weight: 600;
      color: var(--fd-text-primary, #fff);
    }

    .type-badge {
      display: inline-block;
      font-size: 10px;
      font-weight: 500;
      padding: 1px 6px;
      border-radius: 4px;
      margin-left: 6px;
      vertical-align: middle;
      background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.12);
      color: var(--fd-accent, #00fff5);
      border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);

      &.elevated {
        background: rgba(251, 191, 36, 0.12);
        color: #fbbf24;
        border-color: rgba(251, 191, 36, 0.3);
      }
    }

    .helper-desc {
      display: block;
      font-size: 11px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);
      margin-top: 2px;
    }
  }
}

.helper-switch {
  width: 40px;
  height: 22px;
  border-radius: 11px;
  background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.15);
  position: relative;
  transition: all 0.2s ease;
  flex-shrink: 0;

  .switch-thumb {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
    position: absolute;
    top: 2px;
    left: 2px;
    transition: all 0.2s ease;
  }

  &.on {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.4);

    .switch-thumb {
      left: 20px;
      background: var(--fd-accent, #00fff5);
      box-shadow: 0 0 8px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.5);
    }
  }
}

.helper-config {
  padding: 0 16px 14px;
}

.config-field {
  margin-bottom: 12px;

  &:last-child { margin-bottom: 0; }
}

.field-label {
  display: block;
  font-size: 12px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  margin-bottom: 6px;

  .required {
    color: #ff6b6b;
    margin-left: 2px;
  }
}

.field-input-row {
  display: flex;
  gap: 6px;
}

.field-input {
  flex: 1;
  background: rgba(var(--fd-bg-primary-rgb, 15, 15, 35), 0.6);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.12);
  border-radius: 8px;
  padding: 8px 12px;
  font-size: 13px;
  color: var(--fd-text-primary, #fff);
  outline: none;
  transition: border-color 0.2s;

  &::placeholder {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25);
  }

  &:focus {
    border-color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.4);
  }
}

.browse-btn {
  padding: 8px 10px;
  background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  border-radius: 8px;
  cursor: pointer;
  font-size: 14px;
  transition: all 0.2s;

  &:hover {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.15);
  }
}

.field-hint {
  font-size: 11px;
  color: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.5);
  margin-top: 4px;
  font-style: italic;
}

.field-error-msg {
  font-size: 11px;
  color: #ff6b6b;
  margin-top: 4px;
}

.field-input.field-error {
  border-color: rgba(255, 107, 107, 0.6);
  box-shadow: 0 0 0 2px rgba(255, 107, 107, 0.1);
}

.field-path-warn {
  font-size: 11px;
  color: #fbbf24;
  margin-top: 4px;
}

.field-input.field-warn {
  border-color: rgba(251, 191, 36, 0.5);
}

// 命令预览
.preview-section {
  margin-top: 20px;
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.08);
  border-radius: 10px;
  overflow: hidden;
}

.global-save-notice {
  padding: 8px 14px;
  background: rgba(74, 222, 128, 0.12);
  border: 1px solid rgba(74, 222, 128, 0.3);
  border-radius: 8px;
  font-size: 12px;
  color: #4ade80;
  margin-top: 12px;
}

.preview-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  cursor: pointer;
  font-size: 13px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  user-select: none;

  &:hover { color: var(--fd-accent, #00fff5); }

  .preview-toggle { font-size: 10px; }
}

.preview-content {
  padding: 0 14px 14px;
}

.preview-cmd {
  margin-bottom: 8px;

  &:last-child { margin-bottom: 0; }

  .cmd-label {
    font-size: 11px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);
    margin-bottom: 4px;
  }

  .cmd-code {
    display: block;
    font-size: 12px;
    font-family: 'Consolas', 'Monaco', monospace;
    color: var(--fd-accent, #00fff5);
    background: rgba(0, 0, 0, 0.3);
    padding: 6px 10px;
    border-radius: 6px;
    word-break: break-all;
  }
}

// 面板底部
.panel-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 16px 24px;
  border-top: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
}

.btn-secondary {
  padding: 8px 20px;
  border-radius: 8px;
  background: transparent;
  border: 1px solid rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.15);
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.7);
  cursor: pointer;
  font-size: 13px;
  transition: all 0.2s;

  &:hover {
    border-color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
    color: var(--fd-text-primary, #fff);
  }
}

.btn-primary {
  padding: 8px 20px;
  border-radius: 8px;
  background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
  color: var(--fd-accent, #00fff5);
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  transition: all 0.2s;

  &:hover:not(:disabled) {
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
    box-shadow: 0 0 16px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}

// Transition
.slide-enter-from, .slide-leave-to {
  transform: translateX(100%);
}
.slide-enter-active, .slide-leave-active {
  transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.fade-enter-from, .fade-leave-to { opacity: 0; }
.fade-enter-active, .fade-leave-active { transition: opacity 0.2s ease; }

.expand-enter-from, .expand-leave-to {
  opacity: 0;
  max-height: 0;
  padding-top: 0;
  padding-bottom: 0;
}
.expand-enter-active, .expand-leave-active {
  transition: all 0.25s ease;
  overflow: hidden;
}
.expand-enter-to, .expand-leave-from {
  max-height: 300px;
  opacity: 1;
}
</style>
