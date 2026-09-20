import test from 'node:test'
import assert from 'node:assert/strict'
import { dlssNrText } from './dlssNrMessages.js'

test('NR instructions distinguish runtime and SDR/HDR activation for both languages', () => {
  for (const locale of ['en', 'zh', 'zh_TW']) {
    const text = dlssNrText(locale)
    assert.match(text.selectRuntime, /nvngx_dlssnr\.dll/)
    assert.match(text.intro, /SDR/)
    assert.match(text.enableHint, /HDR/)
    assert.doesNotMatch(text.acquisitionDescription, /RTX_Video_SDK/)
    assert.ok(text.states.degraded)
  }
})
