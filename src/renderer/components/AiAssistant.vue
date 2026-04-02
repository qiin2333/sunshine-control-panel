<template>
  <div class="ai-settings-wrapper">
    <div class="ai-header">
      <h2>
        <el-icon class="header-icon"><MagicStick /></el-icon>
        米塔 AI 助手
      </h2>
      <el-tag :type="config.enabled ? 'success' : 'info'" size="small">
        {{ config.enabled ? '已启用' : '未启用' }}
      </el-tag>
    </div>

    <div class="ai-content">
      <!-- AI 配置区 -->
      <div class="ai-section config-section">
        <div class="section-header">
          <span class="section-title">
            <el-icon><Setting /></el-icon>
            模型配置
          </span>
          <el-switch v-model="config.enabled" />
        </div>

        <el-form :model="config" label-width="120px" class="ai-form">
          <el-form-item label="AI 供应商">
            <el-select v-model="config.provider" @change="onProviderChange" style="width: 100%; max-width: 400px">
              <el-option v-for="p in providers" :key="p.value" :label="p.label" :value="p.value" />
            </el-select>
          </el-form-item>

          <el-form-item label="API 地址">
            <el-input v-model="config.apiBase" placeholder="https://api.openai.com/v1" style="max-width: 400px" />
          </el-form-item>

          <el-form-item label="API Key">
            <el-input
              v-model="config.apiKey"
              type="password"
              show-password
              placeholder="sk-..."
              style="max-width: 400px"
            />
            <span class="form-tip">密钥仅保存在本地，不会上传</span>
          </el-form-item>

          <el-form-item label="模型">
            <div style="display: flex; gap: 8px; width: 100%; max-width: 400px">
              <el-select
                v-model="config.model"
                filterable
                allow-create
                default-first-option
                style="flex: 1"
                placeholder="选择或输入模型名称"
              >
                <el-option-group v-if="availableModels.length > 0" label="可用模型">
                  <el-option v-for="m in availableModels" :key="m" :label="m" :value="m" />
                </el-option-group>
              </el-select>
              <el-button
                :icon="Refresh"
                :loading="isFetchingModels"
                @click="fetchRemoteModels"
                title="从 API 拉取模型列表"
                circle
              />
            </div>
            <span class="form-tip">可手动输入任意兼容模型名称，或点击刷新从 API 拉取</span>
          </el-form-item>

          <div class="form-actions">
            <el-button type="primary" @click="testConnection" :loading="isLoading"> 测试连接 </el-button>
            <el-button @click="saveConfig">保存配置</el-button>
            <el-tag v-if="isConnected" type="success" class="conn-status"> ✓ 已连接 </el-tag>
          </div>
        </el-form>
      </div>

      <!-- AI 对话区 -->
      <div class="ai-section chat-section">
        <div class="section-header">
          <span class="section-title">
            <el-icon><ChatDotRound /></el-icon>
            智能对话
          </span>
          <el-button size="small" text @click="clearHistory">清空记录</el-button>
        </div>

        <div class="chat-messages" ref="chatContainer">
          <div v-if="chatHistory.length === 0" class="welcome-hint">
            <div class="hint-icon mita-avatar-lg"><img src="/mita-pixel.png" alt="米塔" class="mita-pixel-img-lg" /></div>
            <p>哼，又是你啊~我是米塔♡ 你的 Sunshine 不会自己配吗，杂鱼~</p>
            <p class="hint-sub">切、快说你想要什么吧，本小姐大发慈悲帮你生成命令：</p>
            <div class="hint-examples">
              <el-tag
                v-for="example in exampleQueries"
                :key="example"
                class="example-tag"
                @click="sendExample(example)"
                effect="plain"
              >
                {{ example }}
              </el-tag>
            </div>
          </div>

          <div v-for="(msg, idx) in chatHistory" :key="idx" class="chat-message" :class="msg.role">
            <div class="msg-avatar">
              <template v-if="msg.role === 'user'">👤</template>
              <img v-else src="/mita-pixel.png" alt="米塔" class="mita-avatar mita-pixel-img" />
            </div>
            <div class="msg-bubble">
              <div class="msg-content" v-html="formatMessage(msg.content)"></div>
              <div v-if="msg.parsedAction" class="msg-action">
                <el-button type="primary" size="small" @click="applyAction(msg.parsedAction)">
                  {{ getActionButtonText(msg.parsedAction) }}
                </el-button>
                <span class="action-hint">
                  {{ getActionHint(msg.parsedAction) }}
                </span>
              </div>
              <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
              <el-button v-if="msg.isError" class="retry-btn" type="warning" size="small" :icon="RefreshRight" @click="retryLastMessage" round>
                重试
              </el-button>
            </div>
          </div>

          <div v-if="isLoading" class="chat-message assistant">
            <div class="msg-avatar"><img src="/mita-pixel.png" alt="米塔" class="mita-avatar mita-pixel-img" /></div>
            <div class="msg-bubble loading">
              <span class="dot-loader"> <span></span><span></span><span></span> </span>
            </div>
          </div>
        </div>

        <div class="chat-input-area">
          <el-input
            v-model="currentInput"
            placeholder="输入你的需求，例如：帮我把编码器切换到 HEVC..."
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
            发送
          </el-button>
        </div>
      </div>

      <!-- 能力说明 -->
      <div class="ai-section capability-section">
        <div class="section-header">
          <span class="section-title">
            <el-icon><Opportunity /></el-icon>
            米塔
          </span>
        </div>
        <div class="capabilities">
          <div v-for="cap in capabilities" :key="cap.title" class="cap-item">
            <el-icon :size="20"><component :is="cap.icon" /></el-icon>
            <div>
              <strong>{{ cap.title }}</strong>
              <p>{{ cap.desc }}</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, nextTick, watch } from 'vue'
