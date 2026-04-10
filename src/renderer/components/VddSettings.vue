<template>
  <div class="vdd-settings-wrapper">
    <div class="vdd-header">
      <h2>
        <el-icon class="header-icon"><Monitor /></el-icon>
        {{ t.vddSettings.title }}
      </h2>
    </div>

    <div class="vdd-content">
      <!-- 显示器认证标识装饰 -->
      <div class="cert-badges">
        <div class="cert-badge hdr">
          <span class="cert-text">HDR</span>
          <span class="cert-sub">10bit</span>
        </div>
        <div class="cert-badge resolution">
          <span class="cert-text">4K</span>
          <span class="cert-sub">UHD</span>
        </div>
        <div class="cert-badge refresh">
          <span class="cert-text">240Hz</span>
          <span class="cert-sub">High Refresh</span>
        </div>
        <div class="cert-badge sync">
          <span class="cert-text">VRR</span>
          <span class="cert-sub">Variable Refresh</span>
        </div>
      </div>

      <el-form :model="settings" label-width="120px" class="vdd-form">
        <!-- 分辨率设置 -->
        <el-form-item :label="t.vddSettings.resolutionPresets">
          <div class="setting-content">
            <el-tag
              v-for="res in resolutionOptions"
              :key="res"
              closable
              @close="removeResolution(res)"
              class="resolution-tag"
              type="info"
            >
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
              :placeholder="t.vddSettings.resPlaceholder"
              style="width: 140px"
            />
            <el-button v-else size="small" @click="showResolutionInput" class="add-btn">
              <el-icon><Plus /></el-icon>
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
              style="width: 100%; max-width: 400px"
              :placeholder="t.vddSettings.gpuPlaceholder"
              @blur="saveGpuEdit"
              @keyup.enter="saveGpuEdit"
            >
              <el-option v-for="gpu in gpuOptions" :key="gpu" :label="gpu" :value="gpu" />
            </el-select>
          </div>
        </el-form-item>

        <!-- 显示器数量 -->
        <el-form-item :label="t.vddSettings.monitorCount">
          <el-input-number v-model="settings.monitors.count" :min="1" :max="1" disabled />
          <span class="form-tip">{{ t.vddSettings.monitorCountTip }}</span>
        </el-form-item>

        <!-- 刷新率设置 -->
        <el-form-item :label="t.vddSettings.refreshRatePresets">
          <div class="setting-content">
            <el-tag
              v-for="rate in refreshRateOptions"
              :key="rate"
              closable
              @close="removeRefreshRate(rate)"
              class="rate-tag"
              type="success"
            >
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
              :placeholder="t.vddSettings.ratePlaceholder"
              style="width: 100px"
            />
            <el-button v-else size="small" @click="showRefreshRateInput" class="add-btn">
              <el-icon><Plus /></el-icon>
              {{ t.vddSettings.addRefreshRate }}
            </el-button>
          </div>
        </el-form-item>

        <!-- 两列布局容器 -->
        <div class="two-column-layout">
          <!-- SDR10 -->
          <el-form-item :label="t.vddSettings.sdr10bit">
            <el-switch v-model="settings.colour.SDR10bit" />
            <span class="form-tip">{{ t.vddSettings.sdr10bitTip }}</span>
          </el-form-item>

          <!-- HDR+ -->
          <el-form-item :label="t.vddSettings.hdr12bit">
            <el-switch v-model="settings.colour.HDRPlus" />
            <span class="form-tip">{{ t.vddSettings.hdr12bitTip }}</span>
          </el-form-item>

          <!-- 色彩模式 -->
          <el-form-item :label="t.vddSettings.colorMode">
            <el-select v-model="settings.colour.ColourFormat" :placeholder="t.vddSettings.selectColorMode" style="width: 180px">
              <el-option label="RGB" value="RGB" />
              <el-option label="YCbCr444" value="YCbCr444" />
              <el-option label="YCbCr422" value="YCbCr422" />
              <el-option label="YCbCr420" value="YCbCr420" />
            </el-select>
          </el-form-item>

          <!-- 日志 -->
          <el-form-item :label="t.vddSettings.loggingLabel">
            <el-switch v-model="settings.logging.logging" />
            <span class="form-tip">{{ t.vddSettings.loggingTip }}</span>
          </el-form-item>

          <!-- 调试日志（仅在日志开启时显示） -->
          <el-form-item :label="t.vddSettings.debugLogging" v-if="settings.logging.logging">
            <el-switch v-model="settings.logging.debuglogging" />
            <span class="form-tip">{{ t.vddSettings.debugLoggingTip }}</span>
          </el-form-item>
        </div>

        <!-- 自定义 EDID -->
        <el-form-item :label="t.vddSettings.customEdid">
          <el-switch v-model="settings.edid.CustomEdid" @change="handleEdidToggle" />
          <span class="form-tip">{{ t.vddSettings.customEdidTip }}</span>
        </el-form-item>

        <!-- EDID 文件管理 -->
        <el-form-item :label="t.vddSettings.edidFile" v-if="settings.edid.CustomEdid">
          <div class="edid-file-manager">
            <el-alert type="warning" :closable="false" show-icon class="edid-warning">
              <template #title>
                <span class="warning-text"
                  >{{ t.vddSettings.edidWarning }}</span
                >
              </template>
            </el-alert>
            <div class="edid-status">
              <el-tag :type="edidFileExists ? 'success' : 'info'" effect="dark">
                {{ edidFileExists ? t.vddSettings.edidUploaded : t.vddSettings.edidNotUploaded }}
              </el-tag>
              <span class="edid-path" v-if="edidFilePath">{{ edidFilePath }}</span>
            </div>
            <div class="edid-actions">
              <el-upload
                ref="uploadRef"
                :auto-upload="false"
                :show-file-list="false"
                :on-change="handleEdidFileChange"
                accept=".bin"
              >
                <el-button size="small" type="primary">
                  <el-icon><Upload /></el-icon>
                  选择EDID文件
                </el-button>
              </el-upload>
              <el-button size="small" @click="downloadEdid" :disabled="!edidFileExists">
                <el-icon><Download /></el-icon>
                {{ t.vddSettings.edidDownload }}
              </el-button>
            </div>
            <div class="edid-info" v-if="edidInfo">
              <el-descriptions :column="2" size="small" border>
                <el-descriptions-item :label="t.vddSettings.edidFileSize">{{ edidInfo.size }} {{ t.vddSettings.edidBytes }}</el-descriptions-item>
                <el-descriptions-item :label="t.vddSettings.edidFormat">
                  {{ edidInfo.size === 128 ? t.vddSettings.edidFormatBasic : edidInfo.size === 256 ? t.vddSettings.edidFormatCea : t.vddSettings.edidFormatUnknown }}
                </el-descriptions-item>
                <el-descriptions-item label="Checksum" :span="2">
                  <el-tag :type="edidInfo.checksumValid ? 'success' : 'danger'" size="small">
                    {{ edidInfo.checksumValid ? t.vddSettings.edidChecksumValid : t.vddSettings.edidChecksumInvalid }}
                  </el-tag>
                </el-descriptions-item>
              </el-descriptions>
            </div>
          </div>
        </el-form-item>

        <!-- 保存按钮 -->
        <el-form-item class="form-actions">
          <el-button type="primary" @click="saveSettings" size="large">
            <el-icon><UploadFilled /></el-icon>
            {{ t.vddSettings.save }}
          </el-button>
        </el-form-item>
      </el-form>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import { Monitor, Plus, UploadFilled, Setting, Document, Upload, Download } from '@element-plus/icons-vue'
