<template>
  <section class="quality-manager ds5-page">
    <header class="quality-header">
      <div>
        <h1>{{ text.title }}</h1>
        <p>{{ text.intro }}</p>
      </div>
    </header>
    <div class="quality-surface">
      <nav class="quality-tabs" role="tablist" :aria-label="text.title">
        <button
          v-for="key in ['components', 'session', 'settings']"
          :id="`quality-tab-${key}`"
          :key="key"
          role="tab"
          :aria-controls="`quality-${key}`"
          :aria-selected="tab === key"
          @click="switchTab(key)"
        >
          {{ text[key] }}
        </button>
      </nav>
      <div class="quality-body">
        <div
          v-show="tab === 'components'"
          id="quality-components"
          role="tabpanel"
          aria-labelledby="quality-tab-components"
        >
          <template v-if="!component">
            <div class="component-cards">
              <article v-for="item in componentCards" :key="item.kind">
                <div class="component-title">
                  <h2>{{ item.name }}</h2>
                  <span>{{ componentLabel(item.kind) }}</span>
                </div>
                <p>{{ item.intro }}</p>
                <button class="quality-button" @click="component = item.kind">
                  <Box />{{ text.manage }}
                </button>
              </article>
            </div>
            <p class="quality-note">{{ text.componentHint }}</p>
          </template>
          <template v-else
            ><button
              class="quality-link"
              @click="showComponents"
            >
              <ArrowLeft />{{ text.back }}</button
            ><EnhancementManager :key="component" :kind="component"
          /></template>
        </div>
        <div
          v-show="tab === 'session'"
          id="quality-session"
          role="tabpanel"
          aria-labelledby="quality-tab-session"
        >
          <div v-if="!online" class="quality-note">
            {{ text.disconnected }}
            <button class="quality-link" @click="pollOnce">
              {{ text.refresh }}
            </button>
          </div>
          <p v-else-if="!pipelines.length" class="quality-note">
            {{ text.noSession }}
          </p>
          <template v-else>
            <label
              >{{ text.choose
              }}<select
                :value="selectedId ?? ''"
                :disabled="saving"
                @change="selectSession(Number($event.target.value))"
              >
                <option value="" disabled>{{ text.choose }}</option>
                <option v-for="p in pipelines" :key="p.id" :value="p.id">
                  #{{ p.id }} · {{ nrOutputLabel(p) }}
                </option>
              </select></label
            >
            <p v-if="selectedId && !pipeline" class="quality-note">
              {{ text.expired }}
            </p>
            <div v-if="pipeline" class="session-grid">
              <div>
                <span>{{ text.enhancement }}</span
                ><strong>{{
                  overlayText.states[nrOverlayState(pipeline, online)]
                }}</strong>
              </div>
              <div>
                <span>{{ text.scale }}</span
                ><strong>{{ pipeline.nr_scale_percent }}%</strong>
              </div>
              <div>
                <span>{{ text.output }}</span
                ><strong>{{ nrOutputLabel(pipeline) }}</strong>
              </div>
            </div>
            <p
              v-if="pipeline?.dv_profile && pipeline.dv_state !== 'active'"
              class="quality-note"
            >
              {{
                pipeline.dv_state === 'waiting'
                  ? overlayText.dvWaiting
                  : overlayText.dvFallback
              }}
            </p>
            <div class="quality-actions">
              <button
                class="quality-button primary"
                :disabled="!pipeline || saving"
                @click="setVisibility(true)"
              >
                <Monitor />{{ text.open }}</button
              ><button class="quality-button" @click="switchTab('settings')">
                {{ text.settings }}
              </button>
            </div>
          </template>
          <p class="quality-note">{{ text.sessionHint }}</p>
        </div>
        <div
          v-show="tab === 'settings'"
          id="quality-settings"
          role="tabpanel"
          aria-labelledby="quality-tab-settings"
          @keydown="captureKey"
        >
          <div class="quality-layout">
            <div>
              <div class="settings-row">
                <div>
                  <h3>{{ text.show }}</h3>
                  <p>{{ text.showHint }}</p>
                </div>
                <el-switch
                  :model-value="visible"
                  :disabled="saving || !loaded"
                  :aria-label="text.show"
                  @change="setVisibility"
                />
              </div>
              <div class="settings-row block">
                <label class="range-title" for="quality-opacity"
                  >{{ text.opacity }}<output>{{ draftOpacity }}%</output></label
                ><input
                  id="quality-opacity"
                  v-model.number="draftOpacity"
                  type="range"
                  min="35"
                  max="95"
                  :disabled="saving || !loaded"
                  @change="save({ opacity: draftOpacity })"
                />
              </div>
              <div class="shortcuts-title">
                <h3>{{ text.keys }}</h3>
                <button
                  class="quality-link"
                  :disabled="saving || !loaded"
                  @click="resetShortcuts"
                >
                  {{ text.reset }}
                </button>
              </div>
              <div
                v-for="item in shortcutRows"
                :key="item.key"
                class="shortcut-row"
              >
                <label :for="`record-${item.key}`">{{ item.label }}</label>
                <div class="shortcut-controls">
                  <button
                    :id="`record-${item.key}`"
                    class="quality-button record"
                    :class="{ recording: recording === item.key }"
                    :disabled="saving || !loaded"
                    @click="startRecording(item.key)"
                  >
                    <span>{{
                      recording === item.key
                        ? text.record
                        : displayShortcut(settings[item.key]) || text.unset
                    }}</span
                    ><Keyboard /></button
                  ><button
                    class="quality-link"
                    :disabled="saving || !loaded || !settings[item.key]"
                    @click="clearShortcut(item.key)"
                  >
                    {{ text.clear }}
                  </button>
                </div>
                <p
                  v-if="settings[item.key] && !registration[item.registered]"
                  class="quality-warning"
                >
                  {{ text.statusUnavailable }}
                </p>
              </div>
              <p class="quality-note">{{ text.keyHint }}</p>
              <p v-if="recording" class="record-hint" role="status">
                {{ text.recordHint }}
              </p>
            </div>
            <aside>
              <div class="preview-title">
                {{ text.preview }} <span>DLSS NR</span>
              </div>
              <div class="preview-stage">
                <div
                  v-if="visible"
                  class="mini-overlay"
                  :style="{
                    background: `rgb(14 16 13 / ${draftOpacity / 100})`
                  }"
                >
                  <header><Rank /><i /> DLSS NR <ArrowUp /></header>
                  <div class="mini-body">
                    <div>
                      {{ text.enhancement }}<span class="mini-toggle" />
                    </div>
                    <p>{{ text.previewState }}</p>
                    <footer>
                      <kbd>{{
                        registration.nrRegistered
                          ? displayShortcut(settings.nrShortcut)
                          : text.disabledKey
                      }}</kbd
                      ><span>{{ text.action }}</span>
                    </footer>
                  </div>
                </div>
                <span v-else>{{ text.hidden }}</span>
              </div>
              <p class="quality-note">{{ text.previewHint }}</p>
            </aside>
          </div>
        </div>
        <div
          class="quality-feedback"
          :class="{ failure: error }"
          role="status"
          aria-live="polite"
        >
          {{
            error
              ? enhancementError(text, error)
              : saved
                ? text.saved
                : text.local
          }}
        </div>
      </div>
    </div>
  </section>
