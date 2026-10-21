import { ref } from 'vue'
import { controllerHub } from '../tauri-adapter.js'

// 模块级共享：控制器 tab 的仿真模式选择与高级选项共享同一份配置副本——
// 挂载读取合并为一次，任一处保存成功后补丁立即并入共享副本，另一处即刻可见。
const config = ref(null)
const loading = ref(false)
const unreachable = ref(false)
let loadInflight = null
let loadInflightEpoch = 0
let configEpoch = 0

async function load() {
  if (loadInflight && loadInflightEpoch === configEpoch) return loadInflight
  const requestEpoch = configEpoch
  loadInflightEpoch = requestEpoch
  loadInflight = (async () => {
    loading.value = true
    try {
      const result = await controllerHub.getConfig()
      if (requestEpoch !== configEpoch) {
        // 读取期间发生过保存：旧快照不得覆盖已保存的补丁。
        // 丢弃后重新拉取完整配置——重试读取会重新校验纪元，
        // 不会覆盖保存写入的内容；保存补丁之外的字段因此不会停留在默认值。
        await load()
        return
      }
      if (result?.success) {
        config.value = { ...(config.value || {}), ...result.data }
        unreachable.value = false
      } else {
        unreachable.value = true
      }
    } catch {
      if (requestEpoch !== configEpoch) return
      unreachable.value = true
    } finally {
      if (loadInflightEpoch === configEpoch && loadInflightEpoch === requestEpoch) {
        loadInflight = null
      }
      loading.value = false
    }
  })()
  return loadInflight
}

// 乐观更新与失败回滚由调用方负责；保存成功后补丁并入共享副本，
// 并推进纪元使读取期间发出的旧快照失效。
async function save(patch) {
  const result = await controllerHub.saveConfig(patch)
  if (result?.success) {
    configEpoch += 1
    config.value = { ...(config.value || {}), ...patch }
  }
  return result
}

export function useControllerHubConfig() {
  return { config, loading, unreachable, load, save }
}
