# 桌宠与桌面 UI 逻辑与安全说明

本文档说明 Sunshine Control Panel（sunshine-gui）中「桌宠」与「桌面 UI」的完整逻辑、资源加载架构及安全设计。

---

## 一、名词与概念区分

| 概念 | 含义 | 对应入口 |
|------|------|----------|
| **桌宠** | 在 `apps.json` 的「Desktop」应用的 `menu-cmd` 里配置的菜单项名称 | 串流会话内菜单点击后执行 `.\assets\gui\sunshine-gui.exe --toolbar` |
| **工具栏** | 小窗口、置顶、透明、右下角，加载 `toolbar/index.html` | 参数 `--toolbar` / `-t`，或托盘「显示工具栏」、全局快捷键 |
| **桌面 UI** | 全屏/大窗口桌面风格界面，加载 `desktop/index.html` | 参数 `--desktop` / `-d`，或托盘「打开桌面 UI」**仅 Debug 构建** |

> **重要**：配置里的「桌宠」实际启动的是**工具栏**（`--toolbar`），不是桌面 UI（`--desktop`）。

---

## 二、启动与窗口管理

### 2.1 启动参数解析（app.rs）

入口 `setup_application()` 读取 `std::env::args()`：

| 参数 | 行为 |
|------|------|
| `--toolbar` / `-t` | 延迟 ~1s 后创建工具栏窗口 |
| `--desktop` / `-d` | 创建桌面 UI 窗口 + 隐藏主窗口 |
| `--url=...` 含 `/pin` | 打开 PIN 输入窗口 |
| 无参数 | 创建主窗口（常规启动） |

### 2.2 单实例机制（main.rs + app.rs）

- 使用 `tauri_plugin_single_instance`，第二个进程启动时把参数传给已运行实例的 `handle_single_instance()`。
- 工具栏有 toggle 行为（有则关，无则创建），桌面 UI 有则激活。

### 2.3 窗口单例保证

- **工具栏**：`create_toolbar_window_internal()` 开头检查 `app.get_webview_window(TOOLBAR_WINDOW_ID).is_some()`，已存在则直接返回。
- **桌面 UI**：`open_desktop_window()` 先查已有窗口，存在则激活。

---

## 三、桌宠资源加载架构

### 3.1 精灵图加载（PixiJS 动画）

工具栏使用 PixiJS 渲染 4×4 共 16 帧的精灵图动画。精灵图来源于 CDN：`https://xxxx.com/toolbar-spritesheet.png`。

**加载链路（三级降级）：**

```
1. IndexedDB 缓存 ──→ 命中则直接使用，后台静默更新
         │
         ↓ 未命中
2. Rust 代理下载 ──→ invoke('fetch_remote_bytes') → 后端 reqwest GET → base64 data URL
         │              成功后缓存到 IndexedDB
         ↓ 失败
3. Canvas Fallback ──→ 本地 Canvas 绘制 16 帧简笔表情图
```

**为什么不直接在 WebView 中 fetch：**
- WebView 运行在 `http://tauri.localhost` origin
- CDN 未配置 `Access-Control-Allow-Origin` 响应头
- 浏览器同源策略（CORS）阻止跨域 fetch
- 详见同目录下《网络请求开发规范》文档

### 3.2 话术加载

话术（桌宠随机弹出的对话气泡文本）从 CDN JSON 文件加载：

```
invoke('fetch_speech_phrases')
  → Rust: cdn_get("https://xxxx.com/speech-phrases.json")
  → 去除 UTF-8 BOM → serde_json 解析
  → 返回 Vec<String>
```

前端有默认话术列表作为 fallback，话术加载失败不影响功能。

### 3.3 缓存策略

所有缓存存储在 IndexedDB（`toolbar-cache` 数据库, `resources` store, DB_VERSION=2），每条缓存包含 `{ data, etag }`，**无固定过期时间，通过 ETag 条件请求验证是否需要更新**。

