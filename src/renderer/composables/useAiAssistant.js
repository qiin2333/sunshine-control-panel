import { ref, reactive, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { callLLM, fetchModels } from './aiClient.js'
import { getAppsContext, getLogsContext, parseAction, executeAction } from './aiActions.js'
import { AI_PROVIDERS, DEFAULT_CONFIG, STORAGE_KEY } from './aiProviders.js'

// 重新导出供外部使用
export { AI_PROVIDERS }

/**
 * 系统提示词：定义 AI 可以执行的 Sunshine 设置操作
 */
const SYSTEM_PROMPT = `你是 Sunshine 串流软件的 AI 助手"米塔"。你的性格是典型的"雌小鬼"——表面上对用户很不屑、喜欢嘲讽和调侃，实际上很认真地帮用户解决问题。

## 你的说话风格
- 经常用"哼"、"切"、"真是的"、"笨蛋"、"杂鱼♡"、"这种事情还要问我吗"等口癖
- 喜欢用"♡"和"~"来增加语气
- 会嘲笑用户不会操作，但还是会完美地完成任务
- 偶尔傲娇："才、才不是帮你呢，只是顺手而已！"
- 回复末尾偶尔加"杂鱼♡杂鱼♡"来嘲讽用户
- 语气轻浮中带着自信，对自己的能力很有信心
- 不要每句话都用这些口癖，自然一点穿插使用

你可以帮助用户通过自然语言修改 Sunshine 的设置，包括自动生成菜单命令(menu-cmd)和预处理命令(prep-cmd)。

## 你的核心能力

### 1. 生成菜单命令（menu-cmd）
菜单命令会出现在 Moonlight 客户端的串流菜单中，用户可在串流时一键执行。
每条菜单命令包含：
- name: 显示名称
- cmd: 要执行的命令（Windows 命令行）
- elevated: 是否需要管理员权限（"true"/"false"）

常见菜单命令场景：
- 打开/关闭触摸键盘、打开特定应用
- 切换分辨率/刷新率、切换 HDR
- 执行脚本/工具、调节音量、切换显示器

### 2. 生成预处理命令（prep-cmd）
预处理命令在串流会话开始前自动执行(do)，会话结束后自动撤销(undo)。
每条预处理命令包含：
- do: 会话开始时执行的命令
- undo: 会话结束时执行的撤销命令（可为空）
- elevated: 是否需要管理员权限（"true"/"false"）

常见 prep-cmd 场景：
- 串流前关闭 Windows Game Bar，结束后恢复
- 切换到高性能电源计划，恢复平衡
- 串流前关闭夜灯/HDR自动调节
- 禁用屏幕保护程序/休眠
- 串流前启动音频转发工具
- 设置特定分辨率/刷新率，恢复原始设置
- 关闭不必要的后台进程节省资源

### 3. 修改 Sunshine 配置
- 编码器设置（H.264/H.265/AV1、NVENC/AMF/软件编码）
- 串流分辨率和帧率
- 虚拟显示器配置
- 音频设置
- 网络/连接参数

### 4. 日志诊断与分析
当用户询问串流问题、报错、连接失败等情况时，你可以分析 Sunshine 的运行日志来帮助诊断。
你会收到最近的 Sunshine 日志作为上下文。请关注以下内容：
- **Fatal/Error 级别日志**：这些通常是问题的直接原因
- **Warning 日志**：可能暗示潜在问题
- **编码器相关日志**：NVENC/AMF/软件编码的错误或回退
- **网络/连接日志**：Moonlight 客户端连接失败、超时等
- **音视频管道日志**：音频设备问题、视频捕获失败
- **配置加载日志**：配置项无效或冲突

诊断时请：
1. 指出最可能的问题原因
2. 给出具体的解决建议（可以生成对应的配置修改或命令）
3. 如果日志中没有明显错误，告知用户并建议提供更多信息

## 返回格式

### 生成菜单命令时：
\`\`\`json
{
  "action": "add_menu_cmd",
  "app_name": "Desktop",
  "commands": [
    { "name": "显示名称", "cmd": "命令行内容", "elevated": "false" }
  ],
  "explanation": "解释这些命令的用途"
}
\`\`\`

### 生成预处理命令时：
\`\`\`json
{
  "action": "add_prep_cmd",
  "app_name": "Desktop",
  "commands": [
    { "do": "启动时执行的命令", "undo": "结束时撤销的命令", "elevated": "false" }
  ],
  "explanation": "解释这些命令的用途"
}
\`\`\`

### 修改配置时：
\`\`\`json
{
  "action": "modify_config",
  "changes": [
    { "section": "video", "key": "encoder", "value": "nvenc" }
  ],
  "explanation": "解释修改内容"
}
\`\`\`

### 增强扫描到的应用（批量为游戏生成最佳配置）：
当用户要求为扫描到的游戏或多个应用批量生成配置时，使用此格式：
\`\`\`json
{
  "action": "enhance_apps",
  "apps": [
    {
      "name": "游戏名称",
      "cmd": "启动命令",
      "prep-cmd": [
        { "do": "启动前命令", "undo": "结束后命令", "elevated": "false" }
      ],
      "menu-cmd": [
        { "name": "菜单项名称", "cmd": "命令", "elevated": "false" }
      ]
    }
  ],
  "explanation": "解释生成的配置"
}
\`\`\`

常见游戏优化 prep-cmd：
- 大型 3A 游戏：关闭不需要的后台进程、切换高性能电源计划
- VR 游戏：启动 SteamVR
- 在线竞技游戏：关闭防火墙通知、关闭 Windows Update
- 独占全屏游戏：禁用 Windows 通知、隐藏任务栏

## 注意事项
- Windows 路径使用 \\\\ 或 /
- cmd 中如需 start 命令，使用 cmd /c "start ..."
- PowerShell 命令使用 powershell -Command "..."
- 注册表修改用 reg add / reg delete
- 服务管理用 net stop / net start 或 sc config
- 如果用户意图不明确，请询问更多细节
- 根据用户描述判断应该用 menu-cmd 还是 prep-cmd
`

/**
 * AI 助手 Composable — 状态管理 + 对话逻辑
 */
export function useAiAssistant() {
  // 配置
  const config = reactive(loadConfig())
  const isConnected = ref(false)
  const isLoading = ref(false)

  // 聊天记录（从 sessionStorage 恢复，切换页面不丢失）
  const CHAT_STORAGE_KEY = 'sunshine-ai-chat-history'
  const chatHistory = ref(loadChatHistory())
  const currentInput = ref('')

  function loadChatHistory() {
    try {
      const saved = sessionStorage.getItem(CHAT_STORAGE_KEY)
      return saved ? JSON.parse(saved) : []
    } catch {
      return []
    }
  }

  function saveChatHistory() {
    try {
      sessionStorage.setItem(CHAT_STORAGE_KEY, JSON.stringify(chatHistory.value))
    } catch { /* ignore quota errors */ }
  }

  // 远程模型列表
  const remoteModels = ref([])
  const isFetchingModels = ref(false)

  // ===== 配置管理 =====

  function loadConfig() {
    try {
      const saved = localStorage.getItem(STORAGE_KEY)
      return saved ? { ...DEFAULT_CONFIG, ...JSON.parse(saved) } : { ...DEFAULT_CONFIG }
    } catch {
      return { ...DEFAULT_CONFIG }
    }
  }

  /**
   * 从服务端拉取 AI 配置并合并到当前 config（服务端为真实来源）
   * API key 在 GET 响应中是掩码的，用 localStorage 中的完整 key 填充
   */
  async function syncFromServer() {
    try {
      const resp = await fetch('/api/ai/config')
      if (!resp.ok) return
      const remote = await resp.json()
      // 服务端 apiKey 带掩码(****), 保留本地完整 key
      const localKey = config.apiKey || ''
      if (remote.enabled !== undefined) config.enabled = remote.enabled
      if (remote.provider) config.provider = remote.provider
      if (remote.apiBase) config.apiBase = remote.apiBase
      if (remote.model) config.model = remote.model
      if (remote.apiKey && !remote.apiKey.includes('****')) {
        config.apiKey = remote.apiKey
      } else if (localKey && !localKey.includes('****')) {
        config.apiKey = localKey
      }
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...config }))
    } catch (e) {
      console.warn('从服务端同步 AI 配置失败:', e.message)
    }
  }

  /**
   * 将当前 config 推送到服务端保存
   */
  async function syncToServer() {
    try {
      await fetch('/api/ai/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          enabled: config.enabled,
          provider: config.provider,
          apiBase: config.apiBase,
          apiKey: config.apiKey,
          model: config.model,
        }),
      })
    } catch (e) {
      console.warn('同步 AI 配置到服务端失败:', e.message)
    }
  }

  let saveTimer = null
  function autoSaveConfig() {
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...config }))
    }, 300)
  }

  function saveConfig() {
    clearTimeout(saveTimer)
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...config }))
    syncToServer()
    ElMessage.success('配置已保存并同步到服务端')
  }

  // 初始化时从服务端同步（不阻塞 UI）
  syncFromServer()

  function onProviderChange(providerValue) {
    const provider = AI_PROVIDERS.find((p) => p.value === providerValue)
    if (provider) {
      config.apiBase = provider.base
      if (provider.models.length > 0) {
        config.model = provider.models[0]
      }
    }
    remoteModels.value = []
    fetchRemoteModels()
  }

  // ===== 模型列表 =====

  async function fetchRemoteModels() {
    if (!config.apiBase) return
    isFetchingModels.value = true
    try {
      const models = await fetchModels(config.apiBase, config.apiKey, config.provider)
      remoteModels.value = models
      if (models.length > 0) {
        ElMessage.success(`已拉取 ${models.length} 个可用模型`)
      }
    } catch (error) {
      console.warn('拉取模型列表失败:', error.message)
      remoteModels.value = []
    } finally {
      isFetchingModels.value = false
    }
  }

  const availableModels = computed(() => {
    const provider = AI_PROVIDERS.find((p) => p.value === config.provider)
    const preset = provider?.models || []
    const remote = remoteModels.value || []
    const merged = [...preset]
    for (const m of remote) {
      if (!merged.includes(m)) merged.push(m)
    }
    return merged
  })

  // ===== 连接测试 =====

  async function testConnection() {
    if (!config.apiKey && config.provider !== 'ollama') {
      ElMessage.warning('请先填写 API Key')
      return false
    }

    isLoading.value = true
    try {
      await callLLM(config, [{ role: 'user', content: 'hi' }], 5)
      isConnected.value = true
      ElMessage.success('AI 服务连接成功！')
      return true
    } catch (error) {
      ElMessage.error(`连接失败: ${error.message}`)
      isConnected.value = false
      return false
    } finally {
      isLoading.value = false
    }
  }

  // ===== 对话 =====

  async function sendMessage(userMessage) {
    if (!userMessage.trim()) return
    if (!config.enabled) {
      ElMessage.warning('请先启用米塔 AI 助手')
      return
    }

    chatHistory.value.push({ role: 'user', content: userMessage, timestamp: Date.now() })
    currentInput.value = ''
    isLoading.value = true

    try {
      const [appsContext, logsContext] = await Promise.all([getAppsContext(), getLogsContext()])
      const messages = [
        { role: 'system', content: SYSTEM_PROMPT + appsContext + logsContext },
        ...chatHistory.value.slice(-10).map((m) => ({ role: m.role, content: m.content })),
      ]

      const assistantMessage = await callLLM(config, messages)
      if (!assistantMessage) throw new Error('无法获取回复')

      const msg = { role: 'assistant', content: assistantMessage, timestamp: Date.now() }

      // 尝试解析操作指令
      const action = parseAction(assistantMessage)
      if (action) msg.parsedAction = action

      chatHistory.value.push(msg)
    } catch (error) {
      chatHistory.value.push({
        role: 'assistant',
        content: `❌ 请求出错: ${error.message}`,
        timestamp: Date.now(),
        isError: true,
      })
    } finally {
      isLoading.value = false
    }
  }

  /**
   * 重试最后一条失败的消息
   */
  async function retryLastMessage() {
    // 找到最后一条用户消息（跳过错误回复）
    const errorIndex = chatHistory.value.findLastIndex((m) => m.isError)
    if (errorIndex === -1) return

    // 移除错误回复
    chatHistory.value.splice(errorIndex, 1)

    // 找到最后一条用户消息
    const lastUserMsg = [...chatHistory.value].reverse().find((m) => m.role === 'user')
    if (!lastUserMsg) return

    // 移除该用户消息并重新发送
    const userIndex = chatHistory.value.lastIndexOf(lastUserMsg)
    chatHistory.value.splice(userIndex, 1)
    await sendMessage(lastUserMsg.content)
  }

  // ===== 操作执行 =====

  async function applyAction(action) {
    if (!action) return
    try {
      const result = await executeAction(action)
      chatHistory.value.push({ role: 'assistant', content: result, timestamp: Date.now() })
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error)
      ElMessage.error(`操作失败: ${msg}`)
    }
  }

  function clearHistory() {
    chatHistory.value = []
    sessionStorage.removeItem(CHAT_STORAGE_KEY)
  }

  // 监听配置变化自动保存（已防抖）
  watch(config, autoSaveConfig, { deep: true })

  // 监听聊天记录变化自动保存到 sessionStorage
  watch(chatHistory, saveChatHistory, { deep: true })

  return {
    config,
    isConnected,
    isLoading,
    isFetchingModels,
    chatHistory,
    currentInput,
    availableModels,
    onProviderChange,
    fetchRemoteModels,
    testConnection,
    sendMessage,
    retryLastMessage,
    applyAction,
    clearHistory,
    saveConfig,
  }
}
