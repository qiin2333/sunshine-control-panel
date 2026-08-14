const CODE_PATTERN = /^(DS5-[A-Z]+-\d{3}):\s*/

export const dualSenseErrorCode = (message) => String(message || '').match(CODE_PATTERN)?.[1] || ''

export const friendlyDualSenseError = (message, messages, context = 'generic') => {
  const code = dualSenseErrorCode(message)
  const contextual = messages?.contexts?.[context]
  return contextual?.[code]
    || messages?.codes?.[code]
    || contextual?.fallback
    || messages?.unknown
    || 'The operation could not be completed. Please try again.'
}
