/**
 * AI provider presets for the Mita control-panel entry point.
 */

export const STORAGE_KEY = 'sunshine-ai-config'

export const DEFAULT_CONFIG = {
  provider: 'openai',
  apiKey: '',
  apiBase: 'https://api.openai.com/v1',
  model: 'gpt-4.1-mini',
  compatibility: 'openai-chat',
  temperature: 0.3,
  max_tokens: 2048,
  enabled: false,
}

export const AI_PROVIDERS = [
  {
    label: 'OpenAI',
    value: 'openai',
    base: 'https://api.openai.com/v1',
    models: ['gpt-4.1-mini', 'gpt-4.1', 'gpt-4o-mini'],
    apiType: 'openai',
    compatibility: 'openai-chat',
  },
  {
    label: 'Anthropic',
    value: 'anthropic',
    base: 'https://api.anthropic.com',
    models: ['claude-3-5-haiku-latest', 'claude-sonnet-4-5'],
    apiType: 'anthropic',
    compatibility: 'anthropic-messages',
  },
  {
    label: 'DeepSeek',
    value: 'deepseek',
    base: 'https://api.deepseek.com/v1',
    models: ['deepseek-chat', 'deepseek-reasoner'],
    apiType: 'openai',
    compatibility: 'openai-chat',
  },
  {
    label: 'Qwen',
    value: 'qwen',
    base: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    models: ['qwen-plus', 'qwen-turbo', 'qwen-max'],
    apiType: 'openai',
    compatibility: 'openai-chat',
  },
  {
    label: 'Gemini',
    value: 'gemini',
    base: 'https://generativelanguage.googleapis.com/v1beta/openai',
    models: ['gemini-2.5-flash', 'gemini-2.5-pro'],
    apiType: 'openai',
    compatibility: 'openai-chat',
  },
  {
    label: 'OpenRouter',
    value: 'openrouter',
    base: 'https://openrouter.ai/api/v1',
    models: ['openai/gpt-4.1-mini', 'anthropic/claude-sonnet-4.5', 'deepseek/deepseek-chat-v3.1'],
    apiType: 'openai',
    compatibility: 'openai-chat',
  },
  {
    label: 'Ollama',
    value: 'ollama',
    base: 'http://localhost:11434/v1',
    models: ['llama3.1', 'qwen2.5', 'gemma3'],
    apiType: 'openai',
    compatibility: 'openai-chat',
  },
  {
    label: 'Mita / Custom',
    value: 'custom',
    base: '',
    models: [],
    apiType: 'openai',
    compatibility: 'openai-chat',
  },
]
