<template>
  <div class="chub-subsection">
    <div class="chub-section-head">
      <span class="chub-section-label">◈ {{ t.controllersHub.emulation.dsuTitle }}</span>
      <span class="chub-section-rule"></span>
    </div>
    <p class="chub-hint">{{ t.controllersHub.emulation.dsuHint }}</p>
    <div class="chub-dsu-row">
      <el-checkbox
        :model-value="config.enable_dsu_server"
        :disabled="busyKey !== ''"
        @change="(v) => saveKey('enable_dsu_server', v)"
      >{{ t.controllersHub.emulation.dsuEnable }}</el-checkbox>
      <label class="chub-dsu-port">
        <span>{{ t.controllersHub.emulation.dsuPort }}</span>
        <el-input-number
          :model-value="config.dsu_server_port"
          :min="1024"
          :max="65535"
          :disabled="!config.enable_dsu_server || busyKey !== ''"
          size="small"
          @change="(v) => saveKey('dsu_server_port', v)"
        />
      </label>
    </div>

    <div class="chub-subsection" style="margin-top: 20px">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.controllersHub.emulation.ds4Title }}</span>
        <span class="chub-section-rule"></span>
      </div>
      <p class="chub-hint">{{ t.controllersHub.emulation.ds4Hint }}</p>
      <div class="chub-check-list">
        <el-checkbox
          :model-value="config.ds4_back_as_touchpad_click"
          :disabled="busyKey !== ''"
          @change="(v) => saveKey('ds4_back_as_touchpad_click', v)"
        >{{ t.controllersHub.emulation.backAsTouchpadClick }}</el-checkbox>
        <el-checkbox
          :model-value="config.motion_as_ds4"
          :disabled="busyKey !== ''"
          @change="(v) => saveKey('motion_as_ds4', v)"
        >{{ t.controllersHub.emulation.motionAsDs4 }}</el-checkbox>
        <el-checkbox
          :model-value="config.touchpad_as_ds4"
          :disabled="busyKey !== ''"
          @change="(v) => saveKey('touchpad_as_ds4', v)"
        >{{ t.controllersHub.emulation.touchpadAsDs4 }}</el-checkbox>
      </div>
    </div>
  </div>
</template>

<script setup>
import { onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { controllerHub } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'

const { t } = useI18n()

const config = ref({
  motion_as_ds4: true,
  touchpad_as_ds4: true,
  ds4_back_as_touchpad_click: true,
  enable_dsu_server: false,
  dsu_server_port: 26760,
})
const busyKey = ref('')

async function loadConfig() {
  try {
    const result = await controllerHub.getConfig()
    if (result?.success) Object.assign(config.value, result.data)
  } catch (error) {
    console.warn('控制器高级配置读取失败:', error)
  }
}

async function saveKey(key, value) {
  if (busyKey.value !== '' || config.value[key] === value) return
  const prev = config.value[key]
  config.value[key] = value
  busyKey.value = key
  try {
    const result = await controllerHub.saveConfig({ [key]: value })
    if (result?.success) {
      ElMessage.success(result.data)
    } else {
      config.value[key] = prev
      ElMessage.error(result?.message || t.value.controllersHub.emulation.saveFailed)
    }
  } catch (error) {
    config.value[key] = prev
    ElMessage.error(String(error))
  } finally {
    busyKey.value = ''
  }
}

onMounted(loadConfig)
</script>