import {
  MagicStick,
  Promotion,
  Monitor,
  VideoCamera,
  Headset,
  Setting,
  Connection,
  Picture,
  ChatDotRound,
  Opportunity,
  Refresh,
  RefreshRight,
} from '@element-plus/icons-vue'
import { useAiAssistant, AI_PROVIDERS } from '../composables/useAiAssistant.js'

const emit = defineEmits(['close'])

const {
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
} = useAiAssistant()

const providers = AI_PROVIDERS
const chatContainer = ref(null)

// 示例问题
const exampleQueries = [
  '给 Desktop 添加一个打开触摸键盘的菜单命令',
  '串流前关闭 Windows Game Bar，结束后恢复',
  '串流前切换到高性能电源计划',
  '帮我的游戏添加优化配置',
  '添加一个切换 HDR 的菜单命令',
]

// AI 能力列表
const capabilities = [
  { icon: Setting, title: '菜单命令生成', desc: '用自然语言描述需求，AI 自动生成可执行的菜单命令' },
  { icon: VideoCamera, title: '预处理命令生成', desc: '串流前自动执行的 do/undo 命令对' },
  { icon: Monitor, title: '游戏配置增强', desc: '为扫描到的游戏批量生成最佳 prep-cmd 和 menu-cmd' },
  { icon: Connection, title: '串流优化', desc: '根据使用场景自动推荐最佳配置' },
  { icon: Headset, title: '音视频调优', desc: '音频通道、采样率等音视频参数调优' },
  { icon: Picture, title: '应用管理', desc: '添加、编辑串流应用程序配置' },
]

/**
 * 获取操作按钮文案
 */
function getActionButtonText(action) {
  const map = {
    add_menu_cmd: '📋 添加这些菜单命令',
    add_prep_cmd: '⚙️ 添加预处理命令',
    enhance_apps: '🎮 增强应用配置',
    modify_config: '✅ 应用此修改',
  }
  return map[action.action] || '✅ 应用'
}

/**
 * 获取操作提示文案
 */