import { vdd } from '../tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

const resolutionOptions = ref(new Set())
const gpuFriendlyName = ref('')
const refreshRateOptions = ref(new Set(['60', '120', '240']))

// 常量定义
const MIN_REFRESH_RATE = 30
const MAX_REFRESH_RATE = 240
const RESOLUTION_PATTERN = /^\d+x\d+$/
const CHINESE_PATTERN = /[\u4e00-\u9fa5]/

const gpuOptions = ref([])

// 初始设置 - 匹配后端的新结构
const initialSettings = {
  monitors: { count: 1 },
  gpu: { friendlyname: '' },
  global: {
    g_refresh_rate: ['60', '120', '240'],
  },
  resolutions: { resolution: [] },
  colour: {
    SDR10bit: false,
    HDRPlus: false,
    ColourFormat: 'RGB',
  },
  logging: { logging: false, debuglogging: true },
  edid: {
    CustomEdid: false,
    PreventSpoof: false,
    EdidCeaOverride: false,
  },
}

const settings = reactive({ ...initialSettings })

// 输入状态
const showResInput = ref(false)
const showRateInput = ref(false)
const newResolution = ref('')
const newRefreshRate = ref('')
const resInputRef = ref(null)
const rateInputRef = ref(null)

// EDID 相关状态
const uploadRef = ref(null)
const edidFileExists = ref(false)
const edidFilePath = ref('')
const edidInfo = ref(null)

