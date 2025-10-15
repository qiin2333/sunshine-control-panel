<template>
  <div class="vdd-settings-wrapper">
    <div class="vdd-header">
      <h2>
        <el-icon class="header-icon"><Monitor /></el-icon>
        虚拟显示器设置
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
        <el-form-item label="分辨率预置">
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
              placeholder="例如: 1920x1080"
              style="width: 140px"
            />
            <el-button
              v-else
              size="small"
              @click="showResolutionInput"
              class="add-btn"
            >
              <el-icon><Plus /></el-icon>
              新增分辨率
            </el-button>
          </div>
        </el-form-item>

        <!-- 显卡设置 -->
        <el-form-item label="GPU绑定">
          <div class="setting-content">
            <el-select
              v-model="gpuFriendlyName"
              filterable
              allow-create
              default-first-option
              style="width: 100%; max-width: 400px"
              placeholder="选择或输入GPU名称"
              @blur="saveGpuEdit"
              @keyup.enter="saveGpuEdit"
            >
              <el-option v-for="gpu in gpuOptions" :key="gpu" :label="gpu" :value="gpu" />
            </el-select>
          </div>
        </el-form-item>

        <!-- 显示器数量 -->
        <el-form-item label="显示器数量">
          <el-input-number v-model="settings.monitors[0].count" :min="1" :max="1" disabled />
          <span class="form-tip">当前版本仅支持1个虚拟显示器</span>
        </el-form-item>

        <!-- 刷新率设置 -->
        <el-form-item label="刷新率预置">
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
              placeholder="30-240"
              style="width: 100px"
            />
            <el-button
              v-else
              size="small"
              @click="showRefreshRateInput"
              class="add-btn"
            >
              <el-icon><Plus /></el-icon>
              新增刷新率
            </el-button>
          </div>
        </el-form-item>

        <!-- SDR10 -->
        <el-form-item label="SDR 10bit">
          <el-switch v-model="settings.colour[0].SDR10bit" />
          <span class="form-tip">启用10bit SDR色彩深度</span>
        </el-form-item>

        <!-- HDR+ -->
        <el-form-item label="HDR 12bit">
          <el-switch v-model="settings.colour[0].HDRPlus" />
          <span class="form-tip">启用12bit HDR+色彩深度</span>
        </el-form-item>

        <!-- 色彩模式 -->
        <el-form-item label="色彩模式">
          <el-select v-model="settings.colour[0].ColourFormat" placeholder="请选择色彩模式" style="width: 180px">
            <el-option label="RGB" value="RGB" />
            <el-option label="YCbCr444" value="YCbCr444" />
            <el-option label="YCbCr422" value="YCbCr422" />
            <el-option label="YCbCr420" value="YCbCr420" />
          </el-select>
        </el-form-item>

        <!-- 日志 -->
        <el-form-item label="调试日志">
          <el-switch v-model="settings.logging[0].logging" />
          <span class="form-tip">启用VDD调试日志</span>
        </el-form-item>

        <!-- 保存按钮 -->
        <el-form-item class="form-actions">
          <el-button type="primary" @click="saveSettings" size="large">
            <el-icon><Select /></el-icon>
            保存设置
          </el-button>
        </el-form-item>
      </el-form>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import { Monitor, Plus, Select } from '@element-plus/icons-vue'
import { vdd } from '../tauri-adapter.js'

const resolutionOptions = ref(new Set())
const gpuFriendlyName = ref('')
const refreshRateOptions = ref(new Set([60, 120, 240]))

// 常量定义
const MIN_REFRESH_RATE = 30
const MAX_REFRESH_RATE = 240
const RESOLUTION_PATTERN = /^\d+x\d+$/
const CHINESE_PATTERN = /[\u4e00-\u9fa5]/

const gpuOptions = ref([])

// 初始设置
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

