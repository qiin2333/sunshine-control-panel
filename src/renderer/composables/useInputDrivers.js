import { reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { vigem, vmouse } from '../tauri-adapter.js'
import { vigemStatusChannel } from './deviceStatusStore.js'
import { useVmouse, reloadVmouseAfterOperation } from './useVmouse.js'


export function useInputDrivers(t) {
  const { ...mouse } = useVmouse(t)
  const vigemStatus = reactive({ installed: false, version: '', status_text: '' })
  const probeFailed = reactive({ vigem: false, vmouse: false })
  const ops = reactive({ vigem: false, vmouse: false })
  const initialized = ref(false)
  const refreshing = ref(false)

  async function refreshAll() {
    if (refreshing.value) return
    refreshing.value = true
    try {
      await Promise.all([
        vigemStatusChannel.fetch().then(result => {
          probeFailed.vigem = !result?.success
          if (result?.success) Object.assign(vigemStatus, result.data)
        }).catch(() => { probeFailed.vigem = true }),
        mouse.loadVmouseStatus(),
      ])
      probeFailed.vmouse = !mouse.vmouseStatusKnown.value
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
          if (flag === 'vigem') vigemStatusChannel.invalidate()
          await reloadVmouseAfterOperation()
          await refreshAll()
        } else {
          ElMessage.error(result?.message || String(result))
        }
      } catch (error) {
        ElMessage.error(String(error))
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
    vigemStatus, ...mouse, probeFailed, ops, initialized, refreshing,
    refreshAll, installVigem, uninstallVigem,
    installVmouse, uninstallVmouse,
  }
}
