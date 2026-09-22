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

// Persisted shortcut names map to the browser's standard button positions.
export const SHORTCUT_BUTTONS = Object.fromEntries(Object.entries(BUTTON).map(([name, index]) => [
  ({ BACK: 'Back', START: 'Start', DPAD_UP: 'Up', DPAD_DOWN: 'Down', DPAD_LEFT: 'Left', DPAD_RIGHT: 'Right' })[name] || name, index
]))
export function shortcutButtons(value) {
  return (value || '').split('+').filter(Boolean).map(name => SHORTCUT_BUTTONS[name]).filter(Number.isInteger)
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
