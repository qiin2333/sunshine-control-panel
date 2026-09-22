// Serialize backend capture leases so a late start cannot undo cancellation.
export function shortcutCapture(invoke, onChange) {
  let generation = 0
  let queue = Promise.resolve()
  let timer
  const setActive = (active) => {
    const operation = queue.then(() => invoke(active))
    queue = operation.catch(() => {})
    return operation
  }
  const cancel = () => {
    ++generation
    clearTimeout(timer)
    onChange('')
    return setActive(false)
  }
  const start = async (key) => {
    const cancelled = cancel()
    const token = generation
    await cancelled
    if (token !== generation) return
    const lease = await setActive(true)
    if (token !== generation) return
    onChange(key, lease)
    timer = setTimeout(() => { void cancel().catch(() => {}) }, 30000)
  }
  return { start, cancel }
}
