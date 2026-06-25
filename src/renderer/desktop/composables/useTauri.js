let _invoke = null

async function ensureInvoke() {
  if (!_invoke) {
    const tauri = await import('@tauri-apps/api/core')
    if (!tauri.isTauri()) {
      throw new Error('Tauri runtime is not available')
    }
    _invoke = tauri.invoke
  }
  return _invoke
}

// Invoke a Tauri backend command when running inside the desktop shell.
export async function tauriInvoke(cmd, params = {}) {
  const invoke = await ensureInvoke()
  return invoke(cmd, params)
}

export async function isTauriRuntime() {
  try {
    const { isTauri } = await import('@tauri-apps/api/core')
    return isTauri()
  } catch {
    return false
  }
}
