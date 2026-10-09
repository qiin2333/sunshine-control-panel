export function formatMessage(message, params = {}) {
  return message.replace(/\{(\w+)\}/g, (placeholder, key) =>
    Object.hasOwn(params, key) ? String(params[key]) : placeholder,
  )
}