// 显示分辨率输入框
const showResolutionInput = () => {
  showResInput.value = true
  nextTick(() => resInputRef.value?.focus())
}

// 显示刷新率输入框
const showRefreshRateInput = () => {
  showRateInput.value = true
  nextTick(() => rateInputRef.value?.focus())
}

// 读取设置
const loadSettings = async () => {
  try {
    const result = await vdd.loadSettings()
    if (!result?.success) {
      ElMessage.warning(t.value.vddSettings.loadDefault)
      return
    }

    const { data } = result

    // 确保 colour、logging 和 edid 字段存在（它们在后端是 Option 类型）
    const mergedData = {
      ...initialSettings,
      ...data,
      colour: data.colour || initialSettings.colour,
      logging: data.logging || initialSettings.logging,
      edid: data.edid || initialSettings.edid,
    }

    Object.assign(settings, mergedData)

    // GPU数据处理 - 新结构：gpu 是单个对象
    if (data.gpu) {
      gpuFriendlyName.value = data.gpu.friendlyname || ''
      settings.gpu.friendlyname = gpuFriendlyName.value
    }

    // 分辨率处理 - 新结构：resolutions 是单个对象，包含 resolution 数组
    const processedResolutions = new Set()
    if (data.resolutions?.resolution) {
      data.resolutions.resolution.forEach((res) => {
        if (res.width && res.height) {
          processedResolutions.add(`${res.width}x${res.height}`)
        }
      })
    }
    resolutionOptions.value = processedResolutions

    // 刷新率处理 - 支持字符串格式（包括NTSC帧率如 "59.94", "29.97"）
    if (data.global?.g_refresh_rate) {
      // 确保所有值都转换为字符串格式，以支持分数刷新率
      const rateArray = data.global.g_refresh_rate.map(rate => String(rate))
      refreshRateOptions.value = new Set(rateArray)
    }

    ElMessage.success(t.value.vddSettings.loadSuccess)
  } catch (error) {
    console.error('Load settings error:', error)
    ElMessage.error(t.value.vddSettings.loadFailed)
  }
}

// 获取GPU列表
const loadGPUs = async () => {
  try {
    const result = await vdd.getGPUs()
    if (result?.success) {
      gpuOptions.value = result.data
      if (gpuFriendlyName.value && !gpuOptions.value.includes(gpuFriendlyName.value)) {
        gpuOptions.value.unshift(gpuFriendlyName.value)
      }
    }
  } catch (error) {
    console.error('Failed to get GPU list:', error)
  }
}

// 保存设置
const saveSettings = async () => {
  try {
    if (CHINESE_PATTERN.test(gpuFriendlyName.value)) {
      ElMessage.error(t.value.vddSettings.saveGpuError)
      return
    }

    // 使用新的单对象结构
    const settingsToSave = {
      ...settings,
      gpu: {
        friendlyname: gpuFriendlyName.value,
      },
      global: {
        // 保持字符串格式，支持NTSC帧率（如 "59.94", "29.97"）
        g_refresh_rate: Array.from(refreshRateOptions.value),
      },
      resolutions: {
        resolution: Array.from(resolutionOptions.value).map((res) => {
          const [width, height] = res.split('x').map(Number)
          return {
            width,
            height,
          }
        }),
      },
    }

    const payload = JSON.parse(JSON.stringify(settingsToSave))
    const result = await vdd.saveSettings(payload)

    if (result?.success) {
      ElMessage.success(t.value.vddSettings.saveSuccessDetail)
    } else {
      throw new Error(result?.message || t.value.vddSettings.unknownError)
    }
  } catch (error) {
    console.error('Save settings error:', error)
    ElMessage.error(t.value.vddSettings.saveFailed.replace('{error}', error.message))
  }
}

