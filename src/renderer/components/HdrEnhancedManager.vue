<template>
  <section class="quality-manager">
    <header class="quality-header">
      <span class="quality-title-icon" aria-hidden="true">
        <svg
          viewBox="0 0 28 28"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path
            d="M17 5.5 5.8 5.2Q3.5 5.2 3.5 7.5L3.2 21Q3.2 23.2 5.5 23.2L21 22.9Q23 22.9 23 20.7V14.5"
          />
          <path d="m5.8 19 5.1-6 4.1 4.5 2.5-2.6 3.2 3.6" />
          <path
            d="m21.5 2 .9 3.7L26 6.8l-3.6 1.1-1 3.6-1.1-3.6-3.6-1.1 3.7-1.1Z"
          />
          <path d="m7 8.8 1.4.1" opacity=".55" />
        </svg>
      </span>
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
          <p class="category-description">{{ text.componentHint }}</p>
          <div class="enhancement-stack">
            <EnhancementManager kind="nr" /><EnhancementManager kind="hdr" />
          </div>
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
            <label v-if="pipelines.length > 1 || !pipeline"
              >{{ text.choose
              }}<select
                :value="selectedId ?? ''"
                :disabled="saving"
                @change="selectSession(Number($event.target.value))"
              >
                <option value="" disabled>{{ text.choose }}</option>
                <option v-for="p in pipelines" :key="p.id" :value="p.id">
                  {{ text.stream }} #{{ p.id }} · {{ nrOutputLabel(p) }}
                </option>
              </select></label
            >
            <div
              v-if="pipeline && pipelines.length === 1"
              class="session-identity"
            >
              <strong>{{ text.stream }} #{{ pipeline.id }}</strong
              ><span>{{ text.connected }}</span>
            </div>
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
            <NrSessionControls
              v-if="pipeline"
              :pipeline="pipeline"
              :online="online"
              @changed="pollOnce"
              @error="error = $event"
            />
            <div class="quality-actions">
              <button
                class="quality-button primary"
                :disabled="!pipeline || saving"
                @click="setVisibility(true)"
              >
                <Monitor aria-hidden="true" />{{ text.open }}</button
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
                  :style="{
                    '--range-progress': ((draftOpacity - 35) / 60) * 100 + '%'
                  }"
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
              <div v-for="group in shortcutGroups" :key="group.label" class="shortcut-group">
                <h4>{{ group.label }}</h4>
                <div class="shortcut-inputs">
                  <div
                    v-for="item in group.inputs"
                    :key="item.key"
                    class="shortcut-row"
                  >
                    <label :for="`record-${item.key}`">{{ item.gamepad ? text.gamepad : text.keyboard }}</label>
                    <div class="shortcut-controls">
                      <select
                        v-if="item.gamepad"
                        :id="`record-${item.key}`"
                        :aria-label="`${group.label} · ${text.gamepad}`"
                        :value="settings[item.key]"
                        :disabled="saving || !loaded || !registration.gamepadSupported"
                        @change="save({ [item.key]: $event.target.value })"
                      >
                        <option value="">{{ text.unset }}</option>
                        <option v-for="option in FIXED_GAMEPAD_SHORTCUTS" :key="option" :value="option">
                          {{ displayGamepadShortcut(option) }}
                        </option>
                      </select>
                      <button
                        v-else
                        :id="`record-${item.key}`"
                        :aria-label="`${group.label} · ${text.keyboard}`"
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
                        ><Keyboard aria-hidden="true" /></button
                      ><button
                        v-if="!item.gamepad"
                        class="quality-link"
                        :disabled="saving || !loaded || !settings[item.key]"
                        @click="clearShortcut(item.key)"
                      >
                        {{ text.clear }}
                      </button>
                    </div>
                    <p
                      v-if="!item.gamepad && settings[item.key] && !registration[item.registered]"
                      class="quality-warning"
                      role="status"
                      aria-live="polite"
                    >
                      {{ text.statusUnavailable }}
                    </p>
                  </div>
                </div>
              </div>
              <p class="quality-note">{{ text.keyHint }}</p>
              <p class="quality-note">{{ text.gamepadHint }}</p>
              <p v-if="loaded && !registration.gamepadSupported" class="quality-note" role="status">{{ text.gamepadUnsupported }}</p>
              <p v-if="recording" class="record-hint" role="status">
                {{ text.recordHint }}
                <button class="quality-link" @click="cancelRecording">{{ text.cancel }}</button>
              </p>
            </div>
            <aside>
              <div class="preview-title">
                {{ text.preview }} <span>DLSS NR</span>
              </div>
              <div class="preview-stage">
                <div
                  class="mini-overlay"
                  :style="{
                    background: `rgb(14 16 13 / ${draftOpacity / 100})`
                  }"
                >
                  <header><Rank aria-hidden="true" /><i aria-hidden="true" /> DLSS NR <ArrowUp aria-hidden="true" /></header>
                  <div class="mini-body">
                    <div>
                      {{ text.enhancement }}<span class="mini-toggle" />
                    </div>
                    <p>{{ text.previewState }}</p>
                    <footer>
                      <kbd>{{
                        nrShortcutLabel(registration) || text.disabledKey
                      }}</kbd
                      ><span>{{ text.action }}</span>
                    </footer>
                  </div>
                </div>
              </div>
              <p class="quality-note">{{ text.previewHint }}</p>
            </aside>
          </div>
        </div>
        <div
          v-if="error || tab === 'settings'"
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
import { ArrowUp, Monitor, Rank } from '@element-plus/icons-vue'
import { displayGamepadShortcut, FIXED_GAMEPAD_SHORTCUTS } from '../composables/controllerButtons.js'
import NrSessionControls from './NrSessionControls.vue'
import EnhancementManager from './EnhancementManager.vue'
import { useI18n } from '../desktop/i18n/index.js'
import {
  DEFAULT_NR_SHORTCUT,
  displayShortcut,
  nrShortcutLabel,
  shortcutFromEvent,
  enhancementControlsText,
  enhancementError
} from '../composables/enhancementControls.js'
import {
  nrOutputLabel,
  nrOverlayState,
  nrPipelines
} from '../composables/nrOverlayState.js'
import { shortcutCapture } from '../composables/shortcutCapture.js'
import { nrOverlayText } from '../composables/nrOverlayMessages.js'
// Element Plus has no keyboard glyph; use its established input icon.
import { EditPen as Keyboard } from '@element-plus/icons-vue'
const { locale } = useI18n()
const text = computed(() => enhancementControlsText(locale.value)),
  overlayText = computed(() => nrOverlayText(locale.value))
