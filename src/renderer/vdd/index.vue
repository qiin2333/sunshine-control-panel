<template>
  <div class="vdd-settings">
    <h2>{{ t.vddSettings.title }}</h2>

    <el-form :model="settings" label-width="120px">
      <!-- 分辨率设置 -->
      <el-form-item :label="t.vddSettings.resolutionPresets">
        <div class="setting-content">
          <el-tag v-for="res in resolutionOptions" :key="res" closable @close="removeResolution(res)" class="mx-1">
            {{ res }}
          </el-tag>
          <el-input
            v-if="showResInput"
            v-model="newResolution"
            class="input-new-tag"
            ref="resInputRef"
            @keyup.enter="addResolution"
            @blur="handleResInputConfirm"
            size="small"
            style="width: 120px"
          />
          <el-button
            v-else
            size="small"
            @click="
              () => {
                showResInput = true
                $nextTick(() => $refs.resInputRef?.focus())
              }
            "
          >
            {{ t.vddSettings.addResolution }}
          </el-button>
        </div>
      </el-form-item>

      <!-- 显卡设置 -->
      <el-form-item :label="t.vddSettings.gpuBinding">
        <div class="setting-content">
          <el-select
            v-model="gpuFriendlyName"
            filterable
            allow-create
            default-first-option
            style="width: 360px"
            @blur="saveGpuEdit"
            @keyup.enter="saveGpuEdit"
          >
            <el-option v-for="gpu in gpuOptions" :key="gpu" :label="gpu" :value="gpu" />
          </el-select>
        </div>
      </el-form-item>

      <!-- 显示器数量 -->
      <el-form-item :label="t.vddSettings.monitorCount">
        <el-input-number v-model="settings.monitors[0].count" :min="1" :max="1" />
      </el-form-item>

      <!-- 刷新率设置 -->
      <el-form-item :label="t.vddSettings.refreshRatePresets">
        <div class="setting-content">
          <el-tag v-for="rate in refreshRateOptions" :key="rate" closable @close="removeRefreshRate(rate)" class="mx-1">
            {{ rate }}Hz
          </el-tag>
          <el-input
            v-if="showRateInput"
            v-model="newRefreshRate"
            class="input-new-tag"
            ref="rateInputRef"
            @keyup.enter="addRefreshRate"
            @blur="handleRateInputConfirm"
            size="small"
            style="width: 120px"
          />
          <el-button
            v-else
            size="small"
            @click="
              () => {
                showRateInput = true
                $nextTick(() => $refs.rateInputRef?.focus())
              }
            "
          >
            {{ t.vddSettings.addRefreshRate }}
          </el-button>
        </div>
      </el-form-item>

      <!-- SDR10 -->
      <el-form-item label="SDR 10bit">
        <el-switch v-model="settings.colour[0].SDR10bit" />
      </el-form-item>

      <!-- HDR+ -->
      <el-form-item label="HDR 12bit">
        <el-switch v-model="settings.colour[0].HDRPlus" />
      </el-form-item>

      <!-- 色彩模式 -->
      <el-form-item :label="t.vddSettings.colorMode">
        <el-select v-model="settings.colour[0].ColourFormat" :placeholder="t.vddSettings.selectColorMode" style="width: 160px">
          <el-option label="RGB" value="RGB" />
          <el-option label="YCbCr444" value="YCbCr444" />
          <el-option label="YCbCr422" value="YCbCr422" />
          <el-option label="YCbCr420" value="YCbCr420" />
        </el-select>
      </el-form-item>

      <!-- 日志 -->
      <el-form-item :label="t.vddSettings.logging">
        <el-switch v-model="settings.logging[0].logging" />
      </el-form-item>

      <!-- 保存按钮 -->
      <el-form-item>
        <el-button type="primary" @click="saveSettings">{{ t.vddSettings.save }}</el-button>
      </el-form-item>
    </el-form>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { vdd } from '@/tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

const resolutionOptions = ref(new Set())
const gpuFriendlyName = ref('')
const refreshRateOptions = ref(new Set([60, 120, 240])) // 使用默认全局刷新率

