<template>
  <div id="toolbar-container" @click.self="handleOutsideClick">
    <!-- 气泡菜单 -->
    <transition name="bubble">
      <div v-if="menuVisible" class="bubble-menu" @click.stop>
        <div v-for="(item, index) in menuItems" :key="item.id" class="bubble-wrapper" :style="getBubbleStyle(index)">
          <div
            class="bubble-item"
            :class="{ danger: item.danger }"
            @click="handleMenuItem(item.id)"
            :title="item.label"
          >
            <div class="bubble-icon" v-html="item.icon"></div>
          </div>
        </div>
      </div>
    </transition>

    <!-- 中心工具栏图标 -->
    <div
      class="toolbar-icon"
      :class="{ active: menuVisible }"
      @mousedown="handleMouseDown"
      @mouseup="handleMouseUp"
      @click.stop="toggleMenu"
      @contextmenu.prevent="toggleMenu"
    >
      <img src="/toolbar-icon.png" class="icon-image" alt="工具栏" />
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

const menuVisible = ref(false)

const menuItems = [
  {
    id: 'main',
    label: '控制面板',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/></svg>',
  },
  {
    id: 'vdd',
    label: '虚拟显示器',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.1-.9-2-2-2H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z"/></svg>',
  },
  {
    id: 'dpi',
    label: '调整 DPI',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M3 17v2h6v-2H3zM3 5v2h10V5H3zm10 16v-2h8v-2h-8v-2h-2v6h2zM7 9v2H3v2h4v2h2V9H7zm14 4v-2H11v2h10zm-6-4h2V7h4V5h-4V3h-2v6z"/></svg>',
  },
  {
    id: 'bitrate',
    label: '码率调整',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14zm2.5-4h-2v2H9v-2H7V9h2V7h1v2h2v1z"/></svg>',
  },
  {
    id: 'close',
    label: '关闭',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/></svg>',
    danger: true,
  },
]

let dragTimeout = null
let isDragging = false

const handleMouseDown = (e) => {
  // 右键不处理
  if (e.button === 2) {
    return
  }

  // 左键长按才拖动
  if (e.button === 0) {
    isDragging = false
    dragTimeout = setTimeout(() => {
      isDragging = true
      const window = getCurrentWindow()
      window.startDragging()
    }, 200) // 200ms 后开始拖动
  }
}

const handleMouseUp = () => {
  if (dragTimeout) {
    clearTimeout(dragTimeout)
    dragTimeout = null
  }
}

const toggleMenu = (e) => {
  if (e) {
    e.preventDefault()
    e.stopPropagation()
  }

  // 如果正在拖动，不切换菜单
  if (isDragging) {
    isDragging = false
    return
  }

  console.log('切换菜单，当前状态:', menuVisible.value)
  menuVisible.value = !menuVisible.value
  console.log('新状态:', menuVisible.value)
}

const handleOutsideClick = () => {
  // 点击容器空白区域时关闭菜单
  if (menuVisible.value) {
    menuVisible.value = false
  }
}

const handleMenuItem = async (action) => {
  menuVisible.value = false

  try {
    await invoke('handle_toolbar_menu_action', { action })
  } catch (error) {
    console.error('菜单操作失败:', error)
  }
}

// 计算气泡位置（圆形分布 + 旋转动画）
const getBubbleStyle = (index) => {
  const totalItems = menuItems.length
  const radius = 70 // 气泡圆的半径（紧凑）
  const startAngle = -90 // 从顶部开始
  const angleStep = 360 / totalItems
  const angle = startAngle + angleStep * index
  const radian = (angle * Math.PI) / 180

  const x = Math.cos(radian) * radius
  const y = Math.sin(radian) * radius

  return {
    transform: `translate(${x}px, ${y}px)`,
    transitionDelay: `${index * 50}ms`,
    animationDelay: `${index * 50}ms`,
  }
}
</script>

<style scoped>
#toolbar-container {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  box-sizing: border-box;
  transform: translateZ(0);
  -webkit-font-smoothing: antialiased;
}

