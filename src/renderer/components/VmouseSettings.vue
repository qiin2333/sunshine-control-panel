<template>
  <div class="vmouse-settings-wrapper">
    <div class="vmouse-header">
      <h2>
        <el-icon class="header-icon"><Mouse /></el-icon>
        {{ t.vmouseSettings.title }}
      </h2>
    </div>

    <div class="vmouse-content">
      <el-form label-width="130px" class="vmouse-form">
        <!-- 驱动状态 -->
        <el-form-item :label="t.vmouseSettings.driverStatus">
          <div class="status-row">
            <el-tag :type="statusTagType" effect="dark" size="large">
              {{ status.installed ? (status.running ? t.vmouseSettings.running : t.vmouseSettings.installed) : t.vmouseSettings.notInstalled }}
            </el-tag>
            <span class="status-detail" v-if="status.status_text">{{ status.status_text }}</span>
            <el-button size="small" circle :icon="Refresh" @click="refreshStatus" :loading="refreshing" />
          </div>
        </el-form-item>

        <!-- 启用开关（sunshine.conf 中的 virtual_mouse） -->
        <el-form-item :label="t.vmouseSettings.functionSwitch">
          <el-switch
            v-model="configEnabled"
            @change="handleConfigToggle"
            :loading="configSaving"
            :active-text="t.vmouseSettings.switchEnabled"
            :inactive-text="t.vmouseSettings.switchDisabled"
          />
          <span class="form-tip">{{ t.vmouseSettings.switchTip }}</span>
        </el-form-item>

        <!-- 驱动路径 -->
        <el-form-item :label="t.vmouseSettings.driverPath" v-if="status.driver_path">
          <span class="driver-path">{{ status.driver_path }}</span>
        </el-form-item>

        <!-- 说明 -->
        <el-form-item :label="t.vmouseSettings.description">
          <div class="description">
            <p>{{ t.vmouseSettings.descText1 }}</p>
            <p>{{ t.vmouseSettings.descText2 }}</p>
          </div>
        </el-form-item>

        <!-- 安装/卸载按钮 -->
        <el-form-item class="form-actions">
          <el-button
            v-if="!status.installed"
            type="primary"
            @click="installDriver"
            :loading="installing"
            size="large"
          >
            <el-icon><Download /></el-icon>
            {{ t.vmouseSettings.installDriver }}
          </el-button>
          <el-button
            v-else
            type="danger"
            @click="uninstallDriver"
            :loading="uninstalling"
            size="large"
            plain
          >
            <el-icon><Delete /></el-icon>
            {{ t.vmouseSettings.uninstallDriver }}
          </el-button>
        </el-form-item>
      </el-form>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Mouse, Refresh, Download, Delete } from '@element-plus/icons-vue'
import { vmouse } from '../tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

const status = reactive({
  installed: false,
  running: false,
  status_text: t.value.vmouseSettings.detecting,
  driver_path: '',
  config_enabled: true,
})

const configEnabled = ref(true)
const refreshing = ref(false)
const installing = ref(false)
const uninstalling = ref(false)
const configSaving = ref(false)

const statusTagType = ref('info')

const updateStatusTag = () => {
  if (status.running) {
    statusTagType.value = 'success'
  } else if (status.installed) {
    statusTagType.value = 'warning'
  } else {
    statusTagType.value = 'info'
  }
}

const refreshStatus = async () => {
  refreshing.value = true
  try {
    const result = await vmouse.getStatus()
    if (result?.success) {
      Object.assign(status, result.data)
      configEnabled.value = result.data.config_enabled
      updateStatusTag()
    }
  } catch (error) {
    console.error('Failed to get vmouse status:', error)
  } finally {
    refreshing.value = false
  }
}

const handleConfigToggle = async (enabled) => {
  configSaving.value = true
  try {
    const result = await vmouse.setConfig(enabled)
    if (result?.success) {
      ElMessage.success(result.data)
    } else {
      throw new Error(result?.message || t.value.vmouseSettings.unknownError)
    }
  } catch (error) {
    ElMessage.error(t.value.vmouseSettings.setFailed.replace('{error}', error.message || error))
    // 回滚开关
    configEnabled.value = !enabled
  } finally {
    configSaving.value = false
  }
}

