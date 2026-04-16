<template>
  <SectionPanel :title="t.rtssTool.osdControl">
    <template #icon><ChatDotRound /></template>
    <template #actions>
      <button v-if="hasCli" class="overlay-btn" @click="onToggleOverlay">
        {{ t.rtssTool.toggleOverlay }}
      </button>
    </template>

    <!-- 格式工具栏 -->
    <div class="format-toolbar">
      <div class="format-group">
        <button class="fmt-btn" @click="insertTag('B')" title="Bold"><b>B</b></button>
        <button class="fmt-btn" @click="insertTag('I')" title="Italic"><i>I</i></button>
      </div>
      <div class="format-group">
        <label class="color-btn" :title="t.rtssTool.textColor">
          <span class="color-preview" :style="{ background: '#' + color }"></span>
          <span>A</span>
          <input type="color" :value="'#' + color" @input="onColorPick" class="hidden-input" />
        </label>
        <button class="fmt-btn" @click="insertColorTag" :title="t.rtssTool.insertColor"><Brush /></button>
      </div>
      <div class="format-group">
        <select v-model="fontSize" class="fmt-select" @change="insertSizeTag" :title="t.rtssTool.fontSize">
          <option :value="0">{{ t.rtssTool.defaultSize }}</option>
          <option :value="12">12px</option>
          <option :value="16">16px</option>
          <option :value="20">20px</option>
          <option :value="24">24px</option>
          <option :value="32">32px</option>
        </select>
      </div>
      <div class="format-group">
        <button class="fmt-btn fmt-tag" @click="insertRawTag('<C=>')" title="Color">&lt;C=&gt;</button>
        <button class="fmt-btn fmt-tag" @click="insertRawTag('<S=>')" title="Size">&lt;S=&gt;</button>
        <button class="fmt-btn fmt-tag" @click="insertRawTag('\\n')" title="Newline">↵</button>
      </div>
    </div>

    <div class="osd-input-group">
      <label class="field-label">{{ t.rtssTool.osdText }}</label>
      <textarea
        ref="textarea"
        v-model="text"
        class="osd-textarea"
        :placeholder="t.rtssTool.osdPlaceholder"
        rows="4"
      ></textarea>
      <div class="osd-hint">{{ t.rtssTool.osdHint }}</div>
    </div>

    <div class="actions">
      <button @click="applyOsd" class="apply-btn" :disabled="applying">
        {{ applying ? t.rtssTool.applying : t.rtssTool.applyOsd }}
      </button>
      <button @click="clearOsd" class="reset-btn" :disabled="applying">
        {{ t.rtssTool.clearOsd }}
      </button>
    </div>
  </SectionPanel>
</template>

<script setup>
import { ref } from 'vue'
import { ChatDotRound, Brush } from '@element-plus/icons-vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../../../desktop/i18n/index.js'
import SectionPanel from '../../components/SectionPanel.vue'

const { t } = useI18n()
const emit = defineEmits(['message'])

defineProps({
  hasCli: { type: Boolean, default: false },
})

const text = ref('')
const applying = ref(false)
const textarea = ref(null)
const color = ref('FFFFFF')
const fontSize = ref(0)

// ─── 格式标签插入 ───
function insertTag(tag) {
  const ta = textarea.value
  if (!ta) return
  const start = ta.selectionStart
  const end = ta.selectionEnd
  const val = text.value
  const selected = val.substring(start, end)
  const insertion = `<${tag}>${selected}<${tag}0>`
  text.value = val.substring(0, start) + insertion + val.substring(end)
  const newPos = start + tag.length + 2 + selected.length
  requestAnimationFrame(() => { ta.focus(); ta.setSelectionRange(newPos, newPos) })
}

function onColorPick(e) {
  color.value = e.target.value.replace('#', '').toUpperCase()
}

function insertColorTag() {
  insertRawTag(`<C=${color.value}>`)
}

