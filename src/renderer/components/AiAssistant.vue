<template>
  <div class="ai-settings-wrapper">
    <div class="ai-header">
      <div class="ai-heading">
        <span class="header-avatar" aria-hidden="true">
          <img src="/mita-pixel.png" alt="" />
        </span>
        <div class="title-stack">
          <span class="mita-kicker">
            <span class="status-dot" :class="{ active: config.enabled }"></span>
            M.I.T.A / AI ASSISTANT
          </span>
          <h2>{{ t.aiAssistant.title }}</h2>
        </div>
      </div>
      <el-tag :type="config.enabled ? 'success' : 'info'" size="small" round class="status-tag">
        {{ config.enabled ? t.aiAssistant.enabled : t.aiAssistant.disabled }}
      </el-tag>
    </div>

    <div class="ai-content" :class="{ chatting: isChatting }">
      <!-- AI 配置区 -->
      <div class="ai-section config-section" :class="{ collapsed: !showConfig }">
        <div class="section-header">
          <div class="config-heading">
            <span class="section-title"><el-icon><Setting /></el-icon>{{ t.aiAssistant.modelConfig }}</span>
            <span v-if="!showConfig" class="config-summary">
              {{ activeProviderLabel }}<template v-if="config.model"> · {{ config.model }}</template>
            </span>
          </div>
          <div class="section-actions">
            <el-switch v-model="config.enabled" :disabled="isClearingApiKey" @change="saveConfig(false)" @click.stop />
            <el-button
              class="config-toggle"
              text
              circle
              :icon="showConfig ? ArrowUp : ArrowDown"
              :title="t.aiAssistant.modelConfig"
              :aria-label="t.aiAssistant.modelConfig"
              :aria-expanded="showConfig"
              @click="showConfig = !showConfig"
            />
          </div>
        </div>

        <el-form v-show="showConfig" :model="config" label-width="120px" class="ai-form">
          <el-form-item :label="t.aiAssistant.provider">
            <el-select v-model="config.provider" class="field-control" @change="onProviderChange">
              <el-option v-for="p in providers" :key="p.value" :label="p.label" :value="p.value" />
            </el-select>
          </el-form-item>

          <el-form-item :label="t.aiAssistant.apiUrl">
            <el-input v-model="config.apiBase" class="field-control" placeholder="https://api.openai.com/v1" />
          </el-form-item>

          <el-form-item label="Compatibility">
            <el-select v-model="config.compatibility" class="field-control">
              <el-option label="OpenAI Chat Completions" value="openai-chat" />
              <el-option label="Anthropic Messages" value="anthropic-messages" />
            </el-select>
          </el-form-item>

          <el-form-item :label="t.aiAssistant.apiKey">
            <el-input
              v-model="config.apiKey"
              type="password"
              show-password
              class="field-control"
              :placeholder="config.apiKeyConfigured ? '••••••••' : 'sk-...'"
            >
              <template v-if="config.apiKeyConfigured" #append>
                <el-button :loading="isClearingApiKey" @click="clearApiKey">{{ t.aiAssistant.clearApiKey }}</el-button>
              </template>
            </el-input>
            <span class="form-tip">{{ t.aiAssistant.apiKeyHint }}</span>
          </el-form-item>

          <el-form-item :label="t.aiAssistant.model">
            <div class="model-field">
              <el-select
                v-model="config.model"
                filterable
                allow-create
                default-first-option
                style="flex: 1"
                :placeholder="t.aiAssistant.modelPlaceholder"
              >
                <el-option-group v-if="availableModels.length > 0" :label="t.aiAssistant.availableModels">
                  <el-option v-for="m in availableModels" :key="m" :label="m" :value="m" />
                </el-option-group>
              </el-select>
              <el-button
                :icon="Refresh"
                :loading="isFetchingModels"
                @click="fetchRemoteModels"
                :title="t.aiAssistant.fetchModels"
                circle
              />
            </div>
            <span class="form-tip">{{ t.aiAssistant.modelHint }}</span>
          </el-form-item>

          <div class="form-actions">
            <el-button type="primary" @click="testConnection" :loading="isLoading"> {{ t.aiAssistant.testConnection }} </el-button>
            <el-button :disabled="isClearingApiKey" @click="saveConfig()">{{ t.aiAssistant.saveConfig }}</el-button>
            <el-tag v-if="isConnected" type="success" class="conn-status"> {{ t.aiAssistant.connected }} </el-tag>
          </div>
        </el-form>
      </div>

      <!-- AI 对话区 -->
      <div class="ai-section chat-section">
        <div class="section-header">
          <div class="chat-heading">
            <span class="section-title"><el-icon><ChatDotRound /></el-icon>{{ t.aiAssistant.smartChat }}</span>
          </div>
          <el-button v-if="chatHistory.length" size="small" text @click="clearHistory">
            {{ t.aiAssistant.clearHistory }}
          </el-button>
        </div>

        <div
          ref="chatContainer"
          class="chat-messages"
          :class="{ empty: chatHistory.length === 0 }"
          aria-live="polite"
          @wheel="handleChatWheel"
        >
          <div v-if="chatHistory.length === 0" class="welcome-hint">
            <div class="mita-avatar-lg">
              <img src="/mita-pixel.png" :alt="t.aiAssistant.mitaName" class="mita-pixel-img-lg" />
            </div>
            <p>{{ t.aiAssistant.welcomeMsg }}</p>
            <p class="hint-sub">{{ t.aiAssistant.welcomeHint }}</p>
            <div class="hint-examples">
              <button
                v-for="example in exampleQueries"
                :key="example"
                type="button"
                class="example-prompt"
                :disabled="!config.enabled"
                @click="selectExample(example)"
              >
                {{ example }}
              </button>
            </div>
          </div>

          <div v-for="(msg, idx) in chatHistory" :key="idx" class="chat-message" :class="msg.role">
            <div class="msg-avatar">
              <el-icon v-if="msg.role === 'user'" class="user-avatar"><UserFilled /></el-icon>
              <img v-else src="/mita-pixel.png" :alt="t.aiAssistant.mitaName" class="mita-pixel-img" />
            </div>
            <div class="msg-bubble">
              <div class="msg-content" v-html="formatMessage(msg.content, msg.parsedAction)"></div>
              <div v-if="msg.parsedAction" class="msg-action">
                <div class="action-main">
                  <el-button type="primary" size="small" @click="applyAction(msg.parsedAction)">
                    {{ getActionButtonText(msg.parsedAction) }}
                  </el-button>
                  <span class="action-hint">{{ getActionHint(msg.parsedAction) }}</span>
                </div>
              </div>
              <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
              <el-button v-if="msg.isError" class="retry-btn" type="warning" size="small" :icon="RefreshRight" @click="retryLastMessage" round>
                {{ t.aiAssistant.retry }}
              </el-button>
            </div>
          </div>

          <div v-if="isLoading" class="chat-message assistant">
            <div class="msg-avatar"><img src="/mita-pixel.png" :alt="t.aiAssistant.mitaName" class="mita-pixel-img" /></div>
            <div class="msg-bubble loading">
              <span class="dot-loader"> <span></span><span></span><span></span> </span>
            </div>
          </div>
        </div>

        <div class="chat-input-area">
          <el-input
            ref="chatInput"
            v-model="currentInput"
            type="textarea"
            :autosize="{ minRows: 1, maxRows: 4 }"
            resize="none"
            :placeholder="t.aiAssistant.inputPlaceholder"
            @keydown.enter.exact.prevent="handleSend"
            :disabled="!config.enabled"
          />
          <el-button
            type="primary"
            :icon="Promotion"
            @click="handleSend"
            :loading="isLoading"
            :disabled="!config.enabled || !currentInput.trim()"
            class="send-btn"
          >
            {{ t.aiAssistant.send }}
          </el-button>
        </div>
      </div>

    </div>
  </div>
