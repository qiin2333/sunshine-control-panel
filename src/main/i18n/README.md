# Electron 菜单国际化实现

这个国际化系统为 Sunshine Control Panel 的 Electron 菜单提供了多语言支持，支持中文、英文、日文、韩文、法文和德文。

## 功能特性

- 🌍 支持 6 种语言：中文、英文、日文、韩文、法文、德文
- 🔄 动态语言切换
- 💾 语言设置持久化存储
- 🎯 自动检测系统语言
- 📝 支持参数化翻译
- 🔧 易于扩展新语言
- 🎛️ 语言菜单作为一级菜单，方便用户访问
- 📊 关于菜单显示真实的Sunshine版本号

## 文件结构

```
src/main/i18n/
├── index.js                 # 国际化核心功能
├── languageSwitcher.js      # 语言切换器
├── test.js                  # 测试文件
├── README.md               # 说明文档
└── locales/                # 语言包目录
    ├── zh-CN.json          # 简体中文
    ├── en-US.json          # 英文
    ├── ja-JP.json          # 日文
    ├── ko-KR.json          # 韩文
    ├── fr-FR.json          # 法文
    └── de-DE.json          # 德文
```

## 使用方法

### 1. 基本翻译

```javascript
import { t } from './i18n/index.js'

// 简单翻译
const windowLabel = t('menu.window')  // "窗口" 或 "Window"

// 带参数的翻译
const message = t('dialog.uninstallComplete', { code: 0 })  // "虚拟显示器卸载完成: 0"
```

### 2. 语言切换

```javascript
import { setLanguage, getCurrentLanguage } from './i18n/index.js'

// 设置语言
setLanguage('en-US')

// 获取当前语言
const currentLang = getCurrentLanguage()
```

### 3. 菜单集成

```javascript
import { setupApplicationMenu } from './menu.js'

// 设置应用菜单（自动初始化国际化）
setupApplicationMenu(mainWindow)
```

### 4. 菜单结构

语言菜单现在作为一级菜单显示，菜单结构如下：

```
窗口 (Window)
管理 (Management)  
使用教程 (Tutorial)
小工具 (Tools)
语言 (Language) ← 新增的一级菜单
  ├── 简体中文
  ├── English
  ├── 日本語
  ├── 한국어
  ├── Français
  └── Deutsch
关于 (About) ← 显示真实的Sunshine版本号
```

### 5. Sunshine版本获取

```javascript
import { getSunshineVersion } from './utils.js'

// 获取Sunshine版本号
const version = await getSunshineVersion()
console.log('Sunshine版本:', version)  // 例如: "0.21.0"
```

## 语言包格式

语言包使用 JSON 格式，支持嵌套结构：

```json
{
  "menu": {
    "window": "窗口",
    "management": "管理"
  },
  "submenu": {
    "minimize": "最小化",
    "close": "关闭"
  },
  "dialog": {
    "confirmUninstall": "确认卸载? 卸载后可通过重新安装基地版sunshine恢复。",
    "uninstallComplete": "虚拟显示器卸载完成: {code}"
  },
  "about": {
    "copyright": "Copyright (c) 2023 Qiin",
    "sunshineVersion": "Sunshine 版本"
  }
}
```

## 添加新语言

1. 在 `locales/` 目录下创建新的语言包文件，如 `es-ES.json`
2. 在 `index.js` 的 `SUPPORTED_LANGUAGES` 数组中添加新语言
3. 在 `getSystemLanguage()` 函数中添加语言映射

## API 参考

### 核心函数

- `t(key, params, languageCode)` - 翻译函数
- `getCurrentLanguage()` - 获取当前语言
- `setLanguage(languageCode)` - 设置语言
- `getSystemLanguage()` - 获取系统语言
- `loadTranslations(languageCode)` - 加载语言包
- `initI18n()` - 初始化国际化

### 语言切换器

- `createLanguageMenu(mainWindow)` - 创建语言一级菜单
- `createLanguageMenuItems(mainWindow)` - 创建语言菜单项列表
- `addLanguageSwitcherToMenu(menuTemplate, mainWindow)` - 添加语言切换到菜单

### 工具函数

- `getSunshineVersion()` - 获取Sunshine版本号（异步）

## 版本获取功能

关于菜单现在会显示真实的Sunshine版本号，而不是Electron版本。版本获取功能：

- 通过执行 `sunshine.exe --version` 命令获取版本
- 支持多种版本号格式的解析
- 包含错误处理和超时机制
- 如果获取失败，显示 "Unknown"

## 注意事项

1. 语言设置会保存在用户数据目录的 `language.json` 文件中
2. 如果找不到指定语言的翻译，会自动回退到默认语言（中文）
3. 语言切换后需要重启应用程序才能完全生效
4. 支持参数化翻译，使用 `{参数名}` 格式
5. Sunshine版本获取是异步操作，需要等待结果

## 扩展建议

- 可以添加更多语言支持
- 可以实现实时语言切换（无需重启）
- 可以添加语言包热重载功能
- 可以集成到渲染进程的 Vue 组件中
- 可以添加版本检查功能，提示用户更新
