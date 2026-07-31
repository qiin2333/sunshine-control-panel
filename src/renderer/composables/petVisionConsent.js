import { confirm as dialogConfirm } from '@tauri-apps/plugin-dialog'

export async function confirmPetVisionEnable(text) {
  try {
    return await dialogConfirm(text.message, {
      title: text.title,
      kind: 'warning',
      okLabel: text.accept,
      cancelLabel: text.cancel,
    })
  } catch (error) {
    console.warn('[桌宠] 无法显示桌面观察安全确认:', error)
    return false
  }
}
