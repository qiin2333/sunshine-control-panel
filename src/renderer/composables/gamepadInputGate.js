import { shortcutButtons } from './controllerButtons.js'

// Reserve shortcut candidates before desktop navigation sees their first key.
// A single reserved key still acts on release; a complete chord is consumed.
export function createGamepadInputGate() {
  let owner = null, pending = [], consumed = false, releasePending = false, config = ''
  const reset = () => { owner = null; pending = []; consumed = false; releasePending = false }
  return {
    reset,
    update(pads, activeIndex, edges, { capturing, bindings }) {
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
      const pressed = index => !!pad?.buttons[index]?.pressed
      if (owner === null && chords.some(chord => chord.some(pressed))) owner = activeIndex
      if (owner === null) return { blocked: false, edges }
      if (chords.some(chord => chord.every(pressed))) consumed = true
      if (pending.length + edges.length > 32) consumed = true
      if (!consumed) pending.push(...edges)
      if (!neutral) return { blocked: true, edges: [] }
      const replay = consumed ? [] : pending
      reset()
      return { blocked: false, edges: replay }
    }
  }
}
