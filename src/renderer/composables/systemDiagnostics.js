/** Report only what GPU enumeration and the local WebUI request establish. */
export async function probeSystemDiagnostics(invoke, fetchRequest, text) {
  const unknown = () => ({ value: text.notTested, status: 'warning', statusText: text.unverified })
  const rows = { gpu: unknown(), encoder: unknown(), network: unknown(), firewall: unknown() }
  rows.encoder.value = text.encoderNotTested
  rows.firewall.value = text.remoteNotTested
  await Promise.all([
    (async () => {
      try {
        const gpus = await invoke('get_gpus')
        const names = Array.isArray(gpus) ? gpus.filter(name => typeof name === 'string' && name.trim()) : []
        rows.gpu = names.length
          ? { value: names.join(' / '), status: 'good', statusText: text.enumerated }
          : { value: text.noGPU, status: 'warning', statusText: text.unknown }
      } catch {
        rows.gpu = { value: text.detectFailed, status: 'warning', statusText: text.unknown }
      }
    })(),
    (async () => {
      try {
        const url = await invoke('get_proxy_url_command')
        const response = await fetchRequest(`${url}/api/config`, { signal: AbortSignal.timeout(3000) })
        if (!response.ok) throw new Error('HTTP failure')
        const data = await response.json()
        if (data?.status !== true && data?.status !== 'true') throw new Error('Unexpected response')
        rows.network = { value: text.localReachable, status: 'good', statusText: text.reachable }
      } catch {
        rows.network = { value: text.localUnconfirmed, status: 'warning', statusText: text.unverified }
      }
    })(),
  ])
  return rows
}