</template>

<script setup>
import { ref, nextTick, watch, computed } from 'vue'
import MarkdownIt from 'markdown-it'
import {
  ArrowDown,
  ArrowUp,
  Promotion,
  Setting,
  ChatDotRound,
  Refresh,
  RefreshRight,
  UserFilled,
} from '@element-plus/icons-vue'
import { useAiAssistant, AI_PROVIDERS } from '../composables/useAiAssistant.js'
import { useI18n } from '../desktop/i18n/index.js'

const { t } = useI18n()

const {
  config,
  needsConfiguration,
  isConnected,
  isLoading,
  isClearingApiKey,
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
  clearApiKey,
  saveConfig,
} = useAiAssistant()

const providers = AI_PROVIDERS
const chatContainer = ref(null)
const chatInput = ref(null)
const showConfig = ref(needsConfiguration.value)
const markdown = new MarkdownIt({
  breaks: true,
  html: false,
  linkify: false,
  typographer: false,
})

const isChatting = computed(() => chatHistory.value.length > 0 || isLoading.value)
const activeProviderLabel = computed(
  () => providers.find((provider) => provider.value === config.provider)?.label || config.provider,
)

// 示例问题
const exampleQueries = computed(() => [
  t.value.aiAssistant.exampleQueries.addKeyboardCmd,
  t.value.aiAssistant.exampleQueries.closeGameBar,
  t.value.aiAssistant.exampleQueries.highPerfPlan,
  t.value.aiAssistant.exampleQueries.optimizeGame,
  t.value.aiAssistant.exampleQueries.toggleHdr,
  t.value.aiAssistant.exampleQueries.checkLogs,
  t.value.aiAssistant.exampleQueries.analyzeConnection,
])

