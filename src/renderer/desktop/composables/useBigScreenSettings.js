import { ref, watch } from 'vue'

/**
 * 大屏模式自己的偏好项。
 *
 * 故意不放进 Rust 侧的 `DesktopSettings`：这些只影响大屏 shell 的交互，主窗口和
 * 托盘都用不到，走 localStorage 可以避免为每个开关改动共享设置结构。
 */

export const BIG_SCREEN_SETTINGS_KEY = 'foundation-desktop-bigscreen'
export const BIG_SCREEN_SETTINGS_UPDATED = 'foundation-bigscreen-settings-updated'

export const defaultBigScreenSettings = {
  /** 启动游戏后最小化大屏窗口，退出后自动恢复。 */
  autoYieldOnLaunch: true,
  /** 允许按左摇杆键切换手柄光标模式。 */
  gamepadCursorEnabled: true,
  /** 焦点在文本框上按 A 时自动弹出屏幕键盘。 */
  oskAutoOpen: true,
  /** 手柄导航音效（tick/确认/返回）。 */
  navSounds: true,
}

function load() {
  try {
    const saved = localStorage.getItem(BIG_SCREEN_SETTINGS_KEY)
    if (!saved) return { ...defaultBigScreenSettings }
    return { ...defaultBigScreenSettings, ...JSON.parse(saved) }
  } catch {
    return { ...defaultBigScreenSettings }
  }
}

export const bigScreenSettings = ref(load())

let persistTimer = null
watch(
  bigScreenSettings,
  (value) => {
    clearTimeout(persistTimer)
    persistTimer = setTimeout(() => {
      try {
        localStorage.setItem(BIG_SCREEN_SETTINGS_KEY, JSON.stringify(value))
      } catch {
        // 存储不可用时保留内存中的值
      }
    }, 200)
    window.dispatchEvent(new CustomEvent(BIG_SCREEN_SETTINGS_UPDATED, { detail: value }))
  },
  { deep: true }
)

export function setBigScreenSetting(key, value) {
  if (!(key in defaultBigScreenSettings)) return
  bigScreenSettings.value = { ...bigScreenSettings.value, [key]: value }
}

export function useBigScreenSettings() {
  return { bigScreenSettings, setBigScreenSetting, defaultBigScreenSettings }
}