/* 气泡菜单容器 */
.bubble-menu {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 50;
  will-change: transform;
  transform: translateZ(0);
}

/* 气泡包裹层（负责定位） */
.bubble-wrapper {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 48px;
  height: 48px;
  margin-left: -24px;
  margin-top: -24px;
  pointer-events: all;
  will-change: transform, margin-top;
  transform: translateZ(0);
  backface-visibility: hidden;
}

/* 气泡项（负责入场动画和样式） */
.bubble-item {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: linear-gradient(
    135deg,
    rgba(255, 182, 193, 0.95) 0%,
    /* 粉色 */ rgba(255, 160, 220, 0.95) 50%,
    /* 淡紫粉 */ rgba(186, 148, 255, 0.95) 100% /* 淡紫色 */
  );
  backdrop-filter: blur(15px);
  box-shadow: 0 4px 20px rgba(255, 182, 193, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  animation: bubbleIn 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
  position: relative;
  will-change: transform, opacity, box-shadow;
  transform: translateZ(0);
  backface-visibility: hidden;
  -webkit-font-smoothing: antialiased;
}

/* 为每个气泡添加不同的可爱颜色和动画延迟 */
.bubble-wrapper:nth-child(1) {
  animation-delay: 0.1s;
}

.bubble-wrapper:nth-child(1) .bubble-item {
  background: linear-gradient(135deg, rgba(255, 182, 193, 0.95) 0%, rgba(255, 192, 203, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(255, 182, 193, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-wrapper:nth-child(2) {
  animation-delay: 0.6s;
}

.bubble-wrapper:nth-child(2) .bubble-item {
  background: linear-gradient(135deg, rgba(173, 216, 230, 0.95) 0%, rgba(135, 206, 250, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(173, 216, 230, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-wrapper:nth-child(3) {
  animation-delay: 1.2s;
}

.bubble-wrapper:nth-child(3) .bubble-item {
  background: linear-gradient(135deg, rgba(221, 160, 221, 0.95) 0%, rgba(218, 112, 214, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(221, 160, 221, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-wrapper:nth-child(4) {
  animation-delay: 1.8s;
}

.bubble-wrapper:nth-child(4) .bubble-item {
  background: linear-gradient(135deg, rgba(255, 193, 7, 0.95) 0%, rgba(255, 152, 0, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(255, 193, 7, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-wrapper:nth-child(5) {
  animation-delay: 2.4s;
}

.bubble-item:hover {
  box-shadow: 0 8px 35px rgba(255, 182, 193, 0.9), 0 0 0 4px rgba(255, 255, 255, 0.6),
    inset 0 3px 10px rgba(255, 255, 255, 0.5);
  z-index: 10;
  transform: scale(1.1) translateZ(0);
}

.bubble-item:hover .bubble-icon {
  transform: scale(1.2) rotate(15deg);
}

.bubble-item.danger {
  background: linear-gradient(135deg, rgba(255, 182, 193, 0.95) 0%, rgba(255, 150, 150, 0.95) 100%);
  box-shadow: 0 4px 20px rgba(255, 150, 150, 0.6), 0 0 0 3px rgba(255, 255, 255, 0.4),
    inset 0 2px 8px rgba(255, 255, 255, 0.3);
}

.bubble-item.danger:hover {
  box-shadow: 0 8px 35px rgba(255, 150, 150, 0.9), 0 0 0 4px rgba(255, 255, 255, 0.6),
    inset 0 3px 10px rgba(255, 255, 255, 0.5);
}

.bubble-icon {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  will-change: transform;
  backface-visibility: hidden;
}

.bubble-icon svg {
  width: 100%;
  height: 100%;
}

/* 中心工具栏图标 */
.toolbar-icon {
  width: 52px;
  height: 52px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  -webkit-app-region: no-drag;
  background: transparent;
  border: none;
  padding: 0;
  margin: 0;
  border-radius: 50%;
  animation: float 3s ease-in-out infinite;
  filter: drop-shadow(0 0 8px rgba(255, 182, 193, 0.4)) 
          drop-shadow(0 0 16px rgba(221, 160, 221, 0.2));
  transition: all 0.4s cubic-bezier(0.34, 1.56, 0.64, 1);
  position: relative;
  z-index: 100;
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
  -webkit-font-smoothing: antialiased;
}

.toolbar-icon:hover {
  animation: pulse 1.5s ease-in-out infinite;
  filter: drop-shadow(0 0 12px rgba(255, 182, 193, 0.6)) 
          drop-shadow(0 0 24px rgba(221, 160, 221, 0.3));
}

.toolbar-icon.active {
  transform: scale(1.15) translateZ(0);
  filter: drop-shadow(0 0 16px rgba(123, 80, 87, 0.8)) 
          drop-shadow(0 0 32px rgba(221, 160, 221, 0.4));
}

.icon-image {
  width: 100%;
  height: 100%;
  object-fit: contain;
  pointer-events: none;
  display: block;
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
}

/* 气泡入场动画（带旋转，使用 3D 加速） */
@keyframes bubbleIn {
  0% {
    opacity: 0;
    transform: scale(0) rotate(-180deg) translate3d(0, 0, 0);
  }
  70% {
    transform: scale(1.1) rotate(10deg) translate3d(0, 0, 0);
  }
  100% {
    opacity: 1;
    transform: scale(1) rotate(0deg) translate3d(0, 0, 0);
  }
}

/* 气泡浮动（入场后） */
@keyframes bubbleFloat2 {
  0%,
  100% {
    transform: scale(1) rotate(0deg) translate3d(0, 0, 0);
  }
  50% {
    transform: scale(1) rotate(0deg) translate3d(0, -5px, 0);
  }
}

.bubble-item .bubble-icon {
  animation: iconScale 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
  will-change: transform;
  transform: translateZ(0);
  backface-visibility: hidden;
}

@keyframes iconScale {
  0% {
    transform: scale(0) rotate(-90deg) translateZ(0);
  }
  60% {
    transform: scale(1.2) rotate(10deg) translateZ(0);
  }
  100% {
    transform: scale(1) rotate(0deg) translateZ(0);
  }
}

/* 可爱的浮动动画（使用 margin 避免 transform 冲突） */
@keyframes bubbleFloat {
  0%,
  100% {
    margin-top: -24px;
  }
  50% {
    margin-top: -28px;
  }
}

/* 摆动动画（结合缩放，使用 3D 加速） */
@keyframes wiggle {
  0%,
  100% {
    transform: scale(1.15) rotate(-5deg) translateZ(0);
  }
  50% {
    transform: scale(1.15) rotate(5deg) translateZ(0);
  }
}

/* 气泡过渡动画 */
.bubble-enter-active {
  transition: opacity 0.3s;
}

.bubble-leave-active {
  transition: opacity 0.2s;
}

.bubble-enter-from,
.bubble-leave-to {
  opacity: 0;
}

.bubble-enter-from .bubble-item {
  transform: scale(0);
}

/* 浮动跳动动画（使用 translate3d 硬件加速） */
@keyframes float {
  0%,
  100% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  50% {
    transform: translate3d(0, -10px, 0) scale(1);
  }
}

/* 脉冲光晕动画（使用 translate3d 硬件加速） */
@keyframes pulse {
  0%,
  100% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  25% {
    transform: translate3d(0, -5px, 0) scale(1.05);
  }
  50% {
    transform: translate3d(0, -10px, 0) scale(1.1);
  }
  75% {
    transform: translate3d(0, -5px, 0) scale(1.05);
  }
}

/* 心跳动画（使用 translate3d 硬件加速） */
@keyframes heartbeat {
  0%,
  100% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  10% {
    transform: translate3d(0, -2px, 0) scale(1.1);
  }
  20% {
    transform: translate3d(0, 0, 0) scale(1);
  }
  30% {
    transform: translate3d(0, -3px, 0) scale(1.15);
  }
  40% {
    transform: translate3d(0, 0, 0) scale(1);
  }
}
</style>