const installDriver = async () => {
  try {
    await ElMessageBox.confirm(
      t.value.vmouseSettings.installConfirm,
      t.value.vmouseSettings.installTitle,
      {
        confirmButtonText: t.value.vmouseSettings.installBtn,
        cancelButtonText: t.value.vmouseSettings.cancelBtn,
        type: 'info',
      }
    )

    installing.value = true
    const result = await vmouse.install()
    if (result?.success) {
      ElMessage.success(result.data)
      // 等一下再刷新状态
      setTimeout(() => refreshStatus(), 2000)
    } else {
      throw new Error(result?.message || t.value.vmouseSettings.installFailed)
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(t.value.vmouseSettings.installError.replace('{error}', error.message || error))
    }
  } finally {
    installing.value = false
  }
}

const uninstallDriver = async () => {
  try {
    await ElMessageBox.confirm(
      t.value.vmouseSettings.uninstallConfirm,
      t.value.vmouseSettings.uninstallTitle,
      {
        confirmButtonText: t.value.vmouseSettings.uninstallBtn,
        cancelButtonText: t.value.vmouseSettings.cancelBtn,
        type: 'warning',
      }
    )

    uninstalling.value = true
    const result = await vmouse.uninstall()
    if (result?.success) {
      ElMessage.success(result.data)
      setTimeout(() => refreshStatus(), 2000)
    } else {
      throw new Error(result?.message || t.value.vmouseSettings.uninstallFailed)
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error(t.value.vmouseSettings.uninstallError.replace('{error}', error.message || error))
    }
  } finally {
    uninstalling.value = false
  }
}

onMounted(() => {
  refreshStatus()
})
</script>

<style lang="less" scoped>
@import '../styles/theme.less';

.vmouse-settings-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

// ========== 深色模式 ==========
[data-bs-theme='dark'] {
  .vmouse-header {
    border-bottom: 1px solid rgba(230, 213, 184, 0.15);
    background: linear-gradient(135deg, rgba(165, 189, 212, 0.1), rgba(230, 213, 184, 0.05));

    h2 {
      color: #e6d5b8;
      text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);

      .header-icon {
        color: @morandi-red;
      }
    }
  }

  .vmouse-form {
    background: linear-gradient(135deg, rgba(61, 50, 53, 0.4), rgba(74, 63, 66, 0.3));
    border: 1px solid rgba(212, 165, 165, 0.2);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 2px 8px rgba(212, 165, 165, 0.1);

    :deep(.el-form-item__label) {
      color: #e6d5b8;
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @morandi-red;
    }
  }

  .form-tip {
    color: rgba(230, 213, 184, 0.6);
  }

  .status-detail {
    color: rgba(230, 213, 184, 0.7);
  }

  .driver-path {
    color: rgba(230, 213, 184, 0.5);
  }

  .description {
    color: rgba(230, 213, 184, 0.6);
  }
}

// ========== 浅色模式 ==========
[data-bs-theme='light'] {
  .vmouse-header {
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

  .vmouse-form {
    background: linear-gradient(135deg, rgba(240, 248, 255, 0.8), rgba(230, 242, 255, 0.6));
    border: 1px solid rgba(74, 158, 255, 0.2);
    box-shadow: 0 8px 32px rgba(74, 158, 255, 0.15), 0 2px 8px rgba(74, 158, 255, 0.1);

    :deep(.el-form-item__label) {
      color: #3a7ed5;
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @gura-blue;
    }
  }

  .form-tip {
    color: rgba(58, 126, 213, 0.6);
  }

  .status-detail {
    color: rgba(58, 126, 213, 0.5);
  }

  .driver-path {
    color: rgba(58, 126, 213, 0.4);
  }

  .description {
    color: rgba(58, 126, 213, 0.6);
  }
}

// ========== 通用样式 ==========
.vmouse-header {
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

.vmouse-content {
  flex: 1;
  overflow-y: auto;
  padding: 32px;

  &::-webkit-scrollbar {
    width: 8px;
  }
}

.vmouse-form {
  max-width: 700px;
  padding: 25px;
  border-radius: 12px;
  transition: all 0.3s ease;
}

.status-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-detail {
  font-size: 13px;
}

.driver-path {
  font-size: 12px;
  font-family: monospace;
  word-break: break-all;
}

.form-tip {
  font-size: 12px;
  margin-left: 12px;
}

.description {
  font-size: 13px;
  line-height: 1.6;

  p {
    margin: 0 0 8px 0;
    &:last-child {
      margin-bottom: 0;
    }
  }
}

.form-actions {
  margin-top: 24px;
}
</style>