/**
 * 获取操作按钮文案
 */
function getActionButtonText(action) {
  const map = {
    add_menu_cmd: t.value.aiAssistant.actions.addMenuCmd,
    add_prep_cmd: t.value.aiAssistant.actions.addPrepCmd,
    enhance_apps: t.value.aiAssistant.actions.enhanceApps,
    modify_config: t.value.aiAssistant.actions.modifyConfig,
  }
  return map[action.action] || t.value.aiAssistant.actions.apply
}

/**
 * 获取操作提示文案
 */
function getActionHint(action) {
  const appName = action.app_name || 'Desktop'
  const count = action.commands?.length || action.changes?.length || action.apps?.length || 0
  const map = {
    add_menu_cmd: t.value.aiAssistant.actionHints.addMenuCmd.replace('{count}', count).replace('{appName}', appName),
    add_prep_cmd: t.value.aiAssistant.actionHints.addPrepCmd.replace('{count}', count).replace('{appName}', appName),
    enhance_apps: t.value.aiAssistant.actionHints.enhanceApps.replace('{count}', count),
    modify_config: t.value.aiAssistant.actionHints.modifyConfig.replace('{count}', count),
  }
  return map[action.action] || ''
}

/**
 * 发送消息
 */
async function handleSend() {
  if (!currentInput.value.trim() || isLoading.value) return
  showConfig.value = false
  const accepted = await sendMessage(currentInput.value)
  if (!accepted) {
    showConfig.value = true
    return
  }
  await nextTick()
  scrollToBottom()
}

/**
 * 选择示例问题并交给用户确认
 */
function selectExample(text) {
  currentInput.value = text
  nextTick(() => chatInput.value?.focus())
}

/**
 * 滚动到底部
 */
function scrollToBottom() {
  if (chatContainer.value) {
    chatContainer.value.scrollTop = chatContainer.value.scrollHeight
  }
}

/**
 * Keep horizontal scrolling local to code blocks and support both a mouse
 * tilt wheel (deltaX) and Shift + wheel in WebView/Chromium.
 */
function handleChatWheel(event) {
  const rawDelta = Math.abs(event.deltaX) > 0.01 ? event.deltaX : event.shiftKey ? event.deltaY : 0
  if (!rawDelta) return

  const eventTarget = event.target instanceof Element ? event.target : null
  const codeBlock = eventTarget?.closest('pre')
  const scrollTarget = codeBlock?.scrollWidth > codeBlock?.clientWidth ? codeBlock : chatContainer.value
  if (!scrollTarget || scrollTarget.scrollWidth <= scrollTarget.clientWidth) return

  const multiplier = event.deltaMode === WheelEvent.DOM_DELTA_LINE
    ? 24
    : event.deltaMode === WheelEvent.DOM_DELTA_PAGE
      ? scrollTarget.clientWidth
      : 1
  const previousScrollLeft = scrollTarget.scrollLeft
  scrollTarget.scrollLeft += rawDelta * multiplier

  if (scrollTarget.scrollLeft !== previousScrollLeft) event.preventDefault()
}

/**
 * Render readable Markdown. Once an action is parsed, its machine-readable
 * JSON is represented by the action card instead of being dumped into chat.
 */
function formatMessage(content, parsedAction) {
  if (!content) return ''

  let displayContent = content
  if (parsedAction) {
    displayContent = displayContent.replace(/```json\s*[\s\S]*?\s*```/i, '').trim()
    if (!displayContent) displayContent = parsedAction.explanation || ''
  }

  return markdown.render(displayContent)
}

/**
 * 格式化时间
 */
function formatTime(ts) {
  if (!ts) return ''
  const d = new Date(ts)
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
}

// 自动滚动到底部
watch(
  () => chatHistory.value.length,
  () => {
    nextTick(scrollToBottom)
  },
)

watch(needsConfiguration, (required) => {
  if (required) showConfig.value = true
})
</script>


<style scoped lang="less" src="./AiAssistant.less"></style>