const tab = ref('components')
const settings = ref({ nrShortcut: '', overlayShortcut: '', nrGamepad: '', overlayGamepad: '', opacity: 62 }),
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
const shortcutGroups = computed(() => [
  { label: text.value.visibilityKey, inputs: [
    { key: 'overlayShortcut', registered: 'overlayRegistered' },
    { key: 'overlayGamepad', gamepad: true }
  ] },
  { label: text.value.nrKey, inputs: [
    { key: 'nrShortcut', registered: 'nrRegistered' },
    { key: 'nrGamepad', gamepad: true }
  ] }
])
let disposed = false,
  polling = false,
  timer,
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
const capture = shortcutCapture(
  active => invoke('nr_overlay_capture_shortcut', { active }),
  key => { recording.value = key }
)
async function cancelRecording() {
  try { await capture.cancel() } catch (e) { if (!disposed) error.value = String(e) }
}
async function startRecording(key) {
  error.value = ''
  try { await capture.start(key) } catch (e) { if (!disposed) error.value = String(e) }
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
  await save({ nrShortcut: DEFAULT_NR_SHORTCUT, overlayShortcut: '', nrGamepad: '', overlayGamepad: '' })
}
function switchTab(key) {
  void cancelRecording()
  tab.value = key
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
onMounted(async () => {
  let settingsRevision = 0
  window.addEventListener('blur', cancelRecording)
  for (const [name, handler] of [
    ['nr-settings-changed', (e) => { settingsRevision++; applyStatus(e.payload) }],
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
    const revision = settingsRevision
    const status = await invoke('nr_overlay_settings')
    if (revision === settingsRevision) applyStatus(status)
  } catch (e) {
    error.value = String(e)
  }
  if (!disposed) {
    void poll()
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

<style src="../styles/EnhancementManager.css"></style>