// 分辨率管理
const validateResolution = (value) => RESOLUTION_PATTERN.test(value)

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

// 刷新率管理
// 支持整数和分数格式（如 60, 59.94, 29.97）
const validateRefreshRate = (value) => {
  // 匹配整数或小数格式（如 60, 59.94, 29.97）
  const pattern = /^\d+(\.\d+)?$/
  if (!pattern.test(value)) {
    return false
  }
  const rate = parseFloat(value)
  // 允许范围：1-480（支持NTSC帧率如29.97, 59.94, 119.88等）
  return rate >= 1 && rate <= 480
}

const addRefreshRate = () => {
  const value = newRefreshRate.value.trim()
  if (!validateRefreshRate(value)) {
    ElMessage.warning(t.value.vddSettings.refreshRateInvalidExt)
    newRefreshRate.value = ''
    return
  }
  const rate = parseFloat(value)
  // 检查范围（允许1-480，包括NTSC帧率）
  if (rate < 1 || rate > 480) {
    ElMessage.warning(t.value.vddSettings.refreshRateRangeExt)
    return
  }
  // 使用字符串格式存储，支持分数格式
  const rateStr = value
  if (refreshRateOptions.value.has(rateStr)) {
    ElMessage.warning(t.value.vddSettings.refreshRateExists)
    newRefreshRate.value = ''
    return
  }
  refreshRateOptions.value.add(rateStr)
  newRefreshRate.value = ''
  showRateInput.value = false
  ElMessage.success(t.value.vddSettings.refreshRateAdded.replace('{value}', rateStr))
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

// GPU名称保存
const saveGpuEdit = () => {
  if (CHINESE_PATTERN.test(gpuFriendlyName.value)) {
    ElMessage.error(t.value.vddSettings.gpuNameNoChinese)
    gpuFriendlyName.value = ''
    return
  }

  if (gpuFriendlyName.value && !gpuOptions.value.includes(gpuFriendlyName.value)) {
    gpuOptions.value.unshift(gpuFriendlyName.value)
  }

  settings.gpu.friendlyname = gpuFriendlyName.value
  ElMessage.success(t.value.vddSettings.gpuUpdated)
}

// ========== EDID 管理功能 ==========

// 检查 EDID 文件是否存在
const checkEdidFile = async () => {
  try {
    const result = await vdd.getEdidFilePath()
    if (result?.success) {
      edidFilePath.value = result.data
    }

    // 尝试读取 EDID 文件
    const readResult = await vdd.readEdidFile()
    if (readResult?.success) {
      edidFileExists.value = true
      const data = readResult.data
      edidInfo.value = {
        size: data.length,
        checksumValid: validateEdidChecksum(data),
      }
    } else {
      edidFileExists.value = false
      edidInfo.value = null
    }
  } catch (error) {
    edidFileExists.value = false
    edidInfo.value = null
  }
}

// 验证 EDID checksum
const validateEdidChecksum = (data) => {
  if (!data || data.length < 128) return false

  let sum = 0
  for (let i = 0; i < 127; i++) {
    sum += data[i]
  }
  sum %= 256

  const expectedChecksum = sum !== 0 ? 256 - sum : 0
  return data[127] === expectedChecksum
}

// 处理 EDID 开关切换
const handleEdidToggle = (value) => {
  if (value && !edidFileExists.value) {
    ElMessage.warning(t.value.vddSettings.edidUploadFirst)
  }
}

// 处理 EDID 文件选择
const handleEdidFileChange = async (file) => {
  if (!file || !file.raw) {
    ElMessage.warning(t.value.vddSettings.edidSelectValid)
    return
  }

  // 检查文件大小
  const fileSize = file.raw.size
  if (fileSize !== 128 && fileSize !== 256) {
    ElMessage.error(t.value.vddSettings.edidSizeInvalid.replace('{size}', fileSize))
    return
  }

  try {
    // 读取文件内容
    const arrayBuffer = await file.raw.arrayBuffer()
    const uint8Array = new Uint8Array(arrayBuffer)

    // 验证 EDID 头部
    const expectedHeader = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]
    const headerValid = expectedHeader.every((byte, index) => uint8Array[index] === byte)

    if (!headerValid) {
      ElMessage.error(t.value.vddSettings.edidHeaderInvalid)
      return
    }

    // 验证 checksum
    const checksumValid = validateEdidChecksum(uint8Array)
    if (!checksumValid) {
      ElMessage.error(t.value.vddSettings.edidChecksumError)
      return
    }

    // 上传文件
    const result = await vdd.uploadEdidFile(Array.from(uint8Array))
    if (result?.success) {
      ElMessage.success(t.value.vddSettings.edidUploadSuccess)
      await checkEdidFile()
    } else {
      throw new Error(result?.message || t.value.vddSettings.uploadFailed)
    }
  } catch (error) {
    console.error('Upload EDID file error:', error)
    ElMessage.error(t.value.vddSettings.uploadError.replace('{error}', error.message))
  }
}

