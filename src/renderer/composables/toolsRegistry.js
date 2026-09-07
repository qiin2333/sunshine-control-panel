/**
 * 工具注册表（轻插件化）
 *
 * 集中描述侧边栏所有菜单项。每个工具自描述其 icon、文案、action、可见性。
 * 新增工具只需在对应 factory 中追加一项，无需改 SidebarMenu.vue。
 *
 * 设计理由：
 *   - 不引入 Tauri Plugin 模式（成本过高，见 README）
 *   - 不做运行时事件总线注册（无第三方扩展场景）
 *   - 仅将"菜单元数据"集中，让组件只负责渲染
 *
 * 用法：
 *   const ctx = { t, router, isDark, isAdmin, locale, ...useTools(), ...useSidebarState() }
 *   const items = computed(() => createUtilityTools(ctx))
 */

import {
  Monitor,
  Delete,
  Link,
  Setting,
  CopyDocument,
  Timer,
  DataLine,
  Cpu,
  Minus,
  Close,
  Sunny,
  Moon,
  Key,
  Download,
  Connection,
  MagicStick,
  Brush,
} from '@element-plus/icons-vue'
import IconLang from '../desktop/icons/IconLang.vue'
import IconGamepad from '../desktop/icons/IconGamepad.vue'
import { ROUTES } from './useRouter.js'

// ───────────────────────────────────────────────
// 类型说明（JSDoc，非强制）
//
// /** @typedef {{
//   id: string,                          // 唯一标识
//   icon: object,                        // Element Plus / Vue 图标组件
//   label: string,                       // 已国际化的显示文案
//   action: () => void | Promise<void>,  // 点击执行
//   isActive?: () => boolean,            // 是否高亮（路由匹配等）
//   hasSwitch?: boolean,                 // 是否带开关（仅 management 项）
//   class?: string,                      // 额外 CSS 类（如 'danger' / 'warning'）
//   visible?: boolean,                   // 显式控制可见性（默认 true）
// }} ToolItem */
// ───────────────────────────────────────────────

/**
 * 管理类菜单（高级设置、虚拟显示器、AI 助手、检查更新等）
 * @param {object} ctx
 * @returns {ToolItem[]}
 */
export function createManagementTools(ctx) {
  return [
    {
      id: 'home',
      icon: Setting,
      label: ctx.t.value.sidebar.advancedSettings,
      action: ctx.goHome,
      isActive: () => ctx.router.isRoute(ROUTES.HOME),
    },
    {
      id: 'vdd',
      icon: Monitor,
      label: ctx.t.value.sidebar.virtualDisplay,
      action: ctx.openVddSettings,
      isActive: () => ctx.router.isRoute(ROUTES.VDD_SETTINGS),
    },
    {
      id: 'web-stream',
      icon: Connection,
      label: ctx.t.value.sidebar.webStream,
      action: ctx.openWebStream,
      isActive: () => ctx.router.isRoute(ROUTES.WEB_STREAM),
      visible: import.meta.env.DEV,
    },
    {
      id: 'device-hub',
      icon: IconGamepad,
      label: ctx.t.value.sidebar.deviceHub,
      action: ctx.openControllers,
      isActive: () => ctx.router.isRoute(ROUTES.CONTROLLERS)
        || ctx.router.isRoute(ROUTES.DUALSENSE)
        || ctx.router.isRoute(ROUTES.CONTROLLERS_HUB),
    },
    {
      id: 'ai-assistant',
      icon: MagicStick,
      label: ctx.t.value.sidebar.aiAssistant,
      action: ctx.openAiAssistant,
      isActive: () => ctx.router.isRoute(ROUTES.AI_ASSISTANT),
    },
    {
      id: 'check-update',
      icon: Download,
      label: ctx.t.value.sidebar.checkUpdate,
      action: ctx.handleCheckForUpdates,
      hasSwitch: true,
    },
  ].filter((it) => it.visible !== false)
}

/**
 * 工具类菜单（外部链接、计时器、手柄测试、剪贴板、清理等）
 * @param {object} ctx
 * @returns {ToolItem[]}
 */
export function createUtilityTools(ctx) {
  return [
    {
      id: 'official-website',
      icon: Link,
      label: ctx.t.value.sidebar.officialWebsite,
      action: () => ctx.openUrl('https://www.alkaidlab.com/'),
    },
    {
      id: 'stream-timer',
      icon: Timer,
      label: ctx.t.value.sidebar.streamTimer,
      action: ctx.openTimer,
    },
    {
      id: 'latency-test',
      icon: DataLine,
      label: ctx.t.value.sidebar.latencyTest,
      action: () => ctx.openUrl('https://yangkile.github.io/D-lay/'),
    },
    {
      id: 'gamepad-test',
      icon: Cpu,
      label: ctx.t.value.sidebar.gamepadTest,
      action: ctx.openGamepadTest,
    },
    {
      id: 'stylus-input-probe',
      icon: Brush,
      label: ctx.t.value.tools.stylusInputProbe,
      action: ctx.openStylusInputProbe,
    },
    {
      id: 'clipboard-sync',
      icon: CopyDocument,
      label: ctx.t.value.sidebar.clipboardSync,
      action: ctx.showClipboardSyncStatus,
      isActive: () => ctx.clipboardSyncEnabled?.value === true,
    },
    {
      id: 'clean-temp',
      icon: Delete,
      label: ctx.t.value.sidebar.cleanTemp,
      action: ctx.cleanupCovers,
    },
  ].filter((it) => it.visible !== false)
}

/**
 * 底部菜单（主题、语言、最小化、关闭、提权）
 * @param {object} ctx
 * @returns {ToolItem[]}
 */
export function createFooterTools(ctx) {
  return [
    {
      id: 'theme',
      icon: ctx.isDark.value ? Sunny : Moon,
      label: ctx.isDark.value ? ctx.t.value.sidebar.lightMode : ctx.t.value.sidebar.darkMode,
      action: ctx.toggleTheme,
    },
    {
      id: 'locale',
      icon: IconLang,
      label: ctx.locale.value === 'zh' ? 'EN' : '中文',
      action: ctx.toggleLocale,
    },
    {
      id: 'minimize',
      icon: Minus,
      label: ctx.t.value.sidebar.minimize,
      action: ctx.minimizeWindow,
    },
    {
      id: 'hide',
      icon: Close,
      label: ctx.t.value.sidebar.hideWindow,
      action: ctx.closeWindow,
      class: 'danger',
    },
    {
      id: 'restart-as-admin',
      icon: Key,
      label: ctx.t.value.sidebar.restartAsAdmin,
      action: ctx.restartAsAdmin,
      class: 'warning',
      visible: !ctx.isAdmin.value,
    },
  ].filter((it) => it.visible !== false)
}
