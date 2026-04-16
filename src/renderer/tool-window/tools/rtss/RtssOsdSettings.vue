<template>
  <SectionPanel icon="⚙️" :title="t.rtssTool.osdSettings">
    <template #actions>
      <button class="refresh-btn" @click="load" :title="t.rtssTool.refresh">↻</button>
    </template>

    <div class="prop-grid">
      <div class="prop-row">
        <label>{{ t.rtssTool.osdEnabled }}</label>
        <ToggleSwitch :modelValue="props.osd_enabled === 1" @update:modelValue="v => setProp('OSD', v ? '1' : '0')" />
      </div>
      <div class="prop-row">
        <label>{{ t.rtssTool.showOwnStats }}</label>
        <ToggleSwitch :modelValue="props.show_own_stats === 1" @update:modelValue="v => setProp('OSDShowOwnStatistics', v ? '1' : '0')" />
      </div>
      <div class="prop-row">
        <label>{{ t.rtssTool.osdZoom }}</label>
        <div class="zoom-btns">
          <button
            v-for="z in [1, 2, 3, 4]"
            :key="z"
            class="preset-btn small"
            :class="{ active: props.zoom === z }"
            @click="setProp('OnScreenDisplayZoom', String(z))"
          >
            {{ z }}x
          </button>
        </div>
      </div>
      <div class="prop-row">
        <label>{{ t.rtssTool.osdPosition }}</label>
        <div class="pos-inputs">
          <div class="pos-field">
            <span class="pos-label">X</span>
            <input
              type="number"
              :value="props.position_x"
              @change="e => setProp('OnScreenDisplayX', e.target.value)"
              class="pos-input"
              min="0"
            />
          </div>
          <div class="pos-field">
            <span class="pos-label">Y</span>
            <input
              type="number"
              :value="props.position_y"
              @change="e => setProp('OnScreenDisplayY', e.target.value)"
              class="pos-input"
              min="0"
            />
          </div>
        </div>
      </div>
      <div class="prop-row">
        <label>{{ t.rtssTool.coordSpace }}</label>
        <select
          :value="props.coordinate_space"
          @change="e => setProp('OSDCoordinateSpace', e.target.value)"
          class="fmt-select wide"
        >
          <option :value="0">{{ t.rtssTool.coordFramebuffer }}</option>
          <option :value="1">{{ t.rtssTool.coordScreen }}</option>
        </select>
      </div>
    </div>
  </SectionPanel>
</template>

<script setup>
import { reactive, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../../../desktop/i18n/index.js'
import SectionPanel from '../../components/SectionPanel.vue'
import ToggleSwitch from '../../components/ToggleSwitch.vue'

const { t } = useI18n()
const emit = defineEmits(['message'])

const props = reactive({
  osd_enabled: null,
  show_own_stats: null,
  position_x: null,
  position_y: null,
  zoom: null,
  coordinate_space: null,
})

const PROP_MAP = {
  OSD: 'osd_enabled',
  OSDShowOwnStatistics: 'show_own_stats',
  OnScreenDisplayX: 'position_x',
  OnScreenDisplayY: 'position_y',
  OnScreenDisplayZoom: 'zoom',
  OSDCoordinateSpace: 'coordinate_space',
}

async function load() {
  try {
    const data = await invoke('rtss_get_osd_properties', { profile: null })
    Object.assign(props, data)
  } catch (e) {
    console.warn('获取 OSD 属性失败:', e)
  }
}

async function setProp(key, value) {
  try {
    await invoke('rtss_set_osd_property', { key, value, profile: null })
    if (PROP_MAP[key]) {
      props[PROP_MAP[key]] = parseInt(value)
    }
    emit('message', t.value.rtssTool.propApplied, 'success')
  } catch (e) {
    emit('message', String(e), 'error')
  }
}

onMounted(() => load())

defineExpose({ load })
</script>

<style lang="less" scoped>
.refresh-btn {
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.7);
  font-size: 16px;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 8px;
  transition: all 0.2s;

  &:hover { color: white; background: rgba(255, 255, 255, 0.1); }
}

.prop-grid {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.prop-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;

  > label {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.8);
    flex-shrink: 0;
  }
}

.zoom-btns {
  display: flex;
  gap: 4px;
}

.preset-btn {
  padding: 4px 10px;
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.8);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover { background: rgba(255, 255, 255, 0.14); color: white; }
  &.active { background: rgba(147, 197, 253, 0.25); border-color: #93c5fd; color: #c4b5fd; }
  &.small { padding: 3px 8px; font-size: 12px; }
}

.pos-inputs {
  display: flex;
  gap: 8px;
}

.pos-field {
  display: flex;
  align-items: center;
  gap: 4px;
}

.pos-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  font-weight: 600;
}

.pos-input {
  width: 60px;
  padding: 4px 8px;
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  color: white;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  outline: none;
  text-align: center;

  &:focus { border-color: rgba(96, 165, 250, 0.6); box-shadow: 0 0 0 2px rgba(96, 165, 250, 0.15); }
  &::-webkit-inner-spin-button,
  &::-webkit-outer-spin-button { -webkit-appearance: none; margin: 0; }
}

.fmt-select {
  padding: 4px 8px;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.2);
  color: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  cursor: pointer;
  outline: none;

  &.wide { min-width: 80px; }
  option { background: #1e1e2e; color: white; }
}
</style>
