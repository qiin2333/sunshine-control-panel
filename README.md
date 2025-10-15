# Sunshine Control Panel (Tauri)

基于 Tauri 2.8.4 的 Sunshine 控制面板 GUI。

## 前置要求

- Node.js 和 npm
- Rust 和 Cargo (用于 Tauri)
- Windows SDK (Windows)

## 开发

```bash
# 安装依赖
npm install

# 启动开发服务器
npm run dev

# 仅启动前端开发服务器
npm run dev:renderer
```

## 构建

```bash
# 构建渲染进程
npm run build:renderer

# 构建完整应用
npm run build

# Windows 构建
npm run build:win
```

## 项目结构

```
src-tauri/           # Tauri 后端 (Rust)
  ├── src/
  │   ├── main.rs            # 主入口
  │   ├── proxy_server.rs    # 本地代理服务器
  │   ├── sunshine.rs        # Sunshine 相关功能
  │   ├── vdd.rs            # VDD 驱动管理
  │   ├── utils.rs          # 工具函数
  │   ├── system.rs         # 系统信息
  │   └── fs_utils.rs       # 文件系统工具
  ├── inject-script.js      # 注入到 Sunshine Web UI 的脚本
  └── Cargo.toml            # Rust 依赖配置

src/renderer/        # 前端 (Vue 3)
  ├── components/           # Vue 组件
  │   ├── SidebarMenu.vue   # 侧边栏菜单
  │   ├── SunshineFrame.vue # Sunshine Web UI iframe
  │   └── ...
  ├── styles/              # Less 样式
  └── ...

vite.config.js       # Vite 构建配置
package.json         # NPM 依赖配置
```

## 特性

- 🎨 现代化 UI，基于 Element Plus
- 🌐 本地代理服务器，解决跨域问题
- 🎭 主题同步 (亮色/暗色)
- 🖼️ 拖放背景图片
- 📊 VDD 驱动管理
- 🔧 Sunshine 配置管理
- 🪟 Windows 风格窗口控件

## 技术栈

- **前端**: Vue 3 + Element Plus + Less
- **后端**: Rust + Tauri 2.8.4
- **HTTP**: Axum (代理服务器)
- **构建**: Vite

## 集成到 Sunshine

编译后的 GUI 会自动安装到 Sunshine 的 `assets/gui` 目录：

```
Sunshine/
  └── assets/
      └── gui/
          └── sunshine-gui.exe
```

## 注意事项

- Tauri GUI 是可选组件，不影响 Sunshine 核心功能
- 需要 Rust 工具链才能构建 Tauri 应用
- 首次构建会下载并编译 Rust 依赖，需要较长时间
