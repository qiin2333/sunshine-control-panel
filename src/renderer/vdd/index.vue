<template>
  <div class="vdd-settings">
    <h2>虚拟显示器设置</h2>

    <el-form :model="settings" label-width="120px">
      <!-- 分辨率设置 -->
      <el-form-item label="分辨率预置">
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
            + 新增分辨率
          </el-button>
        </div>
      </el-form-item>

      <!-- 显卡设置 -->
      <el-form-item label="GPU绑定">
        <div class="setting-content">
          <el-input
            v-model="gpuFriendlyName"
            style="width: 360px"
            @blur="saveGpuEdit"
            @keyup.enter="saveGpuEdit"
          />
        </div>
      </el-form-item>

      <!-- 显示器数量 -->
      <el-form-item label="显示器数量">
        <el-input-number v-model="settings.monitors[0].count" :min="1" :max="1" />
      </el-form-item>

      <!-- 刷新率设置 -->
      <el-form-item label="刷新率预置">
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
            + 新增刷新率
          </el-button>
        </div>
      </el-form-item>

      <!-- 保存按钮 -->
      <el-form-item>
        <el-button type="primary" @click="saveSettings">保存设置</el-button>
      </el-form-item>
    </el-form>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'

const resolutionOptions = ref(new Set())
const gpuFriendlyName = ref('')
const refreshRateOptions = ref(new Set([60])) // 默认添加60Hz

// 将默认值和验证逻辑提取为常量
const MIN_REFRESH_RATE = 60
const MAX_REFRESH_RATE = 240
const RESOLUTION_PATTERN = /^\d+x\d+$/
const CHINESE_PATTERN = /[\u4e00-\u9fa5]/

// 优化初始状态设置
const initialSettings = {
  monitors: [{ count: 1 }],
  gpu: [{ friendlyname: [''] }],
  resolutions: [],
}

const settings = ref({ ...initialSettings })

// 新增状态
const showResInput = ref(false)
const showRateInput = ref(false)
const newResolution = ref('')
const newRefreshRate = ref('')

// 读取设置
const loadSettings = async () => {
  try {
    const result = await window.electron.ipcRenderer.invoke('vdd:loadSettings')
    if (!result?.success) {
      ElMessage.warning('加载默认设置')
      return
    }

    const { data } = result
    settings.value = {
      ...initialSettings,
      ...data,
    }

    // GPU数据结构处理优化
    if (Array.isArray(data.gpu) && data.gpu[0]) {
      const gpuData = data.gpu[0]
      gpuFriendlyName.value = typeof gpuData === 'string' ? gpuData : gpuData.friendlyname?.[0] || ''

      settings.value.gpu[0] = {
        friendlyname: [gpuFriendlyName.value],
      }
    }

    // 分辨率和刷新率处理优化
    const processedResolutions = new Set()
    const processedRefreshRates = new Set()

    data.resolutions?.forEach((device) => {
      device.resolution?.forEach((res) => {
        res.width?.forEach((w, i) => {
          const h = res.height?.[i]
          if (w && h) processedResolutions.add(`${w}x${h}`)
        })
        res.refresh_rate?.forEach((rate) => processedRefreshRates.add(rate))
      })
    })

    resolutionOptions.value = processedResolutions
    refreshRateOptions.value = processedRefreshRates

    ElMessage.success('设置加载成功')
  } catch (error) {
    console.error('加载设置错误:', error)
    ElMessage.error('加载设置失败')
  }
}

// 保存设置
const saveSettings = async () => {
  try {
    if (CHINESE_PATTERN.test(gpuFriendlyName.value)) {
      ElMessage.error('保存失败：GPU名称不能包含中文')
      return
    }

    const settingsToSave = {
      ...settings.value,
      gpu: [
        {
          friendlyname: [gpuFriendlyName.value],
        },
      ],
      resolutions: Array.from(resolutionOptions.value).map((res) => {
        const [width, height] = res.split('x').map(Number)
        return {
          resolution: [
            {
              width: [width],
              height: [height],
              refresh_rate: Array.from(refreshRateOptions.value).map(Number),
            },
          ],
        }
      }),
    }

    const payload = JSON.parse(JSON.stringify(settingsToSave))

    console.log('Payload structure:', structuredClone(payload))

    const result = await window.electron.ipcRenderer.invoke('vdd:saveSettings', payload)

    if (result?.success) {
      ElMessage.success('设置已保存')
    } else {
      throw new Error(result?.message || '未知错误')
    }
  } catch (error) {
    console.error('保存设置错误:', error)
    ElMessage.error(`保存失败: ${error.message}`)
  }
}

// 分辨率相关方法
const validateResolution = (value) => {
  return RESOLUTION_PATTERN.test(value)
}

const addResolution = () => {
  const value = newResolution.value.trim()
  if (!validateResolution(value)) {
    ElMessage.warning('请输入正确的分辨率格式，例如：1920x1080')
    newResolution.value = ''
    return
  }
  resolutionOptions.value.add(value)
  newResolution.value = ''
  showResInput.value = false
  ElMessage.success(`已添加分辨率 ${value}`)
}

const removeResolution = (value) => {
  resolutionOptions.value.delete(value)
  ElMessage.info(`已移除分辨率 ${value}`)
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
    ElMessage.warning('请输入有效的刷新率（60-240）')
    newRefreshRate.value = ''
    return
  }
  const rate = parseInt(value)
  if (rate < MIN_REFRESH_RATE || rate > MAX_REFRESH_RATE) {
    ElMessage.warning('刷新率范围应在60-240之间')
    return
  }
  if (refreshRateOptions.value.has(rate)) {
    ElMessage.warning('该刷新率已存在')
    newRefreshRate.value = ''
    return
  }
  refreshRateOptions.value.add(rate)
  newRefreshRate.value = ''
  showRateInput.value = false
  ElMessage.success(`已添加刷新率 ${rate}Hz`)
}

const removeRefreshRate = (value) => {
  refreshRateOptions.value.delete(value)
  ElMessage.info(`已移除刷新率 ${value}Hz`)
}

const handleRateInputConfirm = () => {
  if (newRefreshRate.value) {
    addRefreshRate()
  }
  showRateInput.value = false
}

// 实现保存GPU设置方法
const saveGpuEdit = () => {
  // 添加中文校验
  if (CHINESE_PATTERN.test(gpuFriendlyName.value)) {
    ElMessage.error('GPU名称不能包含中文')
    gpuFriendlyName.value = ''
    return
  }

  settings.value.gpu[0].friendlyname = [gpuFriendlyName.value]
  ElMessage.success('GPU名称已更新')
}

onMounted(() => {
  loadSettings()
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
