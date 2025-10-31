<template>
  <div class="tool-container">
    <div class="tool-header">
      <h2>Moonlight 串流快捷键手册</h2>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="tool-content" v-html="renderedMarkdown"></div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import MarkdownIt from 'markdown-it'
import shortcutsMd from './shortcuts.md?raw'

defineEmits(['close'])

const renderedMarkdown = ref('')

// 初始化 markdown-it
const md = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
})

// 自定义渲染规则：code_inline -> kbd
const defaultCodeInlineRender =
  md.renderer.rules.code_inline || ((tokens, idx, options, env, self) => self.renderToken(tokens, idx, options))
md.renderer.rules.code_inline = (tokens, idx, options, env, self) => {
  const token = tokens[idx]
  return `<kbd>${token.content}</kbd>`
}

// 自定义渲染规则：blockquote -> note
const defaultBlockquoteOpen =
  md.renderer.rules.blockquote_open || ((tokens, idx, options, env, self) => self.renderToken(tokens, idx, options))
md.renderer.rules.blockquote_open = () => {
  return '<div class="note">'
}
md.renderer.rules.blockquote_close = () => {
  return '</div>'
}

onMounted(() => {
  renderedMarkdown.value = md.render(shortcutsMd)
})
</script>

<style scoped>
.tool-container {
  width: 680px;
  max-height: 85vh;
  color: white;
}

.tool-header {
  padding: 16px 24px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  position: relative;
}

.tool-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  text-align: center;
}

.close-btn {
  position: absolute;
  top: 12px;
  right: 16px;
  width: 32px;
  height: 32px;
  border: none;
  background: rgba(255, 255, 255, 0.2);
  color: white;
  font-size: 28px;
  line-height: 1;
  border-radius: 50%;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background: rgba(255, 255, 255, 0.3);
  transform: rotate(90deg);
}

.tool-content {
  padding: 20px 28px;
  max-height: 70vh;
  overflow-y: auto;
}

/* Markdown 渲染样式 - 白色主题 */
.tool-content :deep(h1) {
  font-size: 22px;
  color: white;
  margin-bottom: 16px;
  display: none;
}

.tool-content :deep(h2) {
  font-size: 18px;
  color: white;
  margin: 24px 0 12px 0;
  padding-bottom: 6px;
  border-bottom: 2px solid rgba(255, 255, 255, 0.3);
  font-weight: 600;
}

.tool-content :deep(h3) {
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
  margin: 16px 0 10px 0;
}

.tool-content :deep(ul) {
  list-style: none;
  padding: 0;
  margin: 0 0 14px 0;
}

.tool-content :deep(li) {
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(5px);
  border-radius: 6px;
  margin-bottom: 5px;
  transition: all 0.2s;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  font-size: 13px;
}

.tool-content :deep(li:hover) {
  background: rgba(255, 255, 255, 0.15);
  transform: translateX(3px);
}

.tool-content :deep(kbd) {
  display: inline-block;
  padding: 2px 7px;
  background: rgba(255, 255, 255, 0.25);
  border: 1px solid rgba(255, 255, 255, 0.4);
  border-radius: 4px;
  font-family: 'Consolas', 'Courier New', monospace;
  font-size: 11px;
  font-weight: 600;
  color: white;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.15);
  white-space: nowrap;
}

.tool-content :deep(li code:not(kbd)) {
  background: transparent;
  padding: 0;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.95);
  flex: 1;
}

.tool-content :deep(.note) {
  background: rgba(255, 193, 7, 0.2);
  border-left: 3px solid #ffc107;
  padding: 10px 12px;
  border-radius: 4px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.95);
  margin: 10px 0;
}

.tool-content :deep(.note p) {
  margin: 0;
}

.tool-content :deep(hr) {
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.2);
  margin: 20px 0;
}

.tool-content :deep(a) {
  color: #a8d5ff;
  text-decoration: none;
}

.tool-content :deep(a:hover) {
  text-decoration: underline;
}

.tool-content :deep(p) {
  margin: 6px 0;
  line-height: 1.5;
  font-size: 13px;
}

/* 滚动条样式 */
.tool-content::-webkit-scrollbar {
  width: 6px;
}

.tool-content::-webkit-scrollbar-track {
  background: rgba(255, 255, 255, 0.05);
}

.tool-content::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.3);
  border-radius: 3px;
}

.tool-content::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.4);
}
</style>