| 数据 | 缓存 Key | 说明 |
|------|----------|------|
| 精灵图 | `spritesheet` | Blob 格式，带 ETag，条件请求返回 304 则复用缓存 |
| 话术 | `phrases` | JSON 数组，带 ETag，条件请求返回 304 则复用缓存 |

**加载流程**：启动时先读取缓存立即显示，然后携带缓存的 ETag 发送条件请求。服务端返回 304 表示无更新，返回 200 则更新缓存。这样既保证了首次渲染速度，又确保数据新鲜度。

**定时刷新**：每 30 分钟自动发送 ETag 条件 GET 请求检查更新，资源无变化时服务端返回 304（无 body），流量极小。

**DB 升级兼容**：`onupgradeneeded` 处理 v1→v2升级（删除旧 `images` store，创建 `resources` store）。

### 3.4 后端代理命令

| 命令 | 参数 | 返回 |
|------|------|------|
| `fetch_speech_phrases` | `ifNoneMatch: Option<String>` | `Option<{ phrases, etag }>` (null=304) |
| `fetch_remote_bytes` | `url, ifNoneMatch: Option<String>` | `Option<{ data_url, etag }>` (null=304) |

两者共用 `cdn_client()` 连接池（`OnceLock<reqwest::Client>`）和 `cdn_get()` 请求函数（含 ETag 支持 + 1 次自动重试）。

---

## 四、前端可调用的 Tauri 命令

Desktop / Toolbar / 主窗口**共用同一套 Tauri `invoke_handler`**，capabilities 对 `"windows": ["*"]` 生效。

与桌宠相关的命令：

| 命令 | 文件 | 说明 |
|------|------|------|
| `create_toolbar_window` | toolbar.rs | 创建工具栏窗口 |
| `handle_toolbar_menu_action` | toolbar.rs | 处理气泡菜单操作 |
| `save_toolbar_position` | toolbar.rs | 保存工具栏位置 |
| `fetch_speech_phrases` | commands.rs | 获取话术 |
| `fetch_remote_bytes` | commands.rs | CDN 资源代理下载 |
| `open_tool_window` | commands.rs | 打开主面板/VDD 等工具窗口 |

---

## 五、安全设计

### 5.1 CDN 代理安全

- **域名白名单**：`fetch_remote_bytes` 仅允许指定前缀域名
- **HTTPS 强制**：白名单限定 `https://`，不代理 HTTP
- **只读**：仅 GET 方法，不上传/修改数据
- **URL 注入防护**：白名单末尾有 `/`，阻止 `https://xxxx.com.evil.com/` 类攻击

### 5.2 窗口权限

- **现状**：所有窗口共用全部 Tauri 命令与同一套 capabilities，无按窗口隔离
- **风险**：若 desktop/toolbar 页面存在 XSS，攻击者可调用高危命令
- **建议**：按窗口类型缩小 capabilities（例如 toolbar 仅需资源加载和菜单操作权限）

### 5.3 单实例保护

- 单实例插件保证同一时间只有一个主进程
- 串流内「桌宠」菜单执行的 `sunshine-gui.exe --toolbar` 通过 detach 启动，若已有实例则由单实例机制合并

### 5.4 菜单命令执行

- `run_menu_cmd` 的 `cmd` 和 `elevated` 完全来自 apps.json
- 若 apps.json 被篡改可执行任意命令
- 建议限制 menu_cmd 白名单或做完整性校验

---

## 六、文件结构

```
src-tauri/src/
├── commands.rs      # CDN 代理、话术、工具窗口等命令
├── toolbar.rs       # 工具栏窗口创建、位置保存
├── main.rs          # 命令注册
└── windows.rs       # 窗口管理工具函数

src/renderer/toolbar/
├── ToolbarApp.vue   # 桌宠主组件：PixiJS 动画 + 气泡菜单 + 话术
├── main.js          # Vue 入口
└── index.html       # 工具栏页面
```
