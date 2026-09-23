export const BUTTON = {
  A: 0,
  B: 1,
  X: 2,
  Y: 3,
  LB: 4,
  RB: 5,
  LT: 6,
  RT: 7,
  BACK: 8,
  START: 9,
  L3: 10,
  R3: 11,
  DPAD_UP: 12,
  DPAD_DOWN: 13,
  DPAD_LEFT: 14,
  DPAD_RIGHT: 15,
}

export const FIXED_GAMEPAD_SHORTCUTS = ['LB+RB+X', 'LB+RB+Y']
export const SHORTCUT_PARTS = new Set([BUTTON.LB, BUTTON.RB, BUTTON.X, BUTTON.Y])

export function createFixedShortcutTracker() {
  let candidate = '', since = 0, consumed = false, config = ''
  const reset = () => { candidate = ''; since = 0; consumed = false }
  return {
    reset,
    discard() {
      if (FIXED_GAMEPAD_SHORTCUTS.some(shortcut => config.includes(shortcut))) {
        candidate = ''; consumed = true
      }
    },
    update(pad, bindings, now = performance.now()) {
      const pressed = index => !!pad?.buttons[index]?.pressed
      const partsDown = [...SHORTCUT_PARTS].some(pressed)
      const nextConfig = JSON.stringify(bindings)
      if (config !== nextConfig) {
        const relevant = FIXED_GAMEPAD_SHORTCUTS.some(shortcut =>
          config.includes(shortcut) || bindings.includes(shortcut))
        config = nextConfig
        if (relevant && partsDown) { candidate = ''; consumed = true }
      }
      if (!partsDown) {
        const wasConsumed = consumed
        reset()
        return wasConsumed
      }
      if (consumed) return true
      const exact = face => pad?.mapping === 'standard' && pad.buttons.every((button, index) =>
        !!button.pressed === (index === BUTTON.LB || index === BUTTON.RB || index === face))
      const full = bindings.includes(FIXED_GAMEPAD_SHORTCUTS[0]) && exact(BUTTON.X) ? 'X'
        : bindings.includes(FIXED_GAMEPAD_SHORTCUTS[1]) && exact(BUTTON.Y) ? 'Y' : ''
      if (full !== candidate) { candidate = full; since = now }
      else if (full && now - since >= 500) consumed = true
      return consumed
    }
  }
}

const FACE_CHIPS = {
  xbox: { A: ['A', 'a'], B: ['B', 'b'], X: ['X', 'x'], Y: ['Y', 'y'] },
  ps: { A: ['✕', 'x'], B: ['○', 'b'], X: ['□', 'y'], Y: ['△', 'a'] }
}
export function controllerButtonChip(button, layout = 'xbox') {
  const face = (FACE_CHIPS[layout] || FACE_CHIPS.xbox)[button]
  if (face) return { glyph: face[0], tone: face[1] }
  const aliases = layout === 'ps' ? { LB: 'L1', RB: 'R1', LT: 'L2', RT: 'R2', Back: 'Share', Start: 'Options' } : {}
  return { glyph: aliases[button] || button, tone: 'neutral' }
}
export function displayGamepadShortcut(value, layout = 'xbox') {
  return (value || '').split('+').filter(Boolean).map(button => controllerButtonChip(button, layout).glyph).join(' + ')
}
