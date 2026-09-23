import { shortcutButtons } from './controllerButtons.js'

// Defer buttons that may form a configured shortcut until their release.
// Only a chord held for the same 500 ms as the native input loop is consumed.
export function createGamepadInputGate() {
  let owner = null, pending = [], consumed = false, releasePending = false, config = ''
  let heldChord = '', heldSince = 0
  const reset = () => {
    owner = null; pending = []; consumed = false; releasePending = false
    heldChord = ''; heldSince = 0
  }
  return {
    reset,
    update(pads, activeIndex, edges, { capturing, bindings }, now = performance.now()) {
      const neutral = pads.every(p => p.buttons.every(b => !b.pressed))
      const nextConfig = JSON.stringify(bindings)
      if (nextConfig !== config) {
        reset()
        config = nextConfig
        releasePending = !neutral
        return { blocked: true, edges: [] }
      }
      if (capturing) { reset(); releasePending = true }
      if (releasePending) {
        if (!capturing && neutral) releasePending = false
        return { blocked: true, edges: [] }
      }
      if (owner !== null && owner !== activeIndex) {
        reset()
        releasePending = !neutral
        return { blocked: true, edges: [] }
      }
      const pad = pads.find(p => p.index === activeIndex)
      // Browser indices are only defined for standard mappings.
      const chords = pad?.mapping === 'standard' ? bindings.map(shortcutButtons).filter(b => b.length >= 2) : []
      const reserved = new Set(chords.flat())
      const pressed = index => !!pad?.buttons[index]?.pressed
      if (owner === null && chords.some(chord => chord.some(pressed))) owner = activeIndex
      if (owner === null) return { blocked: false, edges }
      const immediate = edges.filter(edge => !reserved.has(edge.index))
      const deferred = edges.filter(edge => reserved.has(edge.index))
      const complete = chords.find(chord => chord.every(pressed))?.join(',') || ''
      if (complete !== heldChord) { heldChord = complete; heldSince = now }
      else if (complete && now - heldSince >= 500) consumed = true
      if (pending.length + deferred.length > 32) consumed = true
      if (!consumed) pending.push(...deferred)
      if ([...reserved].some(pressed)) return { blocked: false, edges: immediate }
      const replay = consumed ? [] : pending
      reset()
      return { blocked: false, edges: [...replay, ...immediate] }
    }
  }
}
