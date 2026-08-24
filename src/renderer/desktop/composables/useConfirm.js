import { computed, ref } from 'vue'

/**
 * 应用内确认弹窗。
 *
 * 不能用 `window.confirm`：原生对话框接不到手柄输入，沙发上会直接卡死在一个
 * 点不掉的弹窗前面。
 */

const initialState = {
  open: false,
  title: '',
  message: '',
  confirmLabel: '',
  cancelLabel: '',
  danger: false,
}

export const confirmState = ref({ ...initialState })
export const confirmOpen = computed(() => confirmState.value.open)

let resolveCurrent = null

function settle(value) {
  const resolve = resolveCurrent
  resolveCurrent = null
  confirmState.value = { ...initialState }
  resolve?.(value)
}

export function requestConfirm({
  title = '',
  message = '',
  confirmLabel = '',
  cancelLabel = '',
  danger = false,
} = {}) {
  if (resolveCurrent) settle(false)
  confirmState.value = { open: true, title, message, confirmLabel, cancelLabel, danger }
  return new Promise((resolve) => {
    resolveCurrent = resolve
  })
}

export function acceptConfirm() {
  settle(true)
}

export function rejectConfirm() {
  settle(false)
}