// 下载当前 EDID
const downloadEdid = async () => {
  try {
    const result = await vdd.readEdidFile()
    if (result?.success) {
      const data = new Uint8Array(result.data)
      const blob = new Blob([data], { type: 'application/octet-stream' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = 'user_edid.bin'
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)
      ElMessage.success(t.value.vddSettings.edidDownloadSuccess)
    } else {
      throw new Error(result?.message || t.value.vddSettings.readFailed)
    }
  } catch (error) {
    console.error('Download EDID file error:', error)
    ElMessage.error(t.value.vddSettings.downloadError.replace('{error}', error.message))
  }
}

onMounted(() => {
  loadSettings()
  loadGPUs()
  checkEdidFile()
})
</script>

<style lang="less" scoped>
@import '../styles/theme.less';

.vdd-settings-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

// ========== 深色模式 ==========
[data-bs-theme='dark'] {
  .vdd-header {
    border-bottom: 1px solid rgba(230, 213, 184, 0.15);
    background: linear-gradient(135deg, rgba(212, 165, 165, 0.1), rgba(230, 213, 184, 0.05));

    h2 {
      color: #e6d5b8;
      text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);

      .header-icon {
        color: @morandi-red;
      }
    }
  }

  .vdd-form {
    background: linear-gradient(135deg, rgba(61, 50, 53, 0.4), rgba(74, 63, 66, 0.3));
    border: 1px solid rgba(212, 165, 165, 0.2);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 2px 8px rgba(212, 165, 165, 0.1);

    :deep(.el-form-item__label) {
      color: #e6d5b8;
    }

    :deep(.el-input__inner),
    :deep(.el-input-number__decrease),
    :deep(.el-input-number__increase) {
      background: rgba(230, 213, 184, 0.1);
      border-color: rgba(230, 213, 184, 0.2);
      color: #e6d5b8;

      &:hover {
        border-color: rgba(230, 213, 184, 0.4);
      }

      &:focus {
        border-color: @morandi-red;
      }
    }

    :deep(.el-select__wrapper) {
      background: rgba(230, 213, 184, 0.1);
      border-color: rgba(230, 213, 184, 0.2);

      &:hover {
        border-color: rgba(230, 213, 184, 0.4);
      }

      &.is-focused {
        border-color: @morandi-red;
      }
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @morandi-red;
    }
  }

  .form-tip {
    color: rgba(230, 213, 184, 0.6);
  }

  .add-btn {
    background: rgba(212, 165, 165, 0.2);
    border-color: rgba(212, 165, 165, 0.3);
    color: #e6d5b8;

    &:hover {
      background: rgba(212, 165, 165, 0.3);
      border-color: @morandi-red;
    }
  }

  .form-actions .el-button:not(.el-button--primary) {
    background: rgba(212, 165, 165, 0.2);
    border-color: rgba(212, 165, 165, 0.3);
    color: #e6d5b8;

    &:hover {
      background: rgba(212, 165, 165, 0.3);
      border-color: @morandi-red;
    }
  }

  .vdd-content {
    &::-webkit-scrollbar-track {
      background: rgba(230, 213, 184, 0.05);
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(212, 165, 165, 0.3);

      &:hover {
        background: rgba(212, 165, 165, 0.5);
      }
    }
  }
}

