import { reactive, ref } from 'vue'
import { dualsenseStatusChannel, micStatusChannel, usbipStatusChannel } from './deviceStatusStore.js'

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
      micStatusChannel.fetch({ force }),
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
    // 普通刷新并入在途请求，但不复用其结果：在途请求可能启动于某次
    // 通道失效/prime 之前（设备操作已在后台落定），它的结果已不代表
    // 当前状态。等它落定后追加一次缓存感知读取——状态未变时命中缓存
    // 零探测，变更过则触发重探。强制刷新则直接取代在途请求。
    if (inflight && !force) {
      const joined = inflight
      const generation = ++refreshGeneration
      loading.value = true
      const request = joined
        .catch(() => {})
        .then(() => {
          if (generation !== refreshGeneration) return undefined
          return fetchOnce(false, generation)
        })
        .finally(() => {
          if (generation !== refreshGeneration) return
          loading.value = false
          if (inflight === request) inflight = null
        })
      inflight = request
      return request
    }
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
