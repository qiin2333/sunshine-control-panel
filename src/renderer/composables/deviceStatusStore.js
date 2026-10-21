import { ref, shallowRef } from 'vue'
import { dualsense, usbip, virtualMicrophone, vigem } from '../tauri-adapter.js'

/**
 * 单窗口共享的设备状态通道：同一窗口内的所有面板（总览/控制器/USB/组件）
 * 共用一份 dualsense / usbip 探测结果，避免每次挂载都重新 spawn
 * sidecar --probe 或 usbip 全量探测。
 *
 * - fetch({ force })：force=false 且缓存未过期时直接返回缓存；
 *   并发请求合并为一次后端调用。
 * - invalidate()：操作（安装/卸载/附加设备）前调用，使下一次读取强制重探。
 * - prime(result)：操作返回的新状态直接写入缓存，免去紧随的重复探测。
 *
 * 注意：主窗口与 desktop 窗口是两个 JS 上下文，各自持有独立通道实例。
 */
export function createStatusChannel(fetcher, ttlMs) {
  const data = shallowRef(null)
  const error = ref(null)
  let inflight = null
  let inflightEpoch = 0
  let epoch = 0
  let fetchedAt = 0

  const isFresh = () => data.value !== null && error.value === null && Date.now() - fetchedAt < ttlMs

  async function fetch({ force = false } = {}) {
    if (!force && isFresh()) {
      return { success: true, data: data.value, cached: true }
    }
    // invalidate() 之后不再并入旧的在途请求：操作后的刷新必须反映操作后的状态，
    // 而不是操作开始前就已发出的旧探测结果。
    if (inflight && inflightEpoch === epoch) return inflight
    const requestEpoch = epoch
    inflightEpoch = epoch
    inflight = (async () => {
      try {
        const result = await fetcher()
        if (requestEpoch !== epoch) {
          // 请求期间通道被失效：结果可能早于操作，不写入缓存。
          return result
        }
        if (result?.success) {
          data.value = result.data
          error.value = null
        } else {
          error.value = result?.message || 'probe failed'
        }
        fetchedAt = Date.now()
        return result
      } finally {
        if (inflightEpoch === epoch && inflightEpoch === requestEpoch) inflight = null
      }
    })()
    return inflight
  }

  function invalidate() {
    fetchedAt = 0
    epoch += 1
  }

  /** 操作成功后用返回的完整 { success, data } 信封直接填充缓存。 */
  function prime(result) {
    if (result?.success && result.data) {
      // prime 视为一次确认写入：推进纪元，使操作期间发出的在途探测
      // 完成后不能覆盖已确认的操作结果。
      epoch += 1
      data.value = result.data
      error.value = null
      fetchedAt = Date.now()
    }
  }

  return { fetch, invalidate, prime, data, error }
}

const STATUS_TTL_MS = 10_000
const POLL_INTERVAL_MS = 30_000

export const STATUS_POLL_INTERVAL_MS = POLL_INTERVAL_MS

export const dualsenseStatusChannel = createStatusChannel(() => dualsense.getStatus(), STATUS_TTL_MS)

export const usbipStatusChannel = createStatusChannel(() => usbip.getStatus(), STATUS_TTL_MS)

// 驱动/外设探测会 spawn PowerShell（秒级），缓存收益最大。
export const vigemStatusChannel = createStatusChannel(() => vigem.getStatus(), STATUS_TTL_MS)

// 麦克风是轻量 HTTP 且状态变化较活：TTL 取短，只用于合并并发读取。
export const micStatusChannel = createStatusChannel(() => virtualMicrophone.getStatus(), 2_000)
