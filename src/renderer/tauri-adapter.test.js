import assert from 'node:assert/strict'
import test from 'node:test'

import { isAllowedExternalUrl } from './tauri-adapter.js'

test('external URL validation rejects local paths and unapproved protocols', () => {
  assert.equal(isAllowedExternalUrl('https://example.com'), true)
  assert.equal(isAllowedExternalUrl('HTTP://localhost:47990'), true)
  assert.equal(isAllowedExternalUrl('ms-windows-store://pdp/?ProductId=example'), true)
  assert.equal(isAllowedExternalUrl('C:\\Windows\\System32'), false)
  assert.equal(isAllowedExternalUrl('file:///C:/Windows/System32'), false)
  assert.equal(isAllowedExternalUrl('shell:AppsFolder/example'), false)
})