// 将默认值和验证逻辑提取为常量
const MIN_REFRESH_RATE = 30
const MAX_REFRESH_RATE = 240
const RESOLUTION_PATTERN = /^\d+x\d+$/
const CHINESE_PATTERN = /[\u4e00-\u9fa5]/

// 新增GPU选项列表
const gpuOptions = ref([])

// 优化初始状态设置
const initialSettings = {
  monitors: [{ count: 1 }],
  gpu: [{ friendlyname: [''] }],
  global: {
    g_refresh_rate: [60, 120, 240],
  },
  resolutions: [],
  colour: [
    {
      SDR10bit: false,
      HDRPlus: false,
      ColourFormat: 'RGB',
    },
  ],
  logging: [{ logging: false, debuglogging: true }],
}

const settings = reactive({ ...initialSettings })

// 新增状态
const showResInput = ref(false)
const showRateInput = ref(false)
const newResolution = ref('')
const newRefreshRate = ref('')

// 读取设置
const loadSettings = async () => {
  try {
    const result = await vdd.loadSettings()
    if (!result?.success) {
      ElMessage.warning(t.value.vddSettings.loadDefault)
      return
    }

    const { data } = result
    Object.assign(settings, {
      ...initialSettings,
      ...data,
    })

    // GPU数据结构处理优化
    if (Array.isArray(data.gpu) && data.gpu[0]) {
      const gpuData = data.gpu[0]
      gpuFriendlyName.value = typeof gpuData === 'string' ? gpuData : gpuData.friendlyname?.[0] || ''

      settings.gpu[0] = {
        friendlyname: [gpuFriendlyName.value],
      }
    }

    // 分辨率处理优化
    const processedResolutions = new Set()

    data.resolutions?.forEach((device) => {
      device.resolution?.forEach((res) => {
        res.width?.forEach((w, i) => {
          const h = res.height?.[i]
          if (w && h) processedResolutions.add(`${w}x${h}`)
        })
      })
    })

    resolutionOptions.value = processedResolutions

    // 处理全局刷新率设置
    if (data.global?.g_refresh_rate) {
      refreshRateOptions.value = new Set(data.global.g_refresh_rate)
    }

    ElMessage.success(t.value.vddSettings.loadSuccess)
  } catch (error) {
    console.error('Failed to load settings:', error)
    ElMessage.error(t.value.vddSettings.loadFailed)
  }
}

// 在loadSettings方法后添加获取GPU列表的方法
const loadGPUs = async () => {
  try {
    const result = await vdd.getGPUs()
    if (result?.success) {
      gpuOptions.value = result.data
      // 如果当前选中的GPU不在列表中，则添加到选项
      if (gpuFriendlyName.value && !gpuOptions.value.includes(gpuFriendlyName.value)) {
        gpuOptions.value.unshift(gpuFriendlyName.value)
      }
    }
  } catch (error) {
    console.error('Failed to get GPUs:', error)
  }
}

// 保存设置
const saveSettings = async () => {
  try {
    if (CHINESE_PATTERN.test(gpuFriendlyName.value)) {
      ElMessage.error(t.value.vddSettings.saveGpuError)
      return
    }

    const settingsToSave = {
      ...settings,
      gpu: [
        {
          friendlyname: [gpuFriendlyName.value],
        },
      ],
      global: {
        g_refresh_rate: Array.from(refreshRateOptions.value).map(Number),
      },
      resolutions: [
        {
          resolution: Array.from(resolutionOptions.value).map((res) => {
            const [width, height] = res.split('x').map(Number)
            return {
              width: [width],
              height: [height]
            }
          }),
        },
      ],
    }

    const payload = JSON.parse(JSON.stringify(settingsToSave))

    console.log('Payload structure:', structuredClone(payload))

    const result = await vdd.saveSettings(payload)

    if (result?.success) {
      ElMessage.success(t.value.vddSettings.saveSuccess)
    } else {
      throw new Error(result?.message || 'Unknown error')
    }
  } catch (error) {
    console.error('Failed to save settings:', error)
    ElMessage.error(t.value.vddSettings.saveFailed.replace('{error}', error.message))
  }
}

