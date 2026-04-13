let _invoke = null

async function ensureInvoke() {
  if (!_invoke) {
    const tauri = await import('@tauri-apps/api/core')
    _invoke = tauri.invoke
  }
  return _invoke
}

/**
 * 调用 Tauri 后端命令
 * @param {string} cmd - 命令名称
 * @param {object} params - 参数
 * @returns {Promise<any>}
 */
export async function tauriInvoke(cmd, params = {}) {
  const invoke = await ensureInvoke()
  return invoke(cmd, params)
}
