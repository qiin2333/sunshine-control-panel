# 小工具开发说明

## 📋 概述

本系统提供了一个通用的工具窗口框架，可以快速开发各种小工具。所有工具都运行在全屏无边框窗口中，具有统一的 UI 风格和主题色。

## 🚀 快速开始

### 1. 创建工具组件

在 `src/renderer/tool-window/tools/` 目录下创建新的 Vue 组件：

```vue
<template>
  <div class="tool-container">
    <div class="tool-header">
      <h2>工具名称</h2>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="tool-content">
      <!-- 你的工具内容 -->
    </div>
  </div>
</template>

<script setup>
defineEmits(['close'])

// 你的业务逻辑
</script>

<style scoped>
.tool-container {
  width: 420px;
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
  padding: 30px 24px;
}
</style>
```

### 2. 注册工具

在 `src/renderer/tool-window/ToolWindow.vue` 中注册新工具：

```javascript
onMounted(async () => {
  const toolType = getToolType()
  console.log('加载工具:', toolType)

  try {
    switch (toolType) {
      case 'dpi':
        currentTool.value = defineAsyncComponent(() => import('./tools/DpiAdjusterTool.vue'))
        break
      case 'bitrate':
        currentTool.value = defineAsyncComponent(() => import('./tools/BitrateTool.vue'))
        break
      case 'your-tool':  // 添加你的工具
        currentTool.value = defineAsyncComponent(() => import('./tools/YourTool.vue'))
        break
      default:
        console.error('未知的工具类型:', toolType)
    }
  } catch (error) {
    console.error('加载工具失败:', error)
  }
})
```

### 3. 添加工具栏菜单入口

在 `src/renderer/toolbar/ToolbarApp.vue` 中添加菜单项：

```javascript
const menuItems = [
  // ... 现有菜单项
  {
    id: 'your-tool',
    label: '你的工具',
    icon: '<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="white" d="..."/></svg>',
  },
]
```

### 4. 注册后端处理

在 `src-tauri/src/main.rs` 中添加菜单处理：

```rust
fn handle_toolbar_menu_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
    match event_id {
        // ... 现有处理
        "your-tool" => {
            create_tool_window_internal(app, "your-tool");
        }
        // ...
    }
}
```

## 🎨 主题色规范

### 控制面板主题色

所有工具应使用 Sunshine 控制面板的主题色：

**Gura 蓝色主题（浅色模式）**：
- 主色：`#4a9eff`
- 次色：`#7ab8ff`
- 文本色：`white`

**莫兰迪红黄主题（深色模式）**：
- 主色：`#d4a5a5`
- 次色：`#e6d5b8`
- 文本色：`white`

### 通用样式示例

```css
/* 工具容器 */
.tool-container {
  width: 420px;
  color: white;
}

/* 按钮样式 */
.btn-primary {
  background: white;
  color: #4a9eff;
  border: none;
  border-radius: 25px;
  padding: 10px 32px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-primary:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

/* 输入框样式 */
.input-field {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: 8px;
  padding: 10px 16px;
  color: white;
  font-size: 14px;
}

.input-field::placeholder {
  color: rgba(255, 255, 255, 0.5);
}
```

## 📦 工具窗口特性

### 自动功能

- ✅ **全屏无边框窗口**：自动全屏显示
- ✅ **点击外部关闭**：点击遮罩区域关闭
- ✅ **ESC 键关闭**：按 ESC 键关闭
- ✅ **关闭按钮**：右上角 × 按钮
- ✅ **主题色**：自动应用 Gura 蓝色渐变背景

### 窗口行为

- 窗口创建时先隐藏，等待内容加载完成后再显示（避免闪白）
- 自动居中显示工具面板
- 支持响应式布局（最大宽度 90vw，最大高度 90vh）

## 🔧 调用 Tauri 命令

在工具组件中调用后端命令：

```javascript
import { invoke } from '@tauri-apps/api/core'

// 调用命令
const result = await invoke('your_command', { 
  param1: 'value1',
  param2: 'value2'
})
```

在后端注册命令（`src-tauri/src/main.rs`）：

```rust
#[tauri::command]
async fn your_command(param1: String, param2: String) -> Result<String, String> {
    // 你的业务逻辑
    Ok("成功".to_string())
}

// 在 invoke_handler 中注册
.invoke_handler(tauri::generate_handler![
    // ... 现有命令
    your_command,
])
```

## 📝 示例：BitrateTool

参考 `BitrateTool.vue` 作为简单的占位工具示例：

```vue
<template>
  <div class="tool-container">
    <div class="tool-header">
      <h2>码率调整</h2>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="tool-content">
      <div class="dev-notice">
        <div class="icon">🚧</div>
        <h3>功能开发中</h3>
        <p>码率调整功能即将推出</p>
        <p class="subtitle">敬请期待！</p>
      </div>
    </div>
  </div>
</template>
```

## 📚 完整示例：DpiAdjusterTool

`DpiAdjusterTool.vue` 展示了完整的功能实现，包括：
- 状态管理
- API 调用
- 用户交互
- 错误处理
- 加载状态

## 🎯 最佳实践

1. **组件结构**：
   - 使用 `tool-container` 作为根容器
   - 使用 `tool-header` 放置标题和关闭按钮
   - 使用 `tool-content` 放置主要内容

2. **关闭事件**：
   - 始终通过 `$emit('close')` 触发关闭
   - 不要直接调用 `window.close()`

3. **异步操作**：
   - 使用 `loading` 状态显示加载过程
   - 使用 `message` 显示操作结果

4. **响应式设计**：
   - 使用 `max-width` 和 `max-height` 确保内容适配
   - 避免固定宽度，使用百分比或 `max-width`

5. **主题一致性**：
   - 使用白色文本
   - 按钮使用白色背景 + 主题色文字
   - 保持与控制面板一致的视觉风格

## 🐛 调试技巧

1. **检查工具加载**：
   ```javascript
   console.log('加载工具:', toolType)
   ```

2. **检查窗口创建**：
   后端日志会显示：
   ```
   🔧 创建工具窗口 URL: tool-window/index.html?tool=your-tool
   ```

3. **检查组件挂载**：
   在组件中添加：
   ```javascript
   onMounted(() => {
     console.log('工具组件已挂载')
   })
   ```

## 📖 相关文件

- **工具窗口容器**：`src/renderer/tool-window/ToolWindow.vue`
- **工具组件目录**：`src/renderer/tool-window/tools/`
- **工具栏菜单**：`src/renderer/toolbar/ToolbarApp.vue`
- **后端处理**：`src-tauri/src/main.rs`
- **工具窗口创建**：`create_tool_window_internal()` 函数

## 🎉 开始开发

1. 复制 `BitrateTool.vue` 作为模板
2. 修改组件名称和内容
3. 在 `ToolWindow.vue` 中注册
4. 在 `ToolbarApp.vue` 中添加菜单项
5. 在 `main.rs` 中添加处理逻辑
6. 测试你的工具！

祝你开发愉快！🚀

