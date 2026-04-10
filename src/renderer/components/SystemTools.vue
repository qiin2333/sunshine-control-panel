<template>
  <div class="system-tools">
    <!-- 这个组件提供工具函数的 UI 交互 -->
  </div>
</template>

<script setup>
import { ElMessage, ElMessageBox } from 'element-plus'
import { tools } from '@/tauri-adapter.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

// 暴露给全局使用的函数
defineExpose({
  async confirmAndUninstallVdd() {
    try {
      await ElMessageBox.confirm(
        t.value.systemTools.vddUninstallConfirm,
        t.value.systemTools.vddUninstallTitle,
        {
          confirmButtonText: t.value.systemTools.confirm,
          cancelButtonText: t.value.systemTools.cancel,
          type: 'warning',
        }
      )
      
      const result = await tools.uninstallVddDriver()
      ElMessage.success(result)
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.error(t.value.systemTools.vddUninstallFailed.replace('{error}', error))
      }
    }
  },

  async confirmAndRestartDriver() {
    try {
      await ElMessageBox.confirm(
        t.value.systemTools.restartGpuConfirm,
        t.value.systemTools.restartGpuTitle,
        {
          confirmButtonText: t.value.systemTools.confirm,
          cancelButtonText: t.value.systemTools.cancel,
          type: 'warning',
        }
      )
      
      const result = await tools.restartGraphicsDriver()
      ElMessage.success(result)
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.error(t.value.systemTools.restartGpuFailed?.replace('{error}', error) || String(error))
      }
    }
  },

  async confirmAndRestartSunshine() {
    try {
      await ElMessageBox.confirm(
        t.value.systemTools.restartSunshineConfirm,
        t.value.systemTools.restartSunshineTitle,
        {
          confirmButtonText: t.value.systemTools.confirm,
          cancelButtonText: t.value.systemTools.cancel,
          type: 'warning',
        }
      )
      
      await tools.restartSunshineService()
      
      // 显示详细的成功提示
      await ElMessageBox.alert(
        t.value.systemTools.restartSunshineMsg,
        t.value.systemTools.restartSunshineSuccess,
        {
          confirmButtonText: t.value.systemTools.confirm,
          type: 'success',
        }
      )
      
      // 3秒后关闭窗口
      setTimeout(() => {
        if (window.__TAURI__) {
          window.__TAURI__.window.getCurrent().close()
        }
      }, 3000)
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.error(t.value.systemTools.restartSunshineFailed?.replace('{error}', error) || String(error))
      }
    }
  }
})
</script>

<style scoped>
.system-tools {
  display: none;
}
</style>

