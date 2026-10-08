import assert from 'node:assert/strict'
import test from 'node:test'

import { callLLM, callVisionLLM, codexAuthPhase, codexAuthRetryDelay, fetchModels, hasAiCredentials, isCodexAuthTerminalError, requestCodexAuth } from './aiClient.js'
import { applyCodexConnectionStatus, cacheCodexConnected, CHATGPT_MODELS, CODEX_AUTH_CONNECTED_KEY, readCachedCodexConnected } from './aiProviders.js'

test('ChatGPT model presets use current Codex models', () => {
  assert.deepEqual(CHATGPT_MODELS, ['gpt-6-luna', 'gpt-6-sol', 'gpt-6-astra'])
})

test('pending reauthorization keeps polling while an old account is connected', () => {
  assert.equal(codexAuthPhase({ connected: true, pending: true }), 'pending')
  assert.equal(codexAuthPhase({ connected: true, pending: false, error: 'expired_token' }), 'error')
  assert.equal(codexAuthPhase({ connected: true, pending: false }), 'connected')
})

test('temporary sign-in polling failures retry with capped backoff', () => {
  assert.equal(isCodexAuthTerminalError({ status: 400 }), true)
  assert.equal(isCodexAuthTerminalError({ status: 401 }), true)
  assert.equal(isCodexAuthTerminalError({ status: 429 }), false)
  assert.equal(isCodexAuthTerminalError({ status: 408 }), false)
  assert.equal(isCodexAuthTerminalError(new TypeError('network failed')), false)
  assert.equal(codexAuthRetryDelay(5, 0), 5)
  assert.equal(codexAuthRetryDelay(5, 2), 20)
  assert.equal(codexAuthRetryDelay(5, 8), 30)
})

test('AI credentials respect the selected OpenAI authentication mode', () => {
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'chatgpt', codexConnected: false, apiKeyConfigured: true }), false)
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'chatgpt', codexConnected: true }), true)
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'apiKey', apiBase: 'https://api.openai.com/v1', apiKeyConfigured: true }), true)
  assert.equal(hasAiCredentials({ provider: 'openai', authMode: 'apiKey', apiBase: 'https://api.openai.com/v1' }), false)
  assert.equal(hasAiCredentials({ provider: 'ollama', apiBase: 'http://localhost:11434/v1' }), true)
})

test('ChatGPT local fallback uses the latest cached account connection state', () => {
  const previousStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
  const values = new Map()
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => values.set(key, String(value)),
    },
  })
  try {
    const localConfig = { provider: 'openai', authMode: 'chatgpt', apiKeyConfigured: false }
    assert.equal(readCachedCodexConnected(), null)
    cacheCodexConnected(true)
    assert.equal(values.get(CODEX_AUTH_CONNECTED_KEY), 'true')
    assert.equal(hasAiCredentials(applyCodexConnectionStatus(localConfig, readCachedCodexConnected())), true)
    cacheCodexConnected(false)
    assert.equal(hasAiCredentials(applyCodexConnectionStatus(localConfig, readCachedCodexConnected())), false)
    assert.equal(applyCodexConnectionStatus({ authMode: 'apiKey' }, true).codexConnected, undefined)
  } finally {
    if (previousStorage) Object.defineProperty(globalThis, 'localStorage', previousStorage)
    else delete globalThis.localStorage
  }
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

test('ChatGPT vision requests use OpenAI image parts despite stale Anthropic compatibility', async () => {
  const previousFetch = globalThis.fetch
  let requestBody
  globalThis.fetch = async (_url, options) => {
    requestBody = JSON.parse(options.body)
    return { ok: true, json: async () => ({ choices: [{ message: { content: 'seen' } }] }) }
  }
  try {
    await callVisionLLM({
      provider: 'openai',
      authMode: 'chatgpt',
      compatibility: 'anthropic-messages',
      model: 'gpt-6-luna',
    }, 'system', 'describe', 'data:image/png;base64,aGVsbG8=')
    const userMessage = requestBody.messages.find((message) => message.role === 'user')
    assert.equal(userMessage.content[1].type, 'image_url')
    assert.equal(userMessage.content[1].image_url.url, 'data:image/png;base64,aGVsbG8=')
  } finally {
    globalThis.fetch = previousFetch
  }
})

test('Codex auth POST requests send JSON for Sunshine content-type checks', async () => {
  const previousFetch = globalThis.fetch
  const paths = []
  const bodies = []
  globalThis.fetch = async (url, options = {}) => {
    paths.push(url.split('/').at(-1))
    assert.equal(options.method, 'POST')
    assert.equal(options.headers['Content-Type'], 'application/json')
    bodies.push(JSON.parse(options.body))
    return { ok: true, json: async () => ({ connected: false, pending: true }) }
  }
  try {
    for (const path of ['start', 'poll', 'logout']) {
      await requestCodexAuth(path, path === 'poll' ? { flowId: '17' } : {})
    }
    assert.deepEqual(paths, ['start', 'poll', 'logout'])
    assert.deepEqual(bodies, [{}, { flowId: '17' }, {}])
  } finally {
    globalThis.fetch = previousFetch
  }
})
