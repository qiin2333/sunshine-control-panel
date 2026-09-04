const NATIVE_MESSAGE_TYPES = new Set([
  'native-updater-context-request',
  'native-update-request',
  'native-rtx-hdr-context-request',
  'native-rtx-hdr-open-request',
])

export function isNativeControlPanelMessage(data) {
  return Boolean(data && NATIVE_MESSAGE_TYPES.has(data.type))
}

export function isTrustedNativeControlPanelMessage(data, eventTrusted) {
  return isNativeControlPanelMessage(data)
    && eventTrusted
    && data.source === 'sunshine-webui'
}