// ========== 浅色模式 ==========
[data-bs-theme='light'] {
  .vdd-header {
    border-bottom: 1px solid rgba(74, 158, 255, 0.2);
    background: linear-gradient(135deg, rgba(74, 158, 255, 0.1), rgba(122, 184, 255, 0.05));

    h2 {
      color: #3a7ed5;
      text-shadow: 0 1px 2px rgba(74, 158, 255, 0.2);

      .header-icon {
        color: @gura-blue;
      }
    }
  }

  .vdd-form {
    background: linear-gradient(135deg, rgba(240, 248, 255, 0.8), rgba(230, 242, 255, 0.6));
    border: 1px solid rgba(74, 158, 255, 0.2);
    box-shadow: 0 8px 32px rgba(74, 158, 255, 0.15), 0 2px 8px rgba(74, 158, 255, 0.1);

    :deep(.el-form-item__label) {
      color: #3a7ed5;
    }

    :deep(.el-input__inner),
    :deep(.el-input-number__decrease),
    :deep(.el-input-number__increase) {
      background: rgba(255, 255, 255, 0.8);
      border-color: rgba(74, 158, 255, 0.3);
      color: #3a7ed5;

      &:hover {
        border-color: rgba(74, 158, 255, 0.5);
      }

      &:focus {
        border-color: @gura-blue;
      }
    }

    :deep(.el-select__wrapper) {
      background: rgba(255, 255, 255, 0.8);
      border-color: rgba(74, 158, 255, 0.3);

      &:hover {
        border-color: rgba(74, 158, 255, 0.5);
      }

      &.is-focused {
        border-color: @gura-blue;
      }
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @gura-blue;
    }
  }

  .form-tip {
    color: rgba(58, 126, 213, 0.6);
  }

  .add-btn {
    background: rgba(74, 158, 255, 0.1);
    border-color: rgba(74, 158, 255, 0.3);
    color: #3a7ed5;

    &:hover {
      background: rgba(74, 158, 255, 0.2);
      border-color: @gura-blue;
    }
  }

  .form-actions .el-button:not(.el-button--primary) {
    background: rgba(74, 158, 255, 0.1);
    border-color: rgba(74, 158, 255, 0.3);
    color: #3a7ed5;

    &:hover {
      background: rgba(74, 158, 255, 0.2);
      border-color: @gura-blue;
    }
  }

  .vdd-content {
    &::-webkit-scrollbar-track {
      background: rgba(74, 158, 255, 0.05);
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(74, 158, 255, 0.3);

      &:hover {
        background: rgba(74, 158, 255, 0.5);
      }
    }
  }
}

// ========== 通用样式 ==========
.vdd-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 24px 32px;
  transition: all 0.3s ease;

  h2 {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0;
    font-size: 24px;
    font-weight: 600;
    transition: all 0.3s ease;

    .header-icon {
      font-size: 28px;
      transition: all 0.3s ease;
    }
  }
}

.vdd-content {
  flex: 1;
  overflow-y: auto;
  padding: 32px;
  padding-bottom: 120px; // 为底部认证标识留出空间
  position: relative;

  &::-webkit-scrollbar {
    width: 8px;
  }
}

