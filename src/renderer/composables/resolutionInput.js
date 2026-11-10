export function normalizeResolution(value) {
  const match = typeof value === 'string'
    ? value.match(/^\s*(\d+)\s*[xX*×]\s*(\d+)\s*$/)
    : null
  return match ? `${match[1]}x${match[2]}` : ''
}