function insertSizeTag() {
  if (fontSize.value > 0) insertRawTag(`<S=${fontSize.value}>`)
}

function insertRawTag(tag) {
  const ta = textarea.value
  if (!ta) return
  const start = ta.selectionStart
  const val = text.value
  text.value = val.substring(0, start) + tag + val.substring(start)
  const newPos = start + tag.length
  requestAnimationFrame(() => { ta.focus(); ta.setSelectionRange(newPos, newPos) })
}

// ─── OSD 操作 ───
async function applyOsd() {
  applying.value = true
  try {
    await invoke('rtss_set_osd', { text: text.value, owner: 'Sunshine' })
    emit('message', t.value.rtssTool.osdApplied, 'success')
  } catch (e) {
    emit('message', String(e), 'error')
  } finally {
    applying.value = false
  }
}

async function clearOsd() {
  applying.value = true
  try {
    await invoke('rtss_clear_osd', { owner: 'Sunshine' })
    text.value = ''
    emit('message', t.value.rtssTool.osdCleared, 'success')
  } catch (e) {
    emit('message', String(e), 'error')
  } finally {
    applying.value = false
  }
}

async function onToggleOverlay() {
  try {
    const result = await invoke('rtss_toggle_overlay')
    emit('message',
      result === '1' ? t.value.rtssTool.overlayShown : t.value.rtssTool.overlayHidden,
      'success'
    )
  } catch (e) {
    emit('message', String(e), 'error')
  }
}
</script>

<style lang="less" scoped>
.overlay-btn {
  padding: 4px 10px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.85);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  flex-shrink: 0;

  &:hover { background: rgba(255, 255, 255, 0.14); color: white; }
}

// Format toolbar
.format-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 6px 8px;
  background: rgba(0, 0, 0, 0.12);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
}

.format-group {
  display: flex;
  align-items: center;
  gap: 3px;
  padding-right: 6px;
  border-right: 1px solid rgba(255, 255, 255, 0.1);

  &:last-child { border-right: none; padding-right: 0; }
}

.fmt-btn {
  padding: 4px 8px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;
  min-width: 28px;
  text-align: center;

  &:hover { background: rgba(255, 255, 255, 0.16); color: white; }
}

.fmt-tag {
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 11px;
  padding: 4px 6px;
}

.color-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
  cursor: pointer;
  position: relative;

  &:hover { background: rgba(255, 255, 255, 0.16); color: white; }
}

.color-preview {
  width: 14px;
  height: 14px;
  border-radius: 4px;
  border: 1px solid rgba(255, 255, 255, 0.25);
}

.hidden-input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
  pointer-events: none;
}

.fmt-select {
  padding: 4px 6px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
  cursor: pointer;
  outline: none;

  option { background: #1e1e2e; color: white; }
}

// OSD input
.osd-input-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field-label {
  font-size: 12px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.8);
}

.osd-textarea {
  width: 100%;
  padding: 8px 10px;
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 10px;
  color: white;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 12px;
  line-height: 1.5;
  resize: vertical;
  outline: none;
  transition: border-color 0.2s;
  box-sizing: border-box;

  &:focus { border-color: rgba(96, 165, 250, 0.6); box-shadow: 0 0 0 2px rgba(96, 165, 250, 0.15); }
  &::placeholder { color: rgba(255, 255, 255, 0.3); }
}

.osd-hint {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
}

// Actions
.actions {
  display: flex;
  gap: 8px;
  justify-content: center;
}

.apply-btn {
  padding: 6px 20px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, #60a5fa, #818cf8);
  color: white;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { opacity: 0.9; transform: translateY(-1px); }
  &:disabled { opacity: 0.4; cursor: not-allowed; transform: none; }
}

.reset-btn {
  padding: 6px 16px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { background: rgba(255, 255, 255, 0.14); color: white; }
  &:disabled { opacity: 0.4; cursor: not-allowed; }
}
</style>
