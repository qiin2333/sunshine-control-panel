/**
 * AI 供应商预设配置
 */

export const STORAGE_KEY = 'sunshine-ai-config'

export const DEFAULT_CONFIG = {
  provider: 'openai',
  apiKey: '',
  apiBase: 'https://api.openai.com/v1',
  model: 'gpt-4o-mini',
  enabled: false,
}

export const AI_PROVIDERS = [
  { label: 'OpenAI', value: 'openai', base: 'https://api.openai.com/v1', models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo'], apiType: 'openai' },
  { label: 'Claude (Anthropic)', value: 'anthropic', base: 'https://api.anthropic.com', models: ['claude-sonnet-4-20250514', 'claude-3-5-haiku-20241022', 'claude-3-opus-20240229'], apiType: 'anthropic' },
  { label: 'DeepSeek', value: 'deepseek', base: 'https://api.deepseek.com/v1', models: ['deepseek-chat', 'deepseek-reasoner'], apiType: 'openai' },
  { label: '通义千问 (Qwen)', value: 'qwen', base: 'https://dashscope.aliyuncs.com/compatible-mode/v1', models: ['qwen-max', 'qwen-plus', 'qwen-turbo'], apiType: 'openai' },
  { label: '智谱 (GLM)', value: 'glm', base: 'https://open.bigmodel.cn/api/paas/v4', models: ['glm-4-plus', 'glm-4', 'glm-4-flash'], apiType: 'openai' },
  { label: 'OpenRouter', value: 'openrouter', base: 'https://openrouter.ai/api/v1', models: ['anthropic/claude-sonnet-4-20250514', 'anthropic/claude-3.5-haiku', 'google/gemini-2.0-flash-001', 'deepseek/deepseek-chat-v3-0324'], apiType: 'openai' },
  { label: 'Ollama (本地)', value: 'ollama', base: 'http://localhost:11434/v1', models: ['llama3', 'qwen2', 'mistral'], apiType: 'openai' },
  { label: 'OpenAI 兼容', value: 'compatible', base: '', models: [], apiType: 'openai' },
]