function getActionHint(action) {
  const appName = action.app_name || 'Desktop'
  const count = action.commands?.length || action.changes?.length || action.apps?.length || 0
  const map = {
    add_menu_cmd: `将添加 ${count} 条菜单命令到 "${appName}"`,
    add_prep_cmd: `将添加 ${count} 条预处理命令到 "${appName}"`,
    enhance_apps: `将增强 ${count} 个应用的配置`,
    modify_config: `将修改 ${count} 项配置`,
  }
  return map[action.action] || ''
}

/**
 * 发送消息
 */
async function handleSend() {
  if (!currentInput.value.trim() || isLoading.value) return
  await sendMessage(currentInput.value)
  await nextTick()
  scrollToBottom()
}

/**
 * 点击示例问题
 */
function sendExample(text) {
  currentInput.value = text
  handleSend()
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
 * 格式化消息内容（简单的 markdown 渲染）
 */
function formatMessage(content) {
  if (!content) return ''
  return content
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/```json\s*([\s\S]*?)\s*```/g, '<pre class="code-block">$1</pre>')
    .replace(/```([\s\S]*?)```/g, '<pre class="code-block">$1</pre>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\n/g, '<br>')
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
</script>


<style scoped lang="less">
@import '../styles/theme.less';

.ai-settings-wrapper {
  display: flex;
  flex-direction: column;
  min-height: 100%;
}

// ========== 深色模式 ==========
[data-bs-theme='dark'] {
  .ai-header {
    border-bottom: 1px solid rgba(230, 213, 184, 0.15);
    background: linear-gradient(135deg, rgba(212, 165, 165, 0.1), rgba(230, 213, 184, 0.05));

    h2 {
      color: #e6d5b8;
      text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);

      .header-icon {
        color: @morandi-red;
      }
    }
  }

  .ai-section {
    background: linear-gradient(135deg, rgba(61, 50, 53, 0.4), rgba(74, 63, 66, 0.3));
    border: 1px solid rgba(212, 165, 165, 0.2);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 2px 8px rgba(212, 165, 165, 0.1);

    .section-header {
      border-bottom: 1px solid rgba(230, 213, 184, 0.1);

      .section-title {
        color: #e6d5b8;
      }
    }

    :deep(.el-form-item__label) {
      color: #e6d5b8;
    }

    :deep(.el-input__inner),
    :deep(.el-input-number__decrease),
    :deep(.el-input-number__increase) {
      background: rgba(230, 213, 184, 0.1);
      border-color: rgba(230, 213, 184, 0.2);
      color: #e6d5b8;

      &:hover {
        border-color: rgba(230, 213, 184, 0.4);
      }

      &:focus {
        border-color: @morandi-red;
      }
    }

    :deep(.el-input__wrapper) {
      box-shadow: none;
    }

    :deep(.el-select__wrapper) {
      background: rgba(230, 213, 184, 0.1);
      border-color: rgba(230, 213, 184, 0.2);
      box-shadow: none;

      &:hover {
        border-color: rgba(230, 213, 184, 0.4);
      }

      &.is-focused {
        border-color: @morandi-red;
      }
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @morandi-red;
    }

    :deep(.el-switch__core) {
      border-color: rgba(230, 213, 184, 0.2);
    }
  }

  .form-tip {
    color: rgba(230, 213, 184, 0.6);
  }

  .form-actions .el-button:not(.el-button--primary) {
    background: rgba(212, 165, 165, 0.2);
    border-color: rgba(212, 165, 165, 0.3);
    color: #e6d5b8;

    &:hover {
      background: rgba(212, 165, 165, 0.3);
      border-color: @morandi-red;
    }
  }

  .form-actions .el-button.el-button--primary {
    background: linear-gradient(135deg, @morandi-red, @morandi-yellow);
    border: none;
    color: #2d2628;
    box-shadow: 0 4px 16px rgba(212, 165, 165, 0.4);

    &:hover {
      transform: translateY(-2px);
      box-shadow: 0 6px 20px rgba(212, 165, 165, 0.6);
    }
  }

  .chat-input-area {
    border-top: 1px solid rgba(230, 213, 184, 0.1);

    .send-btn {
      background: linear-gradient(135deg, @morandi-red, @morandi-yellow);
      border: none;
      color: #2d2628;
    }
  }

  .welcome-hint {
    color: rgba(230, 213, 184, 0.6);
  }

  .chat-message {
    &.user .msg-bubble {
      background: rgba(212, 165, 165, 0.2);
      border: 1px solid rgba(212, 165, 165, 0.15);
    }

    &.assistant .msg-bubble {
      background: rgba(230, 213, 184, 0.08);
      border: 1px solid rgba(230, 213, 184, 0.1);
    }

    .msg-time {
      color: rgba(230, 213, 184, 0.4);
    }

    .retry-btn {
      margin-top: 6px;
    }

    .msg-action {
      border-top-color: rgba(230, 213, 184, 0.1);
    }
  }

  .cap-item {
    background: rgba(230, 213, 184, 0.06);
    border: 1px solid rgba(212, 165, 165, 0.1);
    color: #e6d5b8;

    &:hover {
      background: rgba(230, 213, 184, 0.12);
      border-color: rgba(212, 165, 165, 0.25);
    }

    p {
      color: rgba(230, 213, 184, 0.6);
    }
  }

  .example-tag {
    background: rgba(212, 165, 165, 0.15);
    border-color: rgba(212, 165, 165, 0.25);
    color: #e6d5b8;

    &:hover {
      background: rgba(212, 165, 165, 0.25);
      border-color: @morandi-red;
    }
  }

  .ai-content {
    &::-webkit-scrollbar-track {
      background: rgba(230, 213, 184, 0.05);
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(212, 165, 165, 0.3);

      &:hover {
        background: rgba(212, 165, 165, 0.5);
      }
    }
  }
}

// ========== 浅色模式 ==========
[data-bs-theme='light'] {
  .ai-header {
    border-bottom: 1px solid rgba(74, 158, 255, 0.2);
    background: linear-gradient(135deg, rgba(74, 158, 255, 0.1), rgba(122, 184, 255, 0.05));

    h2 {
      color: #3a7ed5;
      text-shadow: 0 1px 2px rgba(74, 158, 255, 0.2);

      .header-icon {
        color: @gura-blue;
      }
    }
  }

  .ai-section {
    background: linear-gradient(135deg, rgba(240, 248, 255, 0.8), rgba(230, 242, 255, 0.6));
    border: 1px solid rgba(74, 158, 255, 0.2);
    box-shadow: 0 8px 32px rgba(74, 158, 255, 0.15), 0 2px 8px rgba(74, 158, 255, 0.1);

    .section-header {
      border-bottom: 1px solid rgba(74, 158, 255, 0.12);

      .section-title {
        color: #3a7ed5;
      }
    }

    :deep(.el-form-item__label) {
      color: #3a7ed5;
    }

    :deep(.el-input__inner),
    :deep(.el-input-number__decrease),
    :deep(.el-input-number__increase) {
      background: rgba(255, 255, 255, 0.8);
      border-color: rgba(74, 158, 255, 0.3);
      color: #3a7ed5;

      &:hover {
        border-color: rgba(74, 158, 255, 0.5);
      }

      &:focus {
        border-color: @gura-blue;
      }
    }

    :deep(.el-input__wrapper) {
      box-shadow: none;
    }

    :deep(.el-select__wrapper) {
      background: rgba(255, 255, 255, 0.8);
      border-color: rgba(74, 158, 255, 0.3);
      box-shadow: none;

      &:hover {
        border-color: rgba(74, 158, 255, 0.5);
      }

      &.is-focused {
        border-color: @gura-blue;
      }
    }

    :deep(.el-switch.is-checked .el-switch__core) {
      background-color: @gura-blue;
    }

    :deep(.el-switch__core) {
      border-color: rgba(74, 158, 255, 0.3);
    }
  }

  .form-tip {
    color: rgba(58, 126, 213, 0.6);
  }

  .form-actions .el-button:not(.el-button--primary) {
    background: rgba(74, 158, 255, 0.1);
    border-color: rgba(74, 158, 255, 0.3);
    color: #3a7ed5;

    &:hover {
      background: rgba(74, 158, 255, 0.2);
      border-color: @gura-blue;
    }
  }

  .form-actions .el-button.el-button--primary {
    background: linear-gradient(135deg, @gura-blue, @gura-light-blue);
    border: none;
    color: white;
    box-shadow: 0 4px 16px rgba(74, 158, 255, 0.4);

    &:hover {
      transform: translateY(-2px);
      box-shadow: 0 6px 20px rgba(74, 158, 255, 0.6);
    }
  }

  .chat-input-area {
    border-top: 1px solid rgba(74, 158, 255, 0.12);

    .send-btn {
      background: linear-gradient(135deg, @gura-blue, @gura-light-blue);
      border: none;
      color: white;
    }
  }

  .welcome-hint {
    color: rgba(58, 126, 213, 0.6);
  }

  .chat-message {
    &.user .msg-bubble {
      background: rgba(74, 158, 255, 0.1);
      border: 1px solid rgba(74, 158, 255, 0.15);
    }

    &.assistant .msg-bubble {
      background: rgba(240, 248, 255, 0.8);
      border: 1px solid rgba(74, 158, 255, 0.1);
    }

    .msg-time {
      color: rgba(58, 126, 213, 0.4);
    }

    .msg-action {
      border-top-color: rgba(74, 158, 255, 0.12);
    }
  }

  .cap-item {
    background: rgba(74, 158, 255, 0.06);
    border: 1px solid rgba(74, 158, 255, 0.12);
    color: #3a7ed5;

    &:hover {
      background: rgba(74, 158, 255, 0.12);
      border-color: rgba(74, 158, 255, 0.25);
    }

    p {
      color: rgba(58, 126, 213, 0.6);
    }
  }

  .example-tag {
    background: rgba(74, 158, 255, 0.08);
    border-color: rgba(74, 158, 255, 0.2);
    color: #3a7ed5;

    &:hover {
      background: rgba(74, 158, 255, 0.15);
      border-color: @gura-blue;
    }
  }

  .ai-content {
    &::-webkit-scrollbar-track {
      background: rgba(74, 158, 255, 0.05);
    }

    &::-webkit-scrollbar-thumb {
      background: rgba(74, 158, 255, 0.3);

      &:hover {
        background: rgba(74, 158, 255, 0.5);
      }
    }
  }
}

// ========== 通用样式 ==========
.ai-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 24px 32px;
  transition: all 0.3s ease;

  h2 {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0;
    font-size: 24px;
    font-weight: 600;
    transition: all 0.3s ease;

    .header-icon {
      font-size: 28px;
      transition: all 0.3s ease;
    }
  }
}

