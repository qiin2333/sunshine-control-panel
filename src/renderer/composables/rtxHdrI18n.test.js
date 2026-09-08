import assert from 'node:assert/strict'
import test from 'node:test'

import { rtxHdrMessages } from './rtxHdrMessages.js'
import { en } from '../desktop/i18n/en.js'
import { zh } from '../desktop/i18n/zh.js'

test('HDR management is vendor-neutral while the backend keeps its identity', () => {
  for (const [locale, messages] of Object.entries({ en, zh })) {
    assert.ok(messages.sidebar.hdrEnhanced.includes('HDR'))
    assert.ok(!messages.sidebar.hdrEnhanced.includes('RTX'))
    assert.ok(messages.hdrEnhanced.description)
    assert.equal(rtxHdrMessages[locale].title, 'NVIDIA RTX HDR')
  }
})

test('RTX HDR component copy explains how to resolve missing files', () => {
  for (const locale of ['en', 'zh']) {
    const text = rtxHdrMessages[locale]
    assert.ok(text.title.includes('RTX HDR'))
    assert.match(text.acquisitionDescription, /nvngx_truehdr\.dll/)
    assert.match(text.bridgeMissingNotice, /重新安装|reinstall/i)
    assert.match(text.runtimeMissingNotice, /nvngx_truehdr\.dll/)
    assert.equal(text.ownershipTitle, undefined)
    assert.equal(text.securityTitle, undefined)
  }
})

test('RTX HDR locale keys stay complete and sorted', () => {
  const englishKeys = Object.keys(rtxHdrMessages.en)
  const chineseKeys = Object.keys(rtxHdrMessages.zh)
  assert.deepEqual(englishKeys, [...englishKeys].sort())
  assert.deepEqual(chineseKeys, [...chineseKeys].sort())
  assert.deepEqual(chineseKeys, englishKeys)
})

test('RTX HDR component copy covers every manager state', () => {
  for (const locale of ['en', 'zh']) {
    assert.deepEqual(
      Object.keys(rtxHdrMessages[locale].states).sort(),
      ['active', 'configured', 'degraded', 'in_use', 'loading', 'not_installed', 'repair_required', 'selected'],
    )
  }
})
