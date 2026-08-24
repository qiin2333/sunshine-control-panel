import test from 'node:test'
import assert from 'node:assert/strict'

import { formatDuration } from './useGameSession.js'

test('durations under an hour render as minutes and seconds', () => {
  assert.equal(formatDuration(0), '0:00')
  assert.equal(formatDuration(9), '0:09')
  assert.equal(formatDuration(65), '1:05')
  assert.equal(formatDuration(3599), '59:59')
})

test('durations of an hour or more drop seconds in favour of hours', () => {
  assert.equal(formatDuration(3600), '1:00')
  assert.equal(formatDuration(3660), '1:01')
  assert.equal(formatDuration(45296), '12:34')
})

test('nonsense inputs clamp to zero instead of rendering NaN', () => {
  assert.equal(formatDuration(-5), '0:00')
  assert.equal(formatDuration(undefined), '0:00')
  assert.equal(formatDuration(null), '0:00')
  assert.equal(formatDuration('abc'), '0:00')
})

test('fractional seconds truncate rather than rounding up past the minute', () => {
  assert.equal(formatDuration(59.9), '0:59')
})