</template>
<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { ArrowLeft, ArrowUp, Box, Monitor, Rank } from '@element-plus/icons-vue'
import EnhancementManager from './EnhancementManager.vue'
import { useI18n } from '../desktop/i18n/index.js'
import { dlssNr, rtxHdr } from '../tauri-adapter.js'
import {
  DEFAULT_NR_SHORTCUT,
  displayShortcut,
  shortcutFromEvent,
  enhancementControlsText,
  enhancementError
} from '../composables/enhancementControls.js'
import {
  nrOutputLabel,
  nrOverlayState,
  nrPipelines
} from '../composables/nrOverlayState.js'
import { nrOverlayText } from '../composables/nrOverlayMessages.js'
// Element Plus has no keyboard glyph; use its established input icon.
import { EditPen as Keyboard } from '@element-plus/icons-vue'
const { locale } = useI18n()
const text = computed(() => enhancementControlsText(locale.value)),
  overlayText = computed(() => nrOverlayText(locale.value))
const tab = ref('components'),
  component = ref(''),
  componentStatus = ref({})
const componentCards = computed(() => [
  { kind: 'hdr', name: 'RTX HDR', intro: text.value.hdrIntro },
  { kind: 'nr', name: 'DLSS NR', intro: text.value.nrIntro }
])
const settings = ref({ nrShortcut: '', overlayShortcut: '', opacity: 62 }),
  registration = ref({})
