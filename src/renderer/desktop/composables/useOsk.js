import { computed, ref } from 'vue'

/**
 * 屏幕键盘服务。
 *
 * 大屏模式下没有鼠标键盘，搜索框、PIN 配对、设备重命名这些地方原本是走不通的。
 * 这里提供一个全局单例：任何地方都可以 `await openOsk(...)` 拿到用户输入。
 */

const initialState = {
  open: false,
  title: '',
  value: '',
  mode: 'text',
  maxLength: 0,
  placeholder: '',
}

export const oskState = ref({ ...initialState })
export const oskOpen = computed(() => oskState.value.open)

let resolveCurrent = null

function settle(value) {
  const resolve = resolveCurrent
  resolveCurrent = null
  oskState.value = { ...initialState }
  resolve?.(value)
}

/**
 * 打开屏幕键盘，resolve 用户确认的文本；取消时 resolve `null`。
 */
export function openOsk({
  value = '',
  title = '',
  mode = 'text',
  maxLength = 0,
  placeholder = '',
} = {}) {
  // 同时只允许一个键盘，新的请求取消旧的
  if (resolveCurrent) settle(null)

  oskState.value = {
    open: true,
    title,
    value: String(value ?? ''),
    mode: mode === 'number' ? 'number' : 'text',
    maxLength: Number(maxLength) > 0 ? Number(maxLength) : 0,
    placeholder,
  }

  return new Promise((resolve) => {
    resolveCurrent = resolve
  })
}

export function commitOsk(value) {
  settle(String(value ?? ''))
}

export function cancelOsk() {
  settle(null)
}

function inferMode(input) {
  if (input.type === 'number' || input.type === 'tel') return 'number'
  if (input.inputMode === 'numeric' || input.inputMode === 'decimal') return 'number'
  return 'text'
}

function inferMaxLength(input) {
  const max = Number(input.maxLength)
  // 未设置时浏览器会返回 -1 或 524288，两者都不该当成真实上限
  return max > 0 && max < 4096 ? max : 0
}

/**
 * 用屏幕键盘编辑一个真实的 input/textarea。
 *
 * 写回后派发 input/change 事件，这样 `v-model` 和 `@input` 绑定都能收到更新。
 */
export async function editInputWithOsk(input, options = {}) {
  const isEditable =
    input instanceof HTMLInputElement || input instanceof HTMLTextAreaElement
  if (!isEditable || input.readOnly || input.disabled) return false

  const next = await openOsk({
    value: input.value,
    title:
      options.title ||
      input.getAttribute('aria-label') ||
      input.getAttribute('placeholder') ||
      '',
    mode: options.mode || inferMode(input),
    maxLength: options.maxLength ?? inferMaxLength(input),
    placeholder: input.getAttribute('placeholder') || '',
  })

  if (next === null) return false

  input.value = next
  input.dispatchEvent(new Event('input', { bubbles: true }))
  input.dispatchEvent(new Event('change', { bubbles: true }))
  return true
}

/** 焦点落在文本输入上时，A 键应该打开键盘而不是「点击」它。 */
export function isTextEntryElement(element) {
  if (element instanceof HTMLTextAreaElement) return true
  if (!(element instanceof HTMLInputElement)) return false
  const nonText = ['checkbox', 'radio', 'button', 'submit', 'reset', 'file', 'range', 'color']
  return !nonText.includes(element.type)
}
