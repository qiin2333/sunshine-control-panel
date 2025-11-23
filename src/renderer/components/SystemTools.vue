<template>
  <div class="system-tools">
    <!-- 这个组件提供工具函数的 UI 交互 -->
  </div>
</template>

<script setup>
import { ElMessage, ElMessageBox } from 'element-plus'
import { tools } from '@/tauri-adapter.js'

// 暴露给全局使用的函数
defineExpose({
  async confirmAndUninstallVdd() {
    try {
      await ElMessageBox.confirm(
        '确定要卸载虚拟显示器驱动吗？此操作需要管理员权限。',
        '确认卸载',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
      )
      
      const result = await tools.uninstallVddDriver()
      ElMessage.success(result)
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.error('卸载失败: ' + error)
      }
    }
  },

  async confirmAndRestartDriver() {
    try {
      await ElMessageBox.confirm(
        '确定要重启显卡驱动吗？这将暂时中断屏幕显示。',
        '确认重启',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
      )
      
      const result = await tools.restartGraphicsDriver()
      ElMessage.success(result)
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.error('重启失败: ' + error)
      }
    }
  },

  async confirmAndRestartSunshine() {
    try {
      await ElMessageBox.confirm(
        '确定要重启 Sunshine 服务吗？这将断开当前所有连接。',
        '确认重启',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        }
      )
      
      await tools.restartSunshineService()
      
      // 显示详细的成功提示
      await ElMessageBox.alert(
        '重启命令已发送！\n\n如果弹出 UAC 提示，请点击"是"以确认。\nSunshine 服务将在几秒钟内重启。',
        '重启成功',
        {
          confirmButtonText: '确定',
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
        ElMessage.error('重启失败: ' + error)
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