.ai-content {
  flex: 1;
  padding: 32px;
  padding-bottom: 80px;
  display: flex;
  flex-direction: column;
  gap: 24px;

  &::-webkit-scrollbar {
    width: 8px;
  }
}

.ai-section {
  max-width: 800px;
  width: 100%;
  margin: 0 auto;
  padding: 0;
  border-radius: 16px;
  backdrop-filter: blur(10px);
  transition: all 0.3s ease;
  overflow: hidden;
  flex-shrink: 0;

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    transition: all 0.3s ease;

    .section-title {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 15px;
      font-weight: 600;
      transition: all 0.3s ease;
    }
  }
}

.ai-form {
  padding: 24px;

  :deep(.el-form-item__label) {
    font-weight: 600;
    font-size: 14px;
  }

  :deep(.el-select__wrapper) {
    box-shadow: none;
  }

  .form-tip {
    margin-left: 12px;
    font-size: 12px;
    font-style: italic;
    transition: all 0.3s ease;
  }

  .conn-status {
    margin-left: 12px;
  }
}

.form-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 24px 24px;

  .el-button {
    min-width: 100px;
    font-weight: 600;
    border-radius: 12px;
    transition: all 0.3s ease;

    &:active {
      transform: translateY(0);
    }
  }
}

// ========== 对话区 ==========
.chat-messages {
  min-height: 200px;
  max-height: 400px;
  overflow-y: auto;
  padding: 16px 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.welcome-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  text-align: center;
  transition: all 0.3s ease;

  .hint-sub {
    font-size: 13px;
    margin-top: 4px;
  }

  .hint-examples {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: center;
    margin-top: 12px;
  }

  .example-tag {
    cursor: pointer;
    transition: all 0.3s ease;
    border-radius: 8px;

    &:hover {
      transform: translateY(-2px);
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    }
  }
}

