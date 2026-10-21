import test from 'node:test'
import assert from 'node:assert/strict'

// src/i18n/index.js 依赖浏览器全局（localStorage/navigator），先打桩再加载。
globalThis.localStorage = {
  store: {},
  getItem(key) { return this.store[key] ?? null },
  setItem(key, value) { this.store[key] = String(value) },
}
Object.defineProperty(globalThis, 'navigator', { value: { language: 'en-US' }, configurable: true })

const { t, setLocale, getDefaultLocale, supportedLocales } = await import('./index.js')

test('t() reads the active locale with en fallback and key passthrough', async () => {
  setLocale('en')
  assert.equal(t('username'), 'Username')
  setLocale('zh')
  assert.notEqual(t('username'), 'Username')
  // 当前语言缺失的键回退到 en，再缺失则原样返回键名
  assert.equal(t('__missing_key__'), '__missing_key__')
})

test('setLocale persists and rejects unknown codes', () => {
  assert.equal(setLocale('zh'), true)
  assert.equal(globalThis.localStorage.getItem('sunshine-locale'), 'zh')
  assert.equal(getDefaultLocale(), 'zh')
  assert.equal(setLocale('klingon'), false)
})

test('getDefaultLocale matches browser language over the default', () => {
  globalThis.localStorage.store = {}
  Object.defineProperty(globalThis, 'navigator', { value: { language: 'de-DE' }, configurable: true })
  // 模块顶层的默认值在加载时已定；显式清除持久化后按当前浏览器语言重查
  assert.equal(getDefaultLocale(), 'de')
})

test('all locales expose the welcome keys', async () => {
  const fs = await import('node:fs')
  const path = await import('node:path')
  const dir = path.join(import.meta.dirname, 'locales')
  for (const file of fs.readdirSync(dir)) {
    const table = (await import(`./locales/${file}`)).default
    for (const key of ['username', 'password', 'welcome_success']) {
      assert.notEqual(table[key], undefined, `${file} missing ${key}`)
    }
  }
  assert.ok(supportedLocales.length >= 20)
})

test('document.lang maps locale codes to BCP 47 tags', async () => {
  const tags = []
  Object.defineProperty(globalThis, 'document', {
    value: { documentElement: { set lang(value) { tags.push(value) } } },
    configurable: true,
  })
  const mod = await import('./index.js')
  for (const code of ['zh_TW', 'ja', 'de', 'zh', 'en_GB']) {
    mod.setLocale(code)
    // Vue watch 异步冲刷且同步去重，逐个等微任务才能记录每次语言变化
    await new Promise((resolve) => setTimeout(resolve, 0))
  }
  // Vue watch 默认异步冲刷，等微任务后再断言
  assert.deepEqual(tags, ['zh-TW', 'ja', 'de', 'zh-CN', 'en-GB'])
})
