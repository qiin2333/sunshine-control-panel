import { reactive, ref } from 'vue'
import { virtualMicrophone } from '../tauri-adapter.js'
import { dualsenseStatusChannel, usbipStatusChannel } from './deviceStatusStore.js'

// Overview status is scoped to its mounted tab; coalesce concurrent refreshes.
export function useDeviceRuntime() {
  const runtime = reactive({
    ds: { verified: false, usbip_available: false, usbip_version: '', component_version: '', runtime_version: '' },
    mic: { component_available: false },
    usb: { ready: false, version: '' },
  })
  const loading = ref(false)
  const loaded = ref(false)
  const loadError = ref(false)
  const probeFailed = reactive({ ds: false, mic: false, usb: false })
  let inflight = null

  async function fetchOnce(force = false) {
    const results = await Promise.allSettled([
      dualsenseStatusChannel.fetch({ force }),
      virtualMicrophone.getStatus(),
      usbipStatusChannel.fetch({ force }),
    ])
    for (const [index, key] of ['ds', 'mic', 'usb'].entries()) {
      const result = results[index].status === 'fulfilled' ? results[index].value : null
      probeFailed[key] = !result?.success
      if (result?.success) runtime[key] = result.data
    }
    loadError.value = Object.values(probeFailed).some(Boolean)
    loaded.value = true
  }

  function refresh(options = {}) {
    const force = options?.force === true
    if (inflight) return inflight
    loading.value = true
    inflight = fetchOnce(force).finally(() => {
      loading.value = false
      inflight = null
    })
    return inflight
  }
  return { runtime, loading, loaded, loadError, probeFailed, refresh }
}
