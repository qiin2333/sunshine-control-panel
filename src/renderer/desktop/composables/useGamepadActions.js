import { ref } from 'vue'

/**
 * 视图级手柄动作注册表。
 *
 * 之前的按键提示条在所有页面显示同样的八项提示，但 X/Y/LT/RT 这些动作只有
 * 应用库真正实现——在仪表盘按 X 毫无反应，提示条在撒谎。这里让每个视图
 * 声明自己支持哪些动作，提示条只显示「当前真的会响应」的按键。
 */

/** 当前视图声明的动作 id 集合，如 ['search', 'favorite', 'menu']。 */
export const viewActions = ref(new Set())

/**
 * 视图在 onMounted 里声明自己的动作，onUnmounted 归还。
 * DesktopApp 会把全局动作（确认/返回/换页/光标）与这里合并后交给提示条。
 */
export function setViewActions(actions = []) {
  viewActions.value = new Set(actions)
}

export function clearViewActions() {
  viewActions.value = new Set()
}
