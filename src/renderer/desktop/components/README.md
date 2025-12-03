# Desktop UI Components

专门的桌面应用 UI 组件库，用于构建现代化的桌面应用界面。

## 安装/导入

```javascript
// 导入所有组件
import { DesktopWindow, TitleBar, DesktopSidebar, DesktopCard, DesktopGrid } from './components'

// 或单独导入
import DesktopWindow from './components/DesktopWindow.vue'
```

## 组件列表

### 1. DesktopWindow - 桌面窗口容器

桌面应用的主容器组件，提供窗口结构、背景效果和布局管理。

**Props:**
- `title` (String): 窗口标题
- `icon` (String): 窗口图标路径
- `showTitleBar` (Boolean): 是否显示标题栏，默认 `true`
- `hasSidebar` (Boolean): 是否有侧边栏，默认 `false`
- `theme` (String): 主题样式，`dark` 或 `light`，默认 `dark`

**Slots:**
- `titlebar`: 自定义标题栏
- `sidebar`: 侧边栏内容
- `default`: 主内容区域
- `footer`: 页脚内容

**使用示例:**
```vue
<template>
  <DesktopWindow title="我的应用" :has-sidebar="true">
    <template #sidebar>
      <DesktopSidebar :items="navItems" />
    </template>
    
    <template #default>
      <div>主内容</div>
    </template>
  </DesktopWindow>
</template>
```

---

### 2. TitleBar - 标题栏组件

自定义窗口标题栏，支持拖拽、图标和窗口控制按钮。

**Props:**
- `title` (String): 标题文本
- `icon` (String): 图标路径
- `draggable` (Boolean): 是否可拖拽，默认 `true`
- `showControls` (Boolean): 是否显示窗口控制按钮，默认 `true`

**Slots:**
- `left`: 左侧内容
- `center`: 中间内容
- `right`: 右侧内容

**使用示例:**
```vue
<TitleBar title="应用标题" icon="/icon.png">
  <template #left>
    <span>自定义左侧内容</span>
  </template>
</TitleBar>
```

---

### 3. WindowControls - 窗口控制按钮

窗口的最小化、最大化、关闭按钮组件。

**Props:**
- `disabled` (Boolean): 是否禁用，默认 `false`

**Events:**
- `minimize`: 点击最小化
- `maximize`: 点击最大化
- `close`: 点击关闭

**使用示例:**
```vue
<WindowControls @close="handleClose" />
```

---

### 4. DesktopSidebar - 侧边栏导航

桌面应用的侧边栏导航组件，支持图标、标签、徽章等。

**Props:**
- `items` (Array): 导航项数组
  - `id` (String): 唯一标识
  - `label` (String): 标签文本
  - `icon` (Component): 图标组件
  - `badge` (String): 徽章文本（可选）
  - `disabled` (Boolean): 是否禁用（可选）
- `bottomItems` (Array): 底部导航项数组（格式同 items）
- `activeItem` (String): 当前激活的项 ID
- `collapsed` (Boolean): 是否收起，默认 `false`
- `collapsible` (Boolean): 是否可收起，默认 `false`
- `showDivider` (Boolean): 是否显示分隔线，默认 `true`

**Events:**
- `item-click`: 点击导航项时触发
- `update:activeItem`: 激活项变化时触发
- `update:collapsed`: 收起状态变化时触发

**使用示例:**
```vue
<DesktopSidebar
  :items="navItems"
  :bottom-items="bottomItems"
  :active-item="activeNav"
  @item-click="handleNavClick"
/>
```

**导航项格式:**
```javascript
const navItems = [
  {
    id: 'dashboard',
    label: '仪表盘',
    icon: IconDashboard,
    badge: '3', // 可选
    disabled: false // 可选
  }
]
```

---

### 5. DesktopCard - 桌面卡片

桌面应用的卡片容器组件，支持多种样式和交互。

**Props:**
- `title` (String): 卡片标题
- `icon` (Component): 标题图标
- `variant` (String): 样式变体，`default` | `primary` | `secondary` | `success` | `warning` | `danger`，默认 `default`
- `hoverable` (Boolean): 是否显示悬停效果，默认 `false`
- `clickable` (Boolean): 是否可点击，默认 `false`
- `showHeader` (Boolean): 是否显示头部，默认 `true`
- `noPadding` (Boolean): 是否移除内边距，默认 `false`