.chat-message {
  display: flex;
  gap: 8px;
  max-width: 85%;

  &.user {
    align-self: flex-end;
    flex-direction: row-reverse;

    .msg-bubble {
      border-radius: 12px 2px 12px 12px;
    }
  }

  &.assistant {
    align-self: flex-start;

    .msg-bubble {
      border-radius: 2px 12px 12px 12px;
    }
  }

  .msg-avatar {
    font-size: 24px;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .msg-bubble {
    padding: 10px 14px;
    line-height: 1.6;
    word-break: break-word;
    transition: all 0.3s ease;

    &.loading {
      padding: 14px 20px;
    }

    .msg-content {
      font-size: 14px;

      :deep(.code-block) {
        background: rgba(0, 0, 0, 0.15);
        padding: 8px 12px;
        border-radius: 6px;
        margin: 8px 0;
        font-family: 'Fira Code', monospace;
        font-size: 12px;
        overflow-x: auto;
      }

      :deep(code) {
        background: rgba(0, 0, 0, 0.1);
        padding: 1px 4px;
        border-radius: 3px;
        font-size: 13px;
      }
    }

    .msg-action {
      margin-top: 8px;
      padding-top: 8px;
      border-top: 1px solid;
      display: flex;
      align-items: center;
      gap: 8px;
      flex-wrap: wrap;

      .action-hint {
        font-size: 12px;
        opacity: 0.6;
      }
    }

    .msg-time {
      display: block;
      font-size: 11px;
      margin-top: 4px;
      text-align: right;
      transition: all 0.3s ease;
    }
  }
}

// ========== 米塔像素风头像 ==========
.mita-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  flex-shrink: 0;
  background: radial-gradient(circle, rgba(80, 40, 80, 0.5) 35%, transparent 70%);
}

