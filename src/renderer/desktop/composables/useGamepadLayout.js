import { computed } from 'vue'
import { gamepadName } from './useGamepad.js'

/**
 * 手柄布局识别：同一套「标准映射」按键在不同手柄上印着不同的符号，
 * 提示条必须跟着用户手里那台走——拿 DualSense 的人看到 "A 确定" 会按错。
 *
 * 识别依据是 gamepad.id（浏览器报告的字符串，通常含厂商标识）：
 *   Sony 054c（DualSense/DualShock，蓝牙下显示为 "Wireless Controller"）
 *   Microsoft 045e / XInput 系
 * 串流时主机上看到的是 Sunshine 的虚拟手柄（通常是 XInput 设备），那种
 * 情况下输入本来就直达游戏、不经过面板；面板里出现的都是本机手柄。
 *
 * Switch Pro 故意不单独识别：Nintendo 的 A/B 与 Xbox 物理位置互换，
 * 正确支持需要连确认/返回语义一起换，超出「两套提示」的范围——先落到
 * Xbox 布局（与标准映射的按钮索引一致），明确不做静默的半吊子支持。
 */

/**
 * PlayStation 识别特征。厂商标识 054c 最可靠；名称兜底覆盖改写 id 的
 * 第三方驱动（8BitDo 等接收器）。
 */
const PS_PATTERNS = [
  /054c/i, // Sony vendor id
  /dualsense/i,
  /dualshock/i,
  /playstation/i,
  // Sony 蓝牙默认名就是以 "Wireless Controller" 开头；不能裸匹配该词组，
  // 否则 "Xbox Elite Wireless Controller" 也会中招
  /^wireless controller/i,
  /ps[345] compatible/i,
]

/**
 * 从 gamepad.id 判断布局。纯函数，便于用真实设备的 id 字符串做单元测试。
 * @returns {'xbox' | 'ps'} 无法识别时落到 xbox（标准映射的按钮索引即 Xbox 约定）
 */
export function detectLayout(idString) {
  const id = String(idString || '').trim()
  if (!id) return 'xbox'
  return PS_PATTERNS.some((pattern) => pattern.test(id)) ? 'ps' : 'xbox'
}

/** 当前活跃手柄的布局，随手柄切换实时更新。 */
export const gamepadLayout = computed(() => detectLayout(gamepadName.value))

/**
 * 动作 id → 两种布局下的 [按键符号, 色调]。
 * PS 语义按主机惯例：✕ 确认、○ 返回、△ 搜索/查看、□ 选项，
 * 色调沿用 PS 手柄的按钮颜色（✕蓝 ○红 △绿 □粉）。
 */
const ACTION_CHIPS = {
  xbox: {
    confirm: ['A', 'a'],
    back: ['B', 'b'],
    search: ['Y', 'y'],
    favorite: ['X', 'x'],
    menu: ['☰', 'neutral'],
    pages: ['LB/RB', 'neutral'],
    scroll: ['LT/RT', 'neutral'],
    cursor: ['L3', 'neutral'],
  },
  ps: {
    confirm: ['✕', 'x'],
    back: ['○', 'b'],
    search: ['△', 'a'],
    favorite: ['□', 'y'],
    menu: ['☰', 'neutral'],
    pages: ['L1/R1', 'neutral'],
    scroll: ['L2/R2', 'neutral'],
    cursor: ['L3', 'neutral'],
  },
}

/**
 * 取某个动作在指定布局下的按键符号与色调。
 * @returns {{ glyph: string, tone: string }}
 */
export function chipFor(actionId, layout = gamepadLayout.value) {
  const [glyph, tone] = ACTION_CHIPS[layout]?.[actionId] || ACTION_CHIPS.xbox[actionId] || ['', 'neutral']
  return { glyph, tone }
}
