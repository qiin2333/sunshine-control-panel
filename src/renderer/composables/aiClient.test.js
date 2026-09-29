import assert from 'node:assert/strict'
import test from 'node:test'

import { callLLM, codexAuthPhase, fetchModels, hasAiCredentials, requestCodexAuth } from './aiClient.js'
import { CHATGPT_MODELS } from './aiProviders.js'

test('ChatGPT model presets use current Codex models', () => {
  assert.deepEqual(CHATGPT_MODELS, ['gpt-6-luna', 'gpt-6-sol', 'gpt-6-astra'])
})

test('pending reauthorization keeps polling while an old account is connected', () => {
  assert.equal(codexAuthPhase({ connected: true, pending: true }), 'pending')
  assert.equal(codexAuthPhase({ connected: true, pending: false, error: 'expired_token' }), 'error')
  assert.equal(codexAuthPhase({ connected: true, pending: false }), 'connected')
})

test('AI credentials respect the selected OpenAI authentication mode', () => {
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'chatgpt', codexConnected: false, apiKeyConfigured: true }), false)
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'chatgpt', codexConnected: true }), true)
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'apiKey', apiBase: 'https://api.openai.com/v1', apiKeyConfigured: true }), true)
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'apiKey', apiBase: 'https://api.openai.com/v1' }), false)
  assert.equal(hasAiCredentials({ provider: 'ollama', apiBase: 'http://localhost:11434/v1' }), true)
})

test('ChatGPT model discovery uses Sunshine even when an API key is saved', async () => {
  const previousFetch = globalThis.fetch
  let synced = false
  globalThis.fetch = async (url) => {
    assert.match(url, /\/api\/ai\/models$/)
    return { ok: true, json: async () => ({ data: [{ id: 'gpt-6-luna' }] }) }
  }
  try {
    const models = await fetchModels('', 'sk-saved', 'openai', async () => { synced = true }, 'chatgpt')
    assert.equal(synced, true)
    assert.deepEqual(models, ['gpt-6-luna'])
  } finally {
    globalThis.fetch = previousFetch
  }
})

test('ChatGPT requests omit unsupported tuning fields while API key requests retain them', async () => {
  const previousFetch = globalThis.fetch
  const bodies = []
  globalThis.fetch = async (_url, options) => {
    bodies.push(JSON.parse(options.body))
    return { ok: true, json: async () => ({ choices: [{ message: { content: 'ok' } }] }) }
  }
  try {
    const messages = [{ role: 'user', content: 'hi' }]
    await callLLM({ provider: 'openai', authMode: 'chatgpt', model: 'gpt-6-luna' }, messages)
    await callLLM({ provider: 'openai', authMode: 'apiKey', model: 'gpt-4.1-mini', temperature: 0.4 }, messages)
    assert.equal('temperature' in bodies[0], false)
    assert.equal('max_tokens' in bodies[0], false)
    assert.equal(bodies[1].temperature, 0.4)
    assert.equal(bodies[1].max_tokens, 2048)
  } finally {
    globalThis.fetch = previousFetch
  }
})

test('Codex auth POST requests send JSON for Sunshine content-type checks', async () => {
  const previousFetch = globalThis.fetch
  const paths = []
  globalThis.fetch = async (url, options = {}) => {
    paths.push(url.split('/').at(-1))
    assert.equal(options.method, 'POST')
    assert.equal(options.headers['Content-Type'], 'application/json')
    assert.equal(options.body, '{}')
    return { ok: true, json: async () => ({ connected: false, pending: true }) }
  }
  try {
    for (const path of ['start', 'poll', 'logout']) await requestCodexAuth(path)
    assert.deepEqual(paths, ['start', 'poll', 'logout'])
  } finally {
    globalThis.fetch = previousFetch
  }
})
