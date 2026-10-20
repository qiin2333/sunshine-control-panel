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

export function useVmouse(t) {
  async function loadVmouseStatus() {
    if (loadInflight) return loadInflight
    loadInflight = (async () => {
      try {
        const result = await vmouse.getStatus()
        if (!result?.success || typeof result.data?.config_enabled !== 'boolean') throw new Error('Invalid status')
        vmouseStatus.value = result.data
        vmouseStatusKnown.value = true
        vmouseNotice.value = ''
      } catch {
        vmouseStatusKnown.value = false
        vmouseNotice.value = t.value.stream.vmouseReadFailed
      } finally {
        loadInflight = null
      }
    })()
    return loadInflight
  }

  async function toggleVmouse() {
    if (vmouseConfigSaving.value || !vmouseStatusKnown.value) return
    vmouseConfigSaving.value = true
    try {
      const enabled = !vmouseEnabled.value
      const result = await vmouse.setConfig(enabled)
      if (!result?.success) throw new Error('Save failed')
      vmouseStatus.value = { ...vmouseStatus.value, config_enabled: enabled }
      vmouseNotice.value = t.value.stream.vmouseSaved
    } catch {
      vmouseNotice.value = t.value.stream.vmouseSaveFailed
    } finally {
      vmouseConfigSaving.value = false
    }
  }

  return { vmouseStatus, vmouseEnabled, vmouseStatusKnown, vmouseNotice, vmouseConfigSaving, loadVmouseStatus, toggleVmouse }
}
