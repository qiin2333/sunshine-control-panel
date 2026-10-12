import { ElMessage } from 'element-plus'
import { useI18n } from '../desktop/i18n/index.js'
import { formatMessage } from '../shared/format-message.js'

/**
 * 窗口控制 Composable
 */
export function useWindowControls(isMaximized) {
  const { t } = useI18n()
  /**
   * 执行窗口操作
   * @param {string} action - 操作名称 (minimize/hide)
   * @param {string} actionName - 操作显示名称
   */
  const performWindowAction = async (action, actionName) => {
    try {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
      const currentWindow = getCurrentWebviewWindow()
      await currentWindow[action]()
      console.log(`✅ 窗口已${actionName}`)
    } catch (error) {
      console.error(`${actionName}窗口失败:`, error)
      ElMessage.error(formatMessage(t.value.windowActions.actionFailed, { action: actionName }))
    }
  }

  /**
   * 最小化窗口
   */
  const minimizeWindow = async () => {
    await performWindowAction('minimize', t.value.windowActions.minimize)
  }

  /**
   * 切换最大化状态
   */
  const toggleMaximize = async () => {
    try {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
      const window = getCurrentWebviewWindow()

      const maximized = await window.isMaximized()

      if (maximized) {
        await window.unmaximize()
        isMaximized.value = false
        console.log('✅ 窗口已还原')
      } else {
        await window.maximize()
        isMaximized.value = true
        console.log('✅ 窗口已最大化')
      }
    } catch (error) {
      console.error('❌ 切换最大化失败:', error)
      ElMessage.error(t.value.windowActions.toggleMaximizeFailed)
    }
  }

  /**
   * 关闭窗口（隐藏）
   */
  const closeWindow = async () => {
    await performWindowAction('hide', t.value.windowActions.hide)
  }

  return {
    minimizeWindow,
    toggleMaximize,
    closeWindow,
  }
}