**Slots:**
- `title`: 自定义标题（会覆盖 title prop）
- `actions`: 头部操作按钮
- `default`: 卡片内容
- `footer`: 卡片底部

**Events:**
- `click`: 点击卡片时触发（需要设置 `clickable` 为 `true`）

**使用示例:**
```vue
<DesktopCard 
  title="系统信息" 
  :icon="IconInfo"
  variant="primary"
  hoverable
>
  <template #actions>
    <button>操作</button>
  </template>
  
  <div>卡片内容</div>
  
  <template #footer>
    <button>确定</button>
  </template>
</DesktopCard>
```

---

### 6. DesktopGrid - 网格布局

响应式网格布局组件，用于排列多个卡片或其他元素。

**Props:**
- `cols` (Number): 列数，1-6，默认 `2`
- `gap` (String): 间距，`xs` | `sm` | `md` | `lg` | `xl`，默认 `md`
- `responsive` (Boolean): 是否响应式，默认 `true`

**使用示例:**
```vue
<DesktopGrid cols="4" gap="lg">
  <DesktopCard v-for="item in items" :key="item.id">
    {{ item.content }}
  </DesktopCard>
</DesktopGrid>
```

---

## Composables

### useWindowControls

窗口控制 composable，提供窗口操作功能。

**返回值:**
```javascript
{
  tauriWindow,      // Tauri 窗口对象
  isMaximized,      // 是否最大化
  isMinimized,      // 是否最小化
  isFocused,        // 是否聚焦
  minimize,         // 最小化函数
  maximize,         // 最大化函数
  unmaximize,       // 还原函数
  toggleMaximize,   // 切换最大化状态
  close,            // 关闭窗口
  show,             // 显示窗口
  hide,             // 隐藏窗口
  setFocus,         // 聚焦窗口
  center,           // 居中窗口
  setSize,          // 设置窗口大小
  getSize           // 获取窗口大小
}
```

**使用示例:**
```vue
<script setup>
import { useWindowControls } from '../composables'

const { isMaximized, minimize, maximize, close } = useWindowControls()
</script>
```

---

## 完整使用示例

```vue
<template>
  <DesktopWindow title="我的桌面应用" :has-sidebar="true">
    <template #sidebar>
      <DesktopSidebar
        :items="navItems"
        :active-item="activeNav"
        @item-click="handleNavClick"
      />
    </template>

    <template #default>
      <div class="page-container">
        <DesktopGrid cols="3" gap="md">
          <DesktopCard 
            v-for="card in cards" 
            :key="card.id"
            :title="card.title"
            :variant="card.variant"
            hoverable
          >
            {{ card.content }}
          </DesktopCard>
        </DesktopGrid>
      </div>
    </template>
  </DesktopWindow>
</template>

<script setup>
import { ref } from 'vue'
import { DesktopWindow, DesktopSidebar, DesktopCard, DesktopGrid } from './components'

const activeNav = ref('dashboard')
const navItems = [/* ... */]
const cards = [/* ... */]

function handleNavClick(item) {
  activeNav.value = item.id
}
</script>
```

---

## 样式定制

所有组件都支持通过 CSS 变量进行主题定制：

```less
:root {
  --desktop-bg-primary: #0f0f23;
  --desktop-bg-secondary: #1a1a2e;
  --desktop-accent-cyan: #00fff5;
  --desktop-border-color: rgba(0, 255, 245, 0.2);
}
```

---

## 最佳实践

1. **使用 DesktopWindow 作为根容器**：提供完整的桌面应用布局
2. **组合使用组件**：DesktopSidebar + DesktopCard + DesktopGrid
3. **利用 Composables**：使用 `useWindowControls` 管理窗口状态
4. **响应式设计**：使用 DesktopGrid 的响应式特性适配不同屏幕
5. **主题一致性**：使用统一的 variant 和主题变量

---

## 注意事项

- 组件使用 `-webkit-app-region: drag` 实现窗口拖拽，需要合理设置 `no-drag` 区域
- 窗口控制功能仅在 Tauri 环境中可用，浏览器环境会优雅降级
- 某些组件依赖特定的 CSS 变量，确保已引入相应的样式文件
