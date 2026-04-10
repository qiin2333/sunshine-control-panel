/**
 * AI HTTP 通信层
 * 负责与各类 AI 服务 API 的通信，通过 Tauri 代理绕过 CORS
 */

import { AI_PROVIDERS } from './aiProviders.js'

/**
 * 通过 Tauri 后端代理发送 HTTP 请求（绕过 CORS）
 * 如果不在 Tauri 环境则回退到 fetch
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
  // 回退到直接 fetch（Web 环境）
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

/**
 * 获取供应商的 API 类型
 */
export function getApiType(providerValue) {
  const provider = AI_PROVIDERS.find((p) => p.value === providerValue)
  return provider?.apiType || 'openai'
}

/**
 * 调用 OpenAI 兼容 API
 */
export async function callOpenAI(apiBase, apiKey, model, messages, maxTokens = 2048) {
  const headers = {}
  if (apiKey) {
    headers['Authorization'] = `Bearer ${apiKey}`
  }

  const base = apiBase.replace(/\/+$/, '')
  const data = await proxyFetch(`${base}/chat/completions`, 'POST', headers, {
    model,
    messages,
    temperature: 0.7,
    max_tokens: maxTokens,
  })
  return data.choices?.[0]?.message?.content || ''
}

/**
 * 调用 Anthropic Claude API
 */
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

/**
 * 统一 LLM 调用入口
 * @param {object} config - AI 配置
 * @param {Array} messages - 消息列表
 * @param {number} maxTokens - 最大 token 数
 */
export async function callLLM(config, messages, maxTokens = 2048) {
  const apiType = getApiType(config.provider)
  if (apiType === 'anthropic') {
    return callAnthropic(config.apiBase, config.apiKey, config.model, messages, maxTokens)
  }
  return callOpenAI(config.apiBase, config.apiKey, config.model, messages, maxTokens)
}

/**
 * 构建带图片的 vision 消息内容（多模态）
 * @param {string} text - 文本提示
 * @param {string} imageDataUrl - data:image/jpeg;base64,... 格式的图片
 * @returns 适用于 OpenAI/Anthropic vision API 的 content 数组
 */
export function buildVisionContent(text, imageDataUrl) {
  // 从 data URL 提取 base64 和 media type
  const match = imageDataUrl.match(/^data:(image\/\w+);base64,(.+)$/)
  if (!match) return text // fallback to text-only

  const [, mediaType, base64Data] = match

  return [
    { type: 'text', text },
    {
      type: 'image_url',
      image_url: { url: imageDataUrl, detail: 'low' }, // low detail = fewer tokens
    },
  ]
}

/**
 * 构建 Anthropic 格式的 vision 消息内容
 */
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

/**
 * 调用带视觉能力的 LLM（发送截屏 + 文本提示）
 */
export async function callVisionLLM(config, systemPrompt, userText, imageDataUrl, maxTokens = 512) {
  const apiType = getApiType(config.provider)

  if (apiType === 'anthropic') {
    const content = buildAnthropicVisionContent(userText, imageDataUrl)
    const messages = [{ role: 'user', content }]
    return callAnthropic(config.apiBase, config.apiKey, config.model, [
      { role: 'system', content: systemPrompt },
      ...messages,
    ], maxTokens)
  }

  // OpenAI 兼容（GPT-4o, Qwen-VL, GLM-4V 等）
  const content = buildVisionContent(userText, imageDataUrl)
  const messages = [
    { role: 'system', content: systemPrompt },
    { role: 'user', content },
  ]
  return callOpenAI(config.apiBase, config.apiKey, config.model, messages, maxTokens)
}

/**
 * 从 API 拉取可用模型列表
 */
export async function fetchModels(apiBase, apiKey, providerValue) {
  const apiType = getApiType(providerValue)

  // Anthropic 不支持 /models 列表接口
  if (apiType === 'anthropic') return []

  if (!apiBase) return []

  const headers = {}
  if (apiKey) {
    headers['Authorization'] = `Bearer ${apiKey}`
  }

  const base = apiBase.replace(/\/+$/, '')
  const data = await proxyFetch(`${base}/models`, 'GET', headers, null)

  return (data.data || data.models || [])
    .map((m) => m.id || m.name || m)
    .filter((m) => typeof m === 'string' && m.length > 0)
    .sort()
}
