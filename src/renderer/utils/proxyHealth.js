export async function checkLocalProxyHealth(
  check,
  { fetchImpl = globalThis.fetch, timeoutMs = 5000 } = {},
) {
  if (!check?.url || !check?.token || typeof fetchImpl !== 'function') return false

  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)

  try {
    const response = await fetchImpl(check.url, {
      cache: 'no-store',
      signal: controller.signal,
    })
    return response.ok && await response.text() === check.token
  } catch {
    return false
  } finally {
    clearTimeout(timer)
  }
}
