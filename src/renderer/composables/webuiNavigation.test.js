import assert from 'node:assert/strict'
import test from 'node:test'
import { getWebuiPage, isWebuiNavigationPath } from './webuiNavigation.js'

test('sidebar tracks WebUI aliases, nested pages, and query/hash navigation', () => {
  assert.equal(getWebuiPage('/index.html')?.id, 'overview')
  assert.equal(getWebuiPage('/apps.html?sort=name')?.id, 'apps')
  assert.equal(getWebuiPage('/config#input')?.id, 'stream-settings')
  assert.equal(getWebuiPage('/apps/edit')?.id, 'apps')
  assert.equal(getWebuiPage('/pin')?.id, 'pairing')
  assert.equal(getWebuiPage('/apps-malicious'), null)
  assert.equal(getWebuiPage('/password'), null)
})

test('shell can request only known local destinations', () => {
  for (const path of ['/', '/apps', '/pin', '/config', '/password', '/password#logout', '/troubleshooting'])
    assert.equal(isWebuiNavigationPath(path), true)
  for (const path of [
    'https://evil.example',
    '//evil.example',
    '/config/../../logout',
    '/unknown',
    '/password#logout-now',
    '/password?redirect=https://evil.example',
    null,
  ])
    assert.equal(isWebuiNavigationPath(path), false)
})
