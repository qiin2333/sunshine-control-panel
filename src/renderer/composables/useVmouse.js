import { ref } from 'vue'
import { vmouse } from '../tauri-adapter.js'

export function useVmouse(t) {
  const vmouseStatus = ref({ installed: false, running: false })
  const vmouseEnabled = ref(false)
  const vmouseStatusKnown = ref(false)
  const vmouseNotice = ref('')
  const vmouseConfigSaving = ref(false)

  async function loadVmouseStatus() {
    try {
      const result = await vmouse.getStatus()
      if (!result?.success || typeof result.data?.config_enabled !== 'boolean') throw new Error('Invalid status')
      vmouseStatus.value = result.data
      vmouseEnabled.value = result.data.config_enabled
      vmouseStatusKnown.value = true
      vmouseNotice.value = ''
    } catch {
      vmouseStatusKnown.value = false
      vmouseNotice.value = t.value.stream.vmouseReadFailed
    }
  }

  async function toggleVmouse() {
    if (vmouseConfigSaving.value || !vmouseStatusKnown.value) return
    vmouseConfigSaving.value = true
    try {
      const enabled = !vmouseEnabled.value
      const result = await vmouse.setConfig(enabled)
      if (!result?.success) throw new Error('Save failed')
      vmouseEnabled.value = enabled
      vmouseNotice.value = t.value.stream.vmouseSaved
    } catch {
      vmouseNotice.value = t.value.stream.vmouseSaveFailed
    } finally {
      vmouseConfigSaving.value = false
    }
  }

  return { vmouseStatus, vmouseEnabled, vmouseStatusKnown, vmouseNotice, vmouseConfigSaving, loadVmouseStatus, toggleVmouse }
}