// ========== 认证标识装饰 ==========
.cert-badges {
  position: absolute;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: 12px;
  pointer-events: none;
  z-index: 1;
  opacity: 0.6;
  transition: opacity 0.3s ease;

  .cert-badge {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 70px;
    height: 70px;
    border: 2.5px solid #000;
    border-radius: 8px;
    padding: 6px;
    background: linear-gradient(135deg, #fff 0%, #f5f5f5 100%);
    position: relative;
    box-shadow: 0 3px 8px rgba(0, 0, 0, 0.25), inset 0 1px 0 rgba(255, 255, 255, 0.8), inset 0 -1px 0 rgba(0, 0, 0, 0.1);

    // 移除外层边框效果，使用单一专业边框
    &::after {
      content: '';
      position: absolute;
      inset: 2px;
      border: 1px solid rgba(0, 0, 0, 0.08);
      border-radius: 6px;
      pointer-events: none;
    }

    .cert-text {
      font-size: 16px;
      font-weight: 900;
      font-family: 'Arial Black', 'Helvetica', sans-serif;
      line-height: 1;
      letter-spacing: 0.3px;
      color: #000;
      text-shadow: none;
    }

    .cert-sub {
      font-size: 8px;
      font-weight: 700;
      margin-top: 3px;
      color: #666;
      text-transform: uppercase;
      letter-spacing: 0.8px;
    }
  }
}

// 深色模式下的标识调整
[data-bs-theme='dark'] {
  .cert-badges {
    opacity: 0.5;
  }

  .cert-badge {
    // 保持黑白专业配色，稍微调暗以适应深色背景
    background: linear-gradient(135deg, #e8e8e8 0%, #d0d0d0 100%);
    border-color: #1a1a1a;
    box-shadow: 0 3px 8px rgba(0, 0, 0, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.3), inset 0 -1px 0 rgba(0, 0, 0, 0.2);

    .cert-text {
      color: #1a1a1a;
    }

    .cert-sub {
      color: #4a4a4a;
    }

    &::after {
      border-color: rgba(0, 0, 0, 0.15);
    }
  }
}

// 浅色模式下的标识调整
[data-bs-theme='light'] {
  .cert-badges {
    opacity: 0.65;
  }

  .cert-badge {
    // 浅色模式保持更亮的白色
    background: linear-gradient(135deg, #fff 0%, #fafafa 100%);
    border-color: #000;
    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.15), inset 0 1px 0 rgba(255, 255, 255, 1), inset 0 -1px 0 rgba(0, 0, 0, 0.05);
  }
}

.vdd-form {
  max-width: 800px;
  margin: 0 auto;
  padding: 32px;
  border-radius: 16px;
  backdrop-filter: blur(10px);
  transition: all 0.3s ease;
  position: relative;
  z-index: 1; // 确保表单在显示器背景上方

  :deep(.el-form-item__label) {
    font-weight: 600;
    font-size: 14px;
  }

  :deep(.el-select__wrapper) {
    box-shadow: none;
  }
}

// 两列布局
.two-column-layout {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
  margin-bottom: 20px;

  :deep(.el-form-item) {
    margin-bottom: 0;
  }
}

.setting-content {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
}

.resolution-tag,
.rate-tag {
  font-weight: 500;
  transition: all 0.3s ease;
}

.form-tip {
  margin-left: 12px;
  font-size: 12px;
  font-style: italic;
  transition: all 0.3s ease;
}

.form-actions {
  margin-top: 32px;
  text-align: center;

  :deep(.el-form-item__content) {
    justify-content: center;
    gap: 16px;
  }
}

// 主按钮在深浅模式下的样式
[data-bs-theme='dark'] .form-actions .el-button.el-button--primary {
  background: linear-gradient(135deg, @morandi-red, @morandi-yellow);
  border: none;
  color: #2d2628;
  box-shadow: 0 4px 16px rgba(212, 165, 165, 0.4);

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(212, 165, 165, 0.6);
  }
}

[data-bs-theme='light'] .form-actions .el-button.el-button--primary {
  background: linear-gradient(135deg, @gura-blue, @gura-light-blue);
  border: none;
  color: white;
  box-shadow: 0 4px 16px rgba(74, 158, 255, 0.4);

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(74, 158, 255, 0.6);
  }
}

.form-actions .el-button {
  min-width: 140px;
  font-weight: 600;
  border-radius: 12px;
  transition: all 0.3s ease;

  &:active {
    transform: translateY(0);
  }
}

// ========== EDID 文件管理器样式 ==========
.edid-file-manager {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid rgba(0, 0, 0, 0.1);

  .edid-warning {
    margin-bottom: 4px;

    .warning-text {
      font-size: 13px;
      font-weight: 500;
    }
  }

  .edid-status {
    display: flex;
    align-items: center;
    gap: 8px;

    .edid-path {
      font-size: 11px;
      color: #999;
      font-family: monospace;
    }
  }

  .edid-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .edid-info {
    margin-top: 4px;
  }
}

[data-bs-theme='dark'] {
  .edid-file-manager {
    background: rgba(230, 213, 184, 0.05);
    border-color: rgba(230, 213, 184, 0.1);

    .edid-path {
      color: rgba(230, 213, 184, 0.6);
    }
  }
}

[data-bs-theme='light'] {
  .edid-file-manager {
    background: rgba(74, 158, 255, 0.03);
    border-color: rgba(74, 158, 255, 0.15);

    .edid-path {
      color: rgba(58, 126, 213, 0.6);
    }
  }
}
</style>
