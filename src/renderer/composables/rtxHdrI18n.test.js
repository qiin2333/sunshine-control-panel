import assert from 'node:assert/strict'
import test from 'node:test'

import { rtxHdrMessages } from './rtxHdrMessages.js'

test('RTX HDR component copy preserves the local-only licensing boundary', () => {
  for (const locale of ['en', 'zh']) {
    const text = rtxHdrMessages[locale]
    assert.ok(text.title.includes('RTX HDR'))
    assert.match(text.boundary, /不会下载|never downloads/)
    assert.match(text.securityHint, /不会再分发|not redistributed/)
    assert.match(text.securityHint, /不会在 GUI 进程内执行|does not execute selected DLLs/)
  }
})

test('RTX HDR component copy covers every manager state', () => {
  for (const locale of ['en', 'zh']) {
    assert.deepEqual(
      Object.keys(rtxHdrMessages[locale].states).sort(),
      ['in_use', 'loading', 'not_installed', 'ready', 'repair_required'],
    )
  }
})
