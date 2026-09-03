import { reactive, ref } from 'vue'
import { dualsense, usbip, virtualMicrophone } from '../tauri-adapter.js'

/**
 * 设备中心「运行时状态」共享源：概览与组件与诊断页签消费同一份数据，
 * 避免各自 tri-fetch 同三条 IPC。挂载刷新带新鲜度窗口，手动刷新传 force。
 */
const FRESH_MS = 15000

const runtime = reactive({
  ds: { verified: false, usbip_available: false, usbip_version: '', component_version: '', runtime_version: '' },
  mic: { component_available: false },
  usb: { ready: false, version: '' },
})
const loading = ref(false)
const loaded = ref(false)
const loadError = ref(false)
let lastFetchedAt = 0
let inflight = null

async function fetchOnce() {
  const [dsResult, micResult, usbResult] = await Promise.all([
    dualsense.getStatus(),
    virtualMicrophone.getStatus(),
    usbip.getStatus(),
  ])
  if (dsResult?.success) Object.assign(runtime.ds, dsResult.data)
  if (micResult?.success) Object.assign(runtime.mic, micResult.data)
  if (usbResult?.success) Object.assign(runtime.usb, usbResult.data)
  loadError.value = !dsResult?.success && !micResult?.success && !usbResult?.success
  loaded.value = true
  lastFetchedAt = Date.now()
}

export function useDeviceRuntime() {
  function refresh(force = false) {
    if (inflight) return inflight
    if (!force && loaded.value && Date.now() - lastFetchedAt < FRESH_MS) {
      return Promise.resolve()
    }
    loading.value = true
    inflight = fetchOnce().finally(() => {
      loading.value = false
      inflight = null
    })
    return inflight
  }
  return { runtime, loading, loaded, loadError, refresh }
}
