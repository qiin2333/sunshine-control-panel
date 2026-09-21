import assert from 'node:assert/strict'
import test from 'node:test'
import {
  shortcutFromEvent,
  displayShortcut,
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