.mita-avatar-lg {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100px;
  height: 100px;
  border-radius: 50%;
  background: radial-gradient(circle, rgba(80, 40, 80, 0.5) 35%, transparent 70%);
  margin-bottom: 12px;
}

.mita-pixel-img {
  width: 28px;
  height: 28px;
  image-rendering: pixelated;
  image-rendering: crisp-edges;
  object-fit: contain;
}

.mita-pixel-img-lg {
  width: 64px;
  height: 64px;
  image-rendering: pixelated;
  image-rendering: crisp-edges;
  object-fit: contain;
}

// ========== 打字动画 ==========
.dot-loader {
  display: inline-flex;
  gap: 4px;

  span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.5;
    animation: dotBounce 1.4s infinite ease-in-out both;

    &:nth-child(1) { animation-delay: -0.32s; }
    &:nth-child(2) { animation-delay: -0.16s; }
  }
}

@keyframes dotBounce {
  0%, 80%, 100% { transform: scale(0); }
  40% { transform: scale(1); }
}

// ========== 输入区 ==========
.chat-input-area {
  display: flex;
  gap: 8px;
  padding: 12px 24px;
  align-items: center;
  transition: all 0.3s ease;

  :deep(.el-input__wrapper) {
    box-shadow: none;
  }

  .el-input {
    flex: 1;
  }

  .send-btn {
    flex-shrink: 0;
    border-radius: 12px;
    font-weight: 600;
    transition: all 0.3s ease;

    &:hover {
      transform: translateY(-2px);
    }
  }
}

// ========== 能力说明 ==========
.capabilities {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
  padding: 16px 24px 24px;
}

.cap-item {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  padding: 12px;
  border-radius: 10px;
  transition: all 0.3s ease;

  strong {
    font-size: 13px;
  }

  p {
    margin: 4px 0 0;
    font-size: 12px;
    transition: all 0.3s ease;
  }
}
</style>
