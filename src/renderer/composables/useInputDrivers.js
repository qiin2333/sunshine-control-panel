import { reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { vigem, vmouse } from '../tauri-adapter.js'

/**
 * 设备中心输入驱动管理页的真实状态与操作。
 * 单个工具失败不清空整面板（每个状态独立 try/catch），
 * 但探测失败会置 probeFailed 标志：UI 显示「状态不可用」，
 * 与「未安装」区分开，避免误导用户重装。确认弹窗留在组件层。
 */
export function useInputDrivers() {
  const vigemStatus = reactive({
    installed: false, running: false, version: '', version_ok: false,
    status_text: '', driver_path: '',
  })
  const vmouseStatus = reactive({
    installed: false, running: false, status_text: '', driver_path: '',
    config_enabled: false,
  })
  const probeFailed = reactive({ vigem: false, vmouse: false })

  const ops = reactive({
    vigem: false, vmouse: false,
  })
  const initialized = ref(false)
  const refreshing = ref(false)

  async function refreshAll() {
    if (refreshing.value) return
    refreshing.value = true
    const jobs = [
      async () => {
        const result = await vigem.getStatus()
        probeFailed.vigem = !result?.success
        if (result?.success) Object.assign(vigemStatus, result.data)
      },
      async () => {
        const result = await vmouse.getStatus()
        probeFailed.vmouse = !result?.success
        if (result?.success) Object.assign(vmouseStatus, result.data)
      },
    ]
    try {
      await Promise.allSettled(jobs.map(async (job) => {
        try {
          await job()
        } catch (error) {
          console.warn('输入驱动状态刷新失败:', error)
        }
      }))
    } finally {
      initialized.value = true
      refreshing.value = false
    }
  }

  function withOp(flag, action) {
    return async () => {
      if (ops[flag]) return
      ops[flag] = true
      try {
        const result = await action()
        if (result?.success) {
          ElMessage.success(result.data)
          setTimeout(() => refreshAll(), 2000)
        } else {
          ElMessage.error(result?.message || String(result))
        }
        return result?.success === true
      } catch (error) {
        ElMessage.error(String(error))
        return false
      } finally {
        ops[flag] = false
      }
    }
  }

  const installVigem = withOp('vigem', () => vigem.install())
  const uninstallVigem = withOp('vigem', () => vigem.uninstall())
  const installVmouse = withOp('vmouse', () => vmouse.install())
  const uninstallVmouse = withOp('vmouse', () => vmouse.uninstall())

  return {
    vigemStatus, vmouseStatus, probeFailed, ops, initialized, refreshing,
    refreshAll, installVigem, uninstallVigem,
    installVmouse, uninstallVmouse,
  }
}
