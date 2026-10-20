import { computed, ref } from 'vue'
import { vmouse } from '../tauri-adapter.js'

// 模块级共享状态：同一窗口内的所有入口（设备中心输入驱动、桌面流设置）
// 看到的是同一份虚拟鼠标状态，一处切换处处更新。
// 注意：主窗口与 desktop 窗口是两个 JS 上下文，各自持有独立实例；
// 跨窗口同步需要后端事件广播，当前未做。
const vmouseStatus = ref({ installed: false, running: false })
const vmouseStatusKnown = ref(false)
const vmouseNotice = ref('')
const vmouseConfigSaving = ref(false)
const vmouseEnabled = computed(() => vmouseStatus.value.config_enabled === true)
let loadInflight = null
let loadInflightEpoch = 0
let statusEpoch = 0
let capturedT = null

async function loadVmouseStatusImpl(t) {
  if (loadInflight && loadInflightEpoch === statusEpoch) return loadInflight
  const requestEpoch = statusEpoch
  loadInflightEpoch = requestEpoch
  loadInflight = (async () => {
    try {
      const result = await vmouse.getStatus()
      if (requestEpoch !== statusEpoch) {
        // 加载期间发生过保存：本次读取已过期，不得覆盖保存后的状态。
        return
      }
      if (!result?.success || typeof result?.data?.config_enabled !== 'boolean') throw new Error('Invalid status')
      vmouseStatus.value = result.data
      vmouseStatusKnown.value = true
      vmouseNotice.value = ''
    } catch {
      if (requestEpoch !== statusEpoch) return
      vmouseStatusKnown.value = false
      vmouseNotice.value = t.value.stream.vmouseReadFailed
    } finally {
      if (loadInflightEpoch === statusEpoch && loadInflightEpoch === requestEpoch) loadInflight = null
    }
  })()
  return loadInflight
}

export function useVmouse(t) {
  capturedT = t
  async function loadVmouseStatus() {
    return loadVmouseStatusImpl(t)
  }

  async function toggleVmouse() {
    if (vmouseConfigSaving.value || !vmouseStatusKnown.value) return
    vmouseConfigSaving.value = true
    try {
      const enabled = !vmouseEnabled.value
      const result = await vmouse.setConfig(enabled)
      if (!result?.success) throw new Error('Save failed')
      vmouseStatus.value = { ...vmouseStatus.value, config_enabled: enabled }
      statusEpoch += 1
      vmouseNotice.value = t.value.stream.vmouseSaved
    } catch {
      vmouseNotice.value = t.value.stream.vmouseSaveFailed
    } finally {
      vmouseConfigSaving.value = false
    }
  }

  return { vmouseStatus, vmouseEnabled, vmouseStatusKnown, vmouseNotice, vmouseConfigSaving, loadVmouseStatus, toggleVmouse }
}

// 驱动安装/卸载完成后调用：作废在途读取并立即重探。
// （后端 bat 落定后返回，无需前端再猜测等待时长。）
export async function reloadVmouseAfterOperation() {
  statusEpoch += 1
  if (!capturedT) return
  await loadVmouseStatusImpl(capturedT)
}
