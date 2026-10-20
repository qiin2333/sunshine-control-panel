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
  let refreshGeneration = 0

  async function fetchOnce(force, generation) {
    const results = await Promise.allSettled([
      dualsenseStatusChannel.fetch({ force }),
      virtualMicrophone.getStatus(),
      usbipStatusChannel.fetch({ force }),
    ])
    if (generation !== refreshGeneration) {
      // 已被强制刷新取代：不得覆盖较新请求的结果或状态。
      return
    }
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
    // 普通刷新并入在途请求；强制刷新取代它——旧请求完成后不得
    // 覆盖结果，也不得重置新请求的 loading/inflight。
    if (inflight && !force) return inflight
    const generation = ++refreshGeneration
    loading.value = true
    const request = fetchOnce(force, generation).finally(() => {
      if (generation !== refreshGeneration) return
      loading.value = false
      if (inflight === request) inflight = null
    })
    inflight = request
    return request
  }
  return { runtime, loading, loaded, loadError, probeFailed, refresh }
}
