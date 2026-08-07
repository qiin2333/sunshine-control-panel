/**
 * AI HTTP client.
 *
 * The Mita control panel is the only human-facing configuration entry, while
 * Sunshine's /api/ai/config and /api/ai/chat/completions remain the shared
 * storage and execution path for every AI feature.
 */

import { AI_PROVIDERS } from './aiProviders.js'

async function getProxyUrl() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    return await invoke('get_proxy_url_command')
  } catch {
    return 'https://localhost:47990'
  }
}

async function fetchJson(url, options = {}) {
  const resp = await fetch(url, options)
  const data = await resp.json().catch(() => ({}))
  if (!resp.ok || data.status === 'error') {
    const message = typeof data.error === 'string' ? data.error : data.error?.message
    throw new Error(message || `${resp.status} - ${JSON.stringify(data).substring(0, 200)}`)
  }
  return data
}

function isSunshineUnavailableError(error) {
  return String(error?.message || error || '').includes('Sunshine service is unavailable')
}

async function refreshSunshineProxyTarget() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('refresh_sunshine_target')
    return true
  } catch {
    try {
      const invoke = window.__TAURI__?.core?.invoke
      if (!invoke) return false
      await invoke('refresh_sunshine_target')
      return true
    } catch {
      return false
    }
  }
}

/**
 * Generic proxy for provider helper endpoints such as /models.
 */
export async function proxyFetch(url, method, headers, body) {
  const tauri = window.__TAURI__
  if (tauri?.core?.invoke) {
    const result = await tauri.core.invoke('ai_api_proxy', {
      request: {
        url,
        method,
        headers: headers || {},
        body: body ? JSON.stringify(body) : null,
      },
    })
    return JSON.parse(result)
  }

  const resp = await fetch(url, {
    method,
    headers: { ...headers, 'Content-Type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!resp.ok) {
    const error = await resp.text()
    throw new Error(`${resp.status} - ${error.substring(0, 200)}`)
  }
  return resp.json()
}

export function getApiType(providerValue) {
  const provider = AI_PROVIDERS.find((p) => p.value === providerValue)
  return provider?.apiType || 'openai'
}

export function getCompatibility(providerValue) {
  const provider = AI_PROVIDERS.find((p) => p.value === providerValue)
  return provider?.compatibility || 'openai-chat'
}

export function isApiKeyRequired(config) {
  const apiBase = config?.apiBase || ''
  return config?.provider !== 'ollama' &&
    !apiBase.includes('localhost') &&
    !apiBase.includes('127.0.0.1') &&
    !apiBase.includes('[::1]')
}

export async function callOpenAI(apiBase, apiKey, model, messages, maxTokens = 2048) {
  const headers = {}
  if (apiKey) headers.Authorization = `Bearer ${apiKey}`

  const base = apiBase.replace(/\/+$/, '')
  const data = await proxyFetch(`${base}/chat/completions`, 'POST', headers, {
    model,
    messages,
    temperature: 0.7,
    max_tokens: maxTokens,
  })
  return data.choices?.[0]?.message?.content || ''
}

export async function callAnthropic(apiBase, apiKey, model, messages, maxTokens = 2048) {
  const systemMsg = messages.find((m) => m.role === 'system')?.content || ''
  const chatMsgs = messages.filter((m) => m.role !== 'system')

  const base = apiBase.replace(/\/+$/, '')
  const data = await proxyFetch(
    `${base}/v1/messages`,
    'POST',
    {
      'x-api-key': apiKey,
      'anthropic-version': '2023-06-01',
    },
    { model, system: systemMsg, messages: chatMsgs, max_tokens: maxTokens }
  )
  return data.content?.map((c) => c.text).join('') || ''
}

export async function callLLM(config, messages, maxTokens = 2048, requestOptions = {}) {
  const proxyUrl = await getProxyUrl()
  const url = `${proxyUrl}/api/ai/chat/completions`
  const options = {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    signal: requestOptions.signal,
    body: JSON.stringify({
      model: config.model,
      messages,
      temperature: Number(config.temperature) || 0.3,
      max_tokens: Number(maxTokens || config.max_tokens) || 2048,
    }),
  }

  let data
  try {
    data = await fetchJson(url, options)
  } catch (error) {
    if (!isSunshineUnavailableError(error) || !(await refreshSunshineProxyTarget())) {
      throw error
    }
    data = await fetchJson(url, options)
  }
  return data.choices?.[0]?.message?.content || ''
}

export function buildVisionContent(text, imageDataUrl) {
  const match = imageDataUrl.match(/^data:(image\/\w+);base64,(.+)$/)
  if (!match) return text

  return [
    { type: 'text', text },
    {
      type: 'image_url',
      image_url: { url: imageDataUrl, detail: 'low' },
    },
  ]
}

export function buildAnthropicVisionContent(text, imageDataUrl) {
  const match = imageDataUrl.match(/^data:(image\/\w+);base64,(.+)$/)
  if (!match) return text

  const [, mediaType, base64Data] = match
  return [
    {
      type: 'image',
      source: { type: 'base64', media_type: mediaType, data: base64Data },
    },
    { type: 'text', text },
  ]
}

export async function callVisionLLM(
  config,
  systemPrompt,
  userText,
  imageDataUrl,
  maxTokens = 512,
  requestOptions = {}
) {
  const isAnthropic = getCompatibility(config.provider) === 'anthropic-messages' || config.compatibility === 'anthropic-messages'
  const content = isAnthropic
    ? buildAnthropicVisionContent(userText, imageDataUrl)
    : buildVisionContent(userText, imageDataUrl)

  return callLLM(config, [
    { role: 'system', content: systemPrompt },
    { role: 'user', content },
  ], maxTokens, requestOptions)
}

export async function fetchModels(apiBase, apiKey, providerValue, syncConfig) {
  const apiType = getApiType(providerValue)
  if (apiType === 'anthropic') return []
  if (!apiBase) return []

  let data
  if (apiKey) {
    const base = apiBase.replace(/\/+$/, '')
    data = await proxyFetch(`${base}/models`, 'GET', { Authorization: `Bearer ${apiKey}` }, null)
  } else {
    await syncConfig()
    const proxyUrl = await getProxyUrl()
    data = await fetchJson(`${proxyUrl}/api/ai/models`)
  }

  return (data.data || data.models || [])
    .map((m) => m.id || m.name || m)
    .filter((m) => typeof m === 'string' && m.length > 0)
    .sort()
}
