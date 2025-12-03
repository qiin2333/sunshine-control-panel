# Desktop UI Framework

模块化的桌面应用 UI 组件库，专门为 Tauri + Vue 3 桌面应用设计。

## 📁 目录结构

```
desktop/
├── components/           # 桌面 UI 组件库
│   ├── DesktopWindow.vue    # 窗口容器
│   ├── TitleBar.vue         # 标题栏
│   ├── WindowControls.vue   # 窗口控制按钮
│   ├── DesktopSidebar.vue   # 侧边栏导航
│   ├── DesktopCard.vue      # 卡片组件
│   ├── DesktopGrid.vue      # 网格布局
│   ├── index.js             # 组件导出
│   └── README.md            # 组件文档
├── composables/          # 组合式函数
│   ├── useWindowControls.js  # 窗口控制
│   └── index.js             # Composables 导出
├── views/                # 视图页面
│   ├── DashboardView.vue
│   ├── DevicesView.vue
│   ├── StreamView.vue
│   ├── ToolsView.vue
│   └── SettingsView.vue
├── icons/                # 图标组件
├── DesktopApp.vue        # 主应用组件
├── desktop.less          # 桌面样式
├── main.js               # 入口文件
└── index.html            # HTML 模板
```

## 🚀 快速开始

### 基础使用

```vue
<template>
  <DesktopWindow title="我的应用" :has-sidebar="true">
    <template #sidebar>
      <DesktopSidebar :items="navItems" :active-item="activeNav" />
    </template>
    
    <template #default>
      <DesktopCard title="欢迎">
        欢迎使用桌面 UI 组件库！
      </DesktopCard>
    </template>
  </DesktopWindow>
</template>

<script setup>
import { ref } from 'vue'
import { DesktopWindow, DesktopSidebar, DesktopCard } from './components'

const activeNav = ref('home')
const navItems = [
  { id: 'home', label: '首页', icon: IconHome }
]
</script>
```

## 📦 组件列表

### 核心组件

| 组件 | 说明 | 文档 |
|------|------|------|
| `DesktopWindow` | 桌面窗口容器 | [文档](./components/README.md#1-desktopwindow) |
| `TitleBar` | 自定义标题栏 | [文档](./components/README.md#2-titlebar) |
| `WindowControls` | 窗口控制按钮 | [文档](./components/README.md#3-windowcontrols) |
| `DesktopSidebar` | 侧边栏导航 | [文档](./components/README.md#4-desktopsidebar) |
| `DesktopCard` | 桌面卡片 | [文档](./components/README.md#5-desktopcard) |
| `DesktopGrid` | 网格布局 | [文档](./components/README.md#6-desktopgrid) |

### Composables

| Composable | 说明 | 文档 |
|------------|------|------|
| `useWindowControls` | 窗口控制功能 | [文档](./components/README.md#usewindowcontrols) |

## 🎨 特性

- ✅ **模块化设计** - 组件独立，易于复用和维护
- ✅ **TypeScript 友好** - 完整的类型支持
- ✅ **响应式布局** - 自适应不同屏幕尺寸
- ✅ **主题定制** - 支持深色/浅色主题
- ✅ **Tauri 集成** - 原生窗口控制无缝集成
- ✅ **可访问性** - 支持键盘导航和屏幕阅读器

## 📖 文档

详细的组件文档请参考：
- [组件文档](./components/README.md)
- [使用示例](./components/README.md#完整使用示例)

## 🔧 开发

### 添加新组件

1. 在 `components/` 目录创建组件文件
2. 在 `components/index.js` 中导出
3. 在 `components/README.md` 中添加文档

### 添加新 Composable

1. 在 `composables/` 目录创建文件
2. 在 `composables/index.js` 中导出
3. 添加使用示例

## 📝 更新日志

### v1.0.0 (当前版本)
- ✨ 初始版本
- ✨ 6 个核心组件
- ✨ 窗口控制 composable
- ✨ 完整的文档

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

与项目主许可证相同

