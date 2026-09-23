import assert from 'node:assert/strict'
import test from 'node:test'
import {
  shortcutFromEvent,
  displayShortcut,
  nrShortcutLabel,
  DEFAULT_NR_SHORTCUT
} from './enhancementControls.js'
const event = (patch = {}) => ({
  key: 'n',
  code: 'KeyN',
  ctrlKey: true,
  altKey: true,
  shiftKey: false,
  metaKey: false,
  ...patch
})
test('recording uses physical codes and requires a supported modifier', () => {
  assert.equal(
    shortcutFromEvent(event({ key: 'т' })).value,
    DEFAULT_NR_SHORTCUT
  )
  assert.equal(
    shortcutFromEvent(event({ ctrlKey: false, altKey: false })).error,
    'nr_shortcut_invalid'
  )
  assert.equal(
    shortcutFromEvent(event({ metaKey: true })).error,
    'nr_shortcut_invalid'
  )
  assert.equal(
    shortcutFromEvent(event({ code: 'Unidentified' })).error,
    'nr_shortcut_invalid'
  )
  assert.equal(
    shortcutFromEvent(event({ key: 'Control', code: 'ControlLeft' })).modifier,
    true
  )
  assert.equal(shortcutFromEvent(event({ key: 'Escape' })).cancel, true)
  assert.equal(shortcutFromEvent(event({ key: 'Tab' })).cancel, true)
})
test('native canonical shortcut strings render without code prefixes', () => {
  assert.equal(displayShortcut('control+alt+KeyN'), 'Ctrl + Alt + N')
  assert.equal(displayShortcut('shift+control+Digit3'), 'Shift + Ctrl + 3')
  assert.equal(displayShortcut(''), '')
})

test('overlay hints use the gamepad binding when the keyboard binding is unavailable', () => {
  const status = { settings: { nrShortcut: 'Ctrl+Alt+KeyN', nrGamepad: 'LB+RB+Y' }, nrRegistered: false, gamepadSupported: true }
  assert.equal(nrShortcutLabel(status), 'LB + RB + Y')
  assert.equal(nrShortcutLabel({ ...status, nrRegistered: true }), 'Ctrl + Alt + N')
  assert.equal(nrShortcutLabel({ ...status, gamepadSupported: false }), '')
})
