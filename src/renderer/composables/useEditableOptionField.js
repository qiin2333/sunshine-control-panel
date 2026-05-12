import { computed, nextTick, ref } from 'vue'
import { ElMessage } from 'element-plus'

const normalizeValues = (values = []) => values
  .map((value) => String(value).trim())
  .filter(Boolean)

const emitMessage = (entry, value) => {
  if (!entry) {
    return
  }

  const normalized = typeof entry === 'function'
    ? { text: entry }
    : entry

  const type = normalized.type || 'info'
  const text = typeof normalized.text === 'function'
    ? normalized.text(value)
    : normalized.text

  if (text) {
    ElMessage[type](text)
  }
}

export function useEditableOptionField({
  initialValues = [],
  compare,
  validate,
  messages = {},
}) {
  const options = ref(new Set(normalizeValues(initialValues)))
  const draft = ref('')
  const visible = ref(false)
  const inputRef = ref(null)

  const sortedOptions = computed(() => Array.from(options.value).sort(compare))

  const setValues = (values = []) => {
    options.value = new Set(normalizeValues(values))
  }

  const showInput = () => {
    visible.value = true
    nextTick(() => inputRef.value?.focus())
  }

  const addValue = (rawValue) => {
    const value = String(rawValue || '').trim()

    if (!validate(value)) {
      emitMessage(messages.invalid)
      return false
    }

    if (options.value.has(value)) {
      emitMessage(messages.exists, value)
      return false
    }

    options.value.add(value)
    emitMessage(messages.added, value)
    return true
  }

  const addDraft = () => {
    const added = addValue(draft.value)
    draft.value = ''
    visible.value = false
    return added
  }

  const confirmInput = () => {
    if (draft.value) {
      addDraft()
      return
    }

    visible.value = false
  }

  const removeValue = (value) => {
    if (options.value.size <= 1) {
      emitMessage(messages.minOne)
      return
    }

    options.value.delete(value)
    emitMessage(messages.removed, value)
  }

  return {
    options,
    draft,
    visible,
    inputRef,
    sortedOptions,
    setValues,
    showInput,
    addDraft,
    addValue,
    confirmInput,
    removeValue,
  }
}