const draftOpacity = ref(62),
  loaded = ref(false),
  saving = ref(false),
  visible = ref(false),
  saved = ref(false),
  error = ref(''),
  recording = ref('')
const pipelines = ref([]),
  selectedId = ref(null),
  online = ref(false)
const pipeline = computed(() =>
  pipelines.value.find((p) => p.id === selectedId.value)
)
const shortcutRows = computed(() => [
  {
    key: 'overlayShortcut',
    label: text.value.visibilityKey,
    registered: 'overlayRegistered'
  },
  { key: 'nrShortcut', label: text.value.nrKey, registered: 'nrRegistered' }
])
let disposed = false,
  polling = false,
  timer,
  captureTimer,
  unlisten = []
function applyStatus(status) {
  if (disposed || !status?.settings) return
  settings.value = status.settings
  registration.value = status
  draftOpacity.value = status.settings.opacity
  selectedId.value = status.target
  loaded.value = true
}
async function save(patch) {
  if (saving.value) return
  saving.value = true
  error.value = ''
  saved.value = false
  try {
    applyStatus(await invoke('nr_overlay_save_settings', { patch }))
    saved.value = true
  } catch (e) {
    error.value = String(e)
    try {
      applyStatus(await invoke('nr_overlay_settings'))
    } catch {}
  } finally {
    saving.value = false
  }
}
async function cancelRecording() {
  clearTimeout(captureTimer)
  recording.value = ''
  try {
    await invoke('nr_overlay_capture_shortcut', { active: false })
  } catch {}
}
async function startRecording(key) {
  await cancelRecording()
  try {
    await invoke('nr_overlay_capture_shortcut', { active: true })
    if (disposed) {
      await cancelRecording()
      return
    }
    recording.value = key
    error.value = ''
    captureTimer = setTimeout(cancelRecording, 30000)
  } catch (e) {
    error.value = String(e)
  }
}
async function captureKey(event) {
  if (!recording.value) return
  const result = shortcutFromEvent(event)
  if (event.key !== 'Tab') event.preventDefault()
  if (result.cancel) {
    await cancelRecording()
    return
  }
  if (result.modifier) return
  if (result.error) {
    error.value = result.error
    return
  }
  const key = recording.value
  await cancelRecording()
  await save({ [key]: result.value })
}
async function clearShortcut(key) {
  await cancelRecording()
  await save({ [key]: '' })
}
async function resetShortcuts() {
  await cancelRecording()
  await save({ nrShortcut: DEFAULT_NR_SHORTCUT, overlayShortcut: '' })
}
function switchTab(key) {
  void cancelRecording()
  tab.value = key
  if (key === 'components') void loadComponents()
}
async function setVisibility(value) {
  try {
    await invoke('nr_overlay_set_visible', { visible: value })
    visible.value = value
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}
async function selectSession(id) {
  try {
    await invoke('nr_overlay_select_session', { id })
    selectedId.value = id
    error.value = ''
  } catch (e) {
    error.value = String(e)
  }
}
async function pollOnce() {
  if (disposed || polling) return
  polling = true
  try {
    const result = await invoke('nr_live_status')
    if (!disposed) {
      pipelines.value = nrPipelines(result?.pipelines)
      online.value = true
      if (
        loaded.value &&
        selectedId.value === null &&
        pipelines.value.length === 1
      )
        await selectSession(pipelines.value[0].id)
    }
  } catch {
    if (!disposed) {
      online.value = false
      pipelines.value = []
    }
  }
  try {
    const value = await invoke('nr_overlay_is_visible')
    if (!disposed) visible.value = value
  } catch {}
  polling = false
}
async function poll() {
  await pollOnce()
  if (!disposed) timer = setTimeout(poll, 1200)
}
function showComponents() {
  component.value = ''
  void loadComponents()
}
async function loadComponents() {
  const results = await Promise.allSettled([
    rtxHdr.getStatus(),
    dlssNr.getStatus()
  ])
  if (disposed) return
  componentStatus.value = Object.fromEntries(
    results.map((r, i) => [
      i ? 'nr' : 'hdr',
      r.status === 'fulfilled' && r.value.success
        ? r.value.data
        : { error: true }
    ])
  )
}
function componentLabel(kind) {
  const s = componentStatus.value[kind]
  return !s
    ? text.value.loading
    : s.error || !s.host_supported
      ? text.value.unavailable
      : s.ready
        ? text.value.ready
        : text.value.needsSetup
}
onMounted(async () => {
  window.addEventListener('blur', cancelRecording)
  for (const [name, handler] of [
    ['nr-settings-changed', (e) => applyStatus(e.payload)],
    [
      'nr-overlay-visibility',
      (e) => {
        visible.value = e.payload
      }
    ],
    [
      'nr-action-error',
      (e) => {
        error.value = e.payload
      }
    ]
  ]) {
    try {
      const stop = await listen(name, handler)
      if (disposed) stop()
      else unlisten.push(stop)
    } catch {}
  }
  try {
    applyStatus(await invoke('nr_overlay_settings'))
  } catch (e) {
    error.value = String(e)
  }
  if (!disposed) {
    void poll()
    void loadComponents()
  }
})
onUnmounted(() => {
  disposed = true
  clearTimeout(timer)
  void cancelRecording()
  unlisten.forEach((stop) => stop())
  window.removeEventListener('blur', cancelRecording)
})
</script>
<style scoped lang="less">
@import '../styles/DualSenseSettings.less';
.quality-manager {
  max-width: 1080px;
}
.quality-header {
  margin-bottom: 22px;
  h1 {
    margin: 0;
    font-size: 25px;
  }
  p {
    margin-top: 6px;
    color: var(--el-text-color-secondary);
  }
}
.quality-surface {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-light);
  border-radius: 12px;
  overflow: hidden;
}
.quality-tabs {
  display: flex;
  gap: 25px;
  padding: 0 24px;
  border-bottom: 1px solid var(--el-border-color-light);
  flex-wrap: wrap;
  button {
    padding: 16px 0 13px;
    color: var(--el-text-color-secondary);
    border: 0;
    border-bottom: 3px solid transparent;
    background: none;
    cursor: pointer;
    font: inherit;
  }
  button[aria-selected='true'] {
    color: var(--el-color-primary);
    border-bottom-color: var(--el-color-primary);
  }
}
.quality-body {
  padding: 24px;
}
.quality-body h2 {
  font-size: 17px;
  margin: 0;
}
.quality-body h3 {
  font-size: 14px;
  margin: 0;
}
.quality-body p {
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
.quality-body svg {
  width: 17px;
  height: 17px;
  flex-shrink: 0;
}
.quality-button,
.quality-link {
  font: inherit;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
}
.quality-button {
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  padding: 9px 13px;
}
.quality-button:hover {
  border-color: var(--el-color-primary);
}
.quality-button.primary {
  background: var(--el-color-primary);
  color: #fff;
  border-color: var(--el-color-primary);
}
.quality-link {
  padding: 5px;
  border: 0;
  background: none;
  color: var(--el-color-primary);
}
.quality-button:disabled,
.quality-link:disabled {
  opacity: 0.45;
  cursor: default;
}
.quality-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 230px;
  gap: 28px;
}
.settings-row {
  padding: 16px 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--el-border-color-light);
  gap: 15px;
  p {
    margin: 4px 0 0;
  }
  &:first-child {
    padding-top: 0;
  }
  &.block {
    display: block;
  }
}
.range-title {
  display: flex;
  justify-content: space-between;
}
input[type='range'] {
  width: 100%;
  margin: 12px 0 0;
  accent-color: var(--el-color-primary);
}
.shortcuts-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 20px;
}
.shortcut-row {
  padding: 13px 0;
  border-bottom: 1px solid var(--el-border-color-light);
}
.shortcut-controls {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}
.record {
  flex: 1;
  justify-content: space-between;
  text-align: left;
  min-width: 0;
  background: var(--el-fill-color-light);
  span {
    overflow-wrap: anywhere;
  }
}
.recording {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}
.record-hint {
  padding: 10px;
  background: var(--el-color-primary-light-9);
  border-radius: 6px;
}
.quality-note {
  font-size: 12px;
  margin-top: 14px;
}
.quality-warning,
.quality-feedback.failure {
  color: var(--el-color-danger);
}
.quality-feedback {
  min-height: 22px;
  margin-top: 18px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.preview-title {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  margin-bottom: 10px;
  color: var(--el-text-color-secondary);
}
.preview-stage {
  background: linear-gradient(145deg, #384b3b, #132024);
  border-radius: 8px;
  padding: 24px 15px;
  min-height: 210px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #d8e2d3;
}
.mini-overlay {
  width: 100%;
  border: 1px solid #8b997c88;
  color: #e4ebde;
  font-size: 12px;
  header {
    display: flex;
    gap: 7px;
    padding: 9px;
    border-bottom: 1px solid #ffffff22;
    i {
      width: 6px;
      height: 6px;
      background: #76b900;
      align-self: center;
    }
    svg:last-child {
      margin-left: auto;
    }
  }
}
.mini-body {
  padding: 12px;
  > div {
    display: flex;
    justify-content: space-between;
  }
  p {
    color: #c1cdb6;
    font-size: 11px;
  }
  footer {
    border-top: 1px solid #ffffff25;
    padding-top: 10px;
    margin-top: 14px;
    display: flex;
    gap: 7px;
    flex-wrap: wrap;
    font-size: 10px;
  }
  kbd {
    font:
      11px Consolas,
      monospace;
    overflow-wrap: anywhere;
  }
}
.mini-toggle {
  width: 29px;
  height: 17px;
  border: 3px solid #76b900;
  background: linear-gradient(90deg, #76b900 50%, #14200b 50%);
}
.component-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  article {
    padding: 20px;
    border: 1px solid var(--el-border-color-light);
    border-radius: 9px;
  }
  button {
    margin-top: 15px;
  }
}
.component-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  span {
    font-size: 12px;
    color: var(--el-text-color-secondary);
  }
}
.session-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0 24px;
  > div {
    padding: 18px 0;
    border-bottom: 1px solid var(--el-border-color-light);
    display: flex;
    gap: 15px;
    justify-content: space-between;
  }
}
select {
  display: block;
  margin-top: 8px;
  width: 100%;
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  padding: 9px;
  font: inherit;
}
.quality-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 20px;
}
@media (max-width: 760px) {
  .quality-manager {
    padding: 20px 14px;
  }
  .quality-layout,
  .component-cards,
  .session-grid {
    grid-template-columns: 1fr;
  }
  .quality-body {
    padding: 18px;
  }
  .quality-tabs {
    padding: 0 18px;
    gap: 15px;
  }
  .quality-layout aside {
    max-width: 320px;
    width: 100%;
  }
}
</style>
