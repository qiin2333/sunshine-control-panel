import test from 'node:test'
import assert from 'node:assert/strict'

import { normalizeResolution } from './resolutionInput.js'
import { useEditableOptionField } from './useEditableOptionField.js'

test('resolution input normalizes common separators without accepting malformed dimensions', () => {
  for (const separator of ['x', 'X', '*', '×']) {
    assert.equal(normalizeResolution(`1920${separator}1080`), '1920x1080')
    assert.equal(normalizeResolution(` 1920 ${separator} 1080 `), '1920x1080')
  }
  for (const value of ['', null, '1920', '1920x', 'x1080', '1920**1080',
    '1920x1080x60', '1920×1080junk', '-1920x1080', '1920.5x1080', '19 20x1080']) {
    assert.equal(normalizeResolution(value), '', String(value))
  }
})

test('editable resolution field normalizes before validation, duplicate detection and persistence', () => {
  const field = useEditableOptionField({
    initialValues: ['1920x1080'],
    normalize: normalizeResolution,
    validate: (value) => /^\d+x\d+$/.test(value),
  })
  for (const value of ['1920*1080', '1920×1080', ' 1920 X 1080 ']) {
    assert.equal(field.addValue(value), false)
  }
  field.draft.value = ' 2560 × 1440 '
  assert.equal(field.addDraft(), true)
  assert.deepEqual([...field.options.value], ['1920x1080', '2560x1440'])
  assert.equal(field.draft.value, '')
  assert.equal(field.addValue('2560**1440'), false)
  assert.deepEqual([...field.options.value], ['1920x1080', '2560x1440'])
})

test('editable fields without a normalizer keep their existing refresh-rate behavior', () => {
  const field = useEditableOptionField({ validate: (value) => /^\d+(\.\d+)?$/.test(value) })
  assert.equal(field.addValue(' 59.94 '), true)
  assert.equal(field.addValue('59.94'), false)
  assert.equal(field.addValue('60×120'), false)
  assert.deepEqual([...field.options.value], ['59.94'])
})