// 分辨率相关方法
const validateResolution = (value) => {
  return RESOLUTION_PATTERN.test(value)
}

const addResolution = () => {
  const value = newResolution.value.trim()
  if (!validateResolution(value)) {
    ElMessage.warning(t.value.vddSettings.resolutionFormatError)
    newResolution.value = ''
    return
  }
  resolutionOptions.value.add(value)
  newResolution.value = ''
  showResInput.value = false
  ElMessage.success(t.value.vddSettings.resolutionAdded.replace('{value}', value))
}

const removeResolution = (value) => {
  if (resolutionOptions.value.size <= 1) {
    ElMessage.error(t.value.vddSettings.resolutionMinOne)
    return
  }
  resolutionOptions.value.delete(value)
  ElMessage.info(t.value.vddSettings.resolutionRemoved.replace('{value}', value))
}

const handleResInputConfirm = () => {
  if (newResolution.value) {
    addResolution()
  }
  showResInput.value = false
}

// 刷新率验证
const validateRefreshRate = (value) => {
  return /^\d+$/.test(value)
}

const addRefreshRate = () => {
  const value = newRefreshRate.value.trim()
  if (!validateRefreshRate(value)) {
    ElMessage.warning(t.value.vddSettings.refreshRateInvalid)
    newRefreshRate.value = ''
    return
  }
  const rate = parseInt(value)
  if (rate < MIN_REFRESH_RATE || rate > MAX_REFRESH_RATE) {
    ElMessage.warning(t.value.vddSettings.refreshRateRange)
    return
  }
  if (refreshRateOptions.value.has(rate)) {
    ElMessage.warning(t.value.vddSettings.refreshRateExists)
    newRefreshRate.value = ''
    return
  }
  refreshRateOptions.value.add(rate)
  newRefreshRate.value = ''
  showRateInput.value = false
  ElMessage.success(t.value.vddSettings.refreshRateAdded.replace('{value}', rate))
}

const removeRefreshRate = (value) => {
  if (refreshRateOptions.value.size <= 1) {
    ElMessage.error(t.value.vddSettings.refreshRateMinOne)
    return
  }
  refreshRateOptions.value.delete(value)
  ElMessage.info(t.value.vddSettings.refreshRateRemoved.replace('{value}', value))
}

const handleRateInputConfirm = () => {
  if (newRefreshRate.value) {
    addRefreshRate()
  }
  showRateInput.value = false
}

// 修改保存GPU设置方法
const saveGpuEdit = () => {
  if (CHINESE_PATTERN.test(gpuFriendlyName.value)) {
    ElMessage.error(t.value.vddSettings.gpuNameNoChinese)
    gpuFriendlyName.value = ''
    return
  }

  // 如果输入的是新选项，添加到列表
  if (gpuFriendlyName.value && !gpuOptions.value.includes(gpuFriendlyName.value)) {
    gpuOptions.value.unshift(gpuFriendlyName.value)
  }

      settings.gpu[0].friendlyname = [gpuFriendlyName.value]
  ElMessage.success('GPU名称已更新')
}

onMounted(() => {
  loadSettings()
  loadGPUs()
})
</script>

<style scoped>
.vdd-settings {
  padding: 20px;
}

.el-form {
  max-width: 700px;
  margin: 0 auto;
  padding: 25px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  backdrop-filter: blur(10px);
}

.setting-item {
  margin-bottom: 20px;
}

.setting-label {
  margin-bottom: 10px;
  font-weight: bold;
  color: #eee;
}

.setting-content {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
}

.input-new-tag {
  width: 120px;
  margin-left: 10px;
  vertical-align: bottom;
}

.el-tag {
  margin-right: 10px;
}

.edit-icon {
  margin-left: 5px;
  cursor: pointer;
}

.el-button {
  /* 确保按钮可交互 */
  pointer-events: auto;
  /* 修复可能被遮挡的情况 */
  position: relative;
  z-index: 1;
}
</style>