// 输入状态
const showResInput = ref(false)
const showRateInput = ref(false)
const newResolution = ref('')
const newRefreshRate = ref('')
const resInputRef = ref(null)
const rateInputRef = ref(null)

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
      ElMessage.warning('加载默认设置')
      return
    }

    const { data } = result
    Object.assign(settings, {
      ...initialSettings,
      ...data,
    })

    // GPU数据处理
    if (Array.isArray(data.gpu) && data.gpu[0]) {
      const gpuData = data.gpu[0]
      gpuFriendlyName.value = typeof gpuData === 'string' ? gpuData : gpuData.friendlyname?.[0] || ''
      settings.gpu[0] = {
        friendlyname: [gpuFriendlyName.value],
      }
    }

    // 分辨率处理
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

    // 刷新率处理
    if (data.global?.g_refresh_rate) {
      refreshRateOptions.value = new Set(data.global.g_refresh_rate)
    }

    ElMessage.success('设置加载成功')
  } catch (error) {
    console.error('加载设置错误:', error)
    ElMessage.error('加载设置失败')
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
    console.error('获取GPU列表失败:', error)
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
              height: [height],
            }
          }),
        },
      ],
    }

    const payload = JSON.parse(JSON.stringify(settingsToSave))
    const result = await vdd.saveSettings(payload)

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

// 分辨率管理
const validateResolution = (value) => RESOLUTION_PATTERN.test(value)

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
  if (resolutionOptions.value.size <= 1) {
    ElMessage.error('必须至少保留一个分辨率')
    return
  }
  resolutionOptions.value.delete(value)
  ElMessage.info(`已移除分辨率 ${value}`)
}

const handleResInputConfirm = () => {
  if (newResolution.value) {
    addResolution()
  }
  showResInput.value = false
}

// 刷新率管理
const validateRefreshRate = (value) => /^\d+$/.test(value)

const addRefreshRate = () => {
  const value = newRefreshRate.value.trim()
  if (!validateRefreshRate(value)) {
    ElMessage.warning('请输入有效的刷新率（30-240）')
    newRefreshRate.value = ''
    return
  }
  const rate = parseInt(value)
  if (rate < MIN_REFRESH_RATE || rate > MAX_REFRESH_RATE) {
    ElMessage.warning('刷新率范围应在30-240之间')
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
  if (refreshRateOptions.value.size <= 1) {
    ElMessage.error('必须至少保留一个刷新率')
    return
  }
  refreshRateOptions.value.delete(value)
  ElMessage.info(`已移除刷新率 ${value}Hz`)
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
    ElMessage.error('GPU名称不能包含中文')
    gpuFriendlyName.value = ''
    return
  }

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

<style lang="less" scoped>
@import '../styles/theme.less';

.vdd-settings-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

// ========== 深色模式 ==========
[data-bs-theme="dark"] {
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
    box-shadow: 
      0 8px 32px rgba(0, 0, 0, 0.3),
      0 2px 8px rgba(212, 165, 165, 0.1);

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
[data-bs-theme="light"] {
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
    box-shadow: 
      0 8px 32px rgba(74, 158, 255, 0.15),
      0 2px 8px rgba(74, 158, 255, 0.1);

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
    box-shadow: 
      0 3px 8px rgba(0, 0, 0, 0.25),
      inset 0 1px 0 rgba(255, 255, 255, 0.8),
      inset 0 -1px 0 rgba(0, 0, 0, 0.1);
    
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
[data-bs-theme="dark"] {
  .cert-badges {
    opacity: 0.5;
  }

  .cert-badge {
    // 保持黑白专业配色，稍微调暗以适应深色背景
    background: linear-gradient(135deg, #e8e8e8 0%, #d0d0d0 100%);
    border-color: #1a1a1a;
    box-shadow: 
      0 3px 8px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.3),
      inset 0 -1px 0 rgba(0, 0, 0, 0.2);

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
[data-bs-theme="light"] {
  .cert-badges {
    opacity: 0.65;
  }

  .cert-badge {
    // 浅色模式保持更亮的白色
    background: linear-gradient(135deg, #fff 0%, #fafafa 100%);
    border-color: #000;
    box-shadow: 
      0 3px 10px rgba(0, 0, 0, 0.15),
      inset 0 1px 0 rgba(255, 255, 255, 1),
      inset 0 -1px 0 rgba(0, 0, 0, 0.05);
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
[data-bs-theme="dark"] .form-actions .el-button.el-button--primary {
  background: linear-gradient(135deg, @morandi-red, @morandi-yellow);
  border: none;
  color: #2d2628;
  box-shadow: 0 4px 16px rgba(212, 165, 165, 0.4);

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(212, 165, 165, 0.6);
  }
}

[data-bs-theme="light"] .form-actions .el-button.el-button--primary {
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
</style>

