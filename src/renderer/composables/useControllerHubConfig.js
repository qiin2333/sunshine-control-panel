import { ref } from 'vue'
import { controllerHub } from '../tauri-adapter.js'

// 模块级共享：控制器 tab 的仿真模式选择与高级选项共享同一份配置副本——
// 挂载读取合并为一次，任一处保存成功后补丁立即并入共享副本，另一处即刻可见。
const config = ref(null)
const loading = ref(false)
const unreachable = ref(false)
let inflight = null

async function load() {
  if (inflight) return inflight
  loading.value = true
  inflight = (async () => {
    try {
      const result = await controllerHub.getConfig()
      if (result?.success) {
        config.value = { ...(config.value || {}), ...result.data }
        unreachable.value = false
      } else {
        unreachable.value = true
      }
    } catch {
      unreachable.value = true
    } finally {
      loading.value = false
      inflight = null
    }
  })()
  return inflight
}

// 乐观更新与失败回滚由调用方负责；保存成功后补丁并入共享副本。
async function save(patch) {
  const result = await controllerHub.saveConfig(patch)
  if (result?.success) {
    config.value = { ...(config.value || {}), ...patch }
  }
  return result
}

export function useControllerHubConfig() {
  return { config, loading, unreachable, load, save }
}
