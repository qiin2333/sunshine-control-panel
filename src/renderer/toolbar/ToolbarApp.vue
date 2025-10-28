<template>
  <div id="toolbar-container">
    <!-- 工具栏图标 -->
    <div 
      class="toolbar-icon"
      @mousedown="handleMouseDown"
      @contextmenu.prevent="showMenu"
    >
      <img src="/toolbar-icon.png" class="icon-image" alt="工具栏" />
    </div>
  </div>
</template>

<script setup>
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

let isDragging = false;

const handleMouseDown = (e) => {
  // 右键不处理
  if (e.button === 2) {
    return;
  }
  
  // 左键开始拖动
  if (e.button === 0) {
    isDragging = true;
    const window = getCurrentWindow();
    window.startDragging();
  }
};

const showMenu = async () => {
  try {
    const window = getCurrentWindow();
    await invoke('show_toolbar_menu', { window });
  } catch (error) {
    console.error('显示菜单失败:', error);
  }
};
</script>

<style scoped>
#toolbar-container {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  padding: 20px;
  box-sizing: border-box;
}

/* 工具栏图标 */
.toolbar-icon {
  width: 60px;
  height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: move;
  -webkit-app-region: no-drag;
  background: transparent;
  border: none;
  padding: 0;
  margin: 0;
  border-radius: 50%;
  animation: float 3s ease-in-out infinite;
  filter: drop-shadow(0 0 10px rgba(102, 126, 234, 0.5)) drop-shadow(0 0 20px rgba(118, 75, 162, 0.3));
}

.toolbar-icon:hover {
  animation: float 3s ease-in-out infinite, pulse 1.5s ease-in-out infinite;
  filter: drop-shadow(0 0 18px rgba(102, 126, 234, 0.9)) 
         drop-shadow(0 0 30px rgba(118, 75, 162, 0.7))
         drop-shadow(0 0 40px rgba(102, 126, 234, 0.5));
}

.toolbar-icon:active {
  animation: none;
  transform: scale(0.9);
  filter: drop-shadow(0 0 6px rgba(102, 126, 234, 0.4));
}

.icon-image {
  width: 100%;
  height: 100%;
  object-fit: contain;
  pointer-events: none;
  display: block;
}

/* 浮动跳动动画 */
@keyframes float {
  0%, 100% {
    transform: translateY(0px);
  }
  50% {
    transform: translateY(-8px);
  }
}

/* 脉冲光晕动画 */
@keyframes pulse {
  0%, 100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.08);
  }
}
</style>
