export const DEFAULT_NR_SHORTCUT = 'Ctrl+Alt+KeyN'

export function displayShortcut(value) {
  return (value || '')
    .split('+')
    .map((key) =>
      key
        .replace(/^Key(?=[A-Z]$)/, '')
        .replace(/^Digit(?=\d$)/, '')
        .replace(/^control$/i, 'Ctrl')
        .replace(/^alt$/i, 'Alt')
        .replace(/^shift$/i, 'Shift')
    )
    .join(' + ')
}

// Physical key codes match the native global-shortcut parser on non-US layouts.
export function shortcutFromEvent(event) {
  if (event.key === 'Escape' || event.key === 'Tab') return { cancel: true }
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key))
    return { modifier: true }
  if (event.metaKey || !(event.ctrlKey || event.altKey || event.shiftKey))
    return { error: 'nr_shortcut_invalid' }
  if (
    !/^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-9]|2[0-4])|Arrow(Up|Down|Left|Right)|Space|Enter|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Minus|Equal|BracketLeft|BracketRight|Backslash|Semicolon|Quote|Comma|Period|Slash|Backquote)$/.test(
      event.code
    )
  )
    return { error: 'nr_shortcut_invalid' }
  return {
    value: [
      event.ctrlKey && 'Ctrl',
      event.altKey && 'Alt',
      event.shiftKey && 'Shift',
      event.code
    ]
      .filter(Boolean)
      .join('+')
  }
}

const messages = {
  zh: {
    stream: '串流',
    connected: '当前连接',
    maintenance: '安装与维护',
    replaceFile: '更换组件文件',
    installSetup: '安装并配置',
    allowUse: '允许串流使用 ',
    nrPermissionHint:
      '这是功能许可开关，不代表当前画面已增强。默认行为在应用设置中选择，正在串流时可到“当前串流”即时开关。',
    hdrPermissionHint:
      '允许应用使用 SDR 转 HDR 功能；是否启用由应用配置决定，下次连接时生效。',
    appDefaults: '应用默认设置',
    setupHint: '先安装增强组件，再选择在哪些串流中使用。',
    setupHelp: '安装需要什么？',
    usageNotes: '功能说明与注意事项',
    permissionSaved: '功能设置已保存。应用默认配置与当前串流效果分别管理。',
    needsInstall: '待安装',
    needsSetup: '需要配置',
    ready: '已就绪',
    installedDisabled: '已安装 · 未启用',
    liveTitle: 'DLSS NR 画面增强',
    liveHint: '立即应用，仅影响这次串流',
    scaleHint: '降低比例可减少 GPU 开销，细节效果可能减弱。',

    title: '画质增强管理',
    intro: '管理增强功能，查看串流效果，设置浮层与快捷键',
    components: '增强功能',
    session: '当前串流',
    settings: '浮层与快捷键',
    componentHint: '默认增强方案按应用配置；这里统一管理组件与实时控制。',
    hdrIntro: '将 SDR 内容转换为 HDR 输出。',
    nrIntro: '增强画面细节，支持 SDR 与原生 HDR。',
    show: '显示画质增强浮层',
    showHint: '串流时快速调节 DLSS NR',
    opacity: '默认背景不透明度',
    keys: '全局快捷键',
    reset: '恢复默认',
    visibilityKey: '显示 / 隐藏浮层',
    nrKey: '切换 DLSS NR',
    unset: '未设置',
    clear: '清除',
    record: '请按下组合键…',
    recordHint:
      '请按下含 Ctrl、Alt 或 Shift 的组合键。Esc 取消，30 秒后自动退出。',
    keyHint: '浮层隐藏时仍可使用，仅作用于选定串流。',
    saved: '已保存',
    local: '本机设置 · 自动保存',
    preview: '浮层预览',
    hidden: '浮层已隐藏',
    previewHint: '快捷键与浮层显示独立；此处只预览外观。',
    previewState: 'NR 状态示意',
    choose: '请选择目标串流',
    expired: '已选串流结束，请重新选择',
    noSession: '等待串流',
    disconnected: '主机未连接',
    refresh: '重试',
    sessionHint: '实时调节只影响选定会话，不修改应用默认设置。',
    output: '输出格式',
    scale: '处理比例',
    enhancement: '画面增强',
    open: '打开实时浮层',
    statusUnavailable: '快捷键未注册，请更换组合或重试保存。',
    disabledKey: '快捷键未设置',
    action: '切换增强',
    errors: {
      nr_shortcut_invalid: '请使用有效的 Ctrl、Alt 或 Shift 组合键',
      nr_shortcut_duplicate: '两个操作不能使用相同快捷键',
      nr_shortcut_conflict: '快捷键已被占用，原设置保持不变',
      nr_shortcut_restore_failed:
        '快捷键恢复失败，请重新设置；下方显示实际注册状态',
      nr_shortcut_unregister: '原快捷键注销失败，设置未保存',
      nr_select_session: '存在多路串流，请先选择目标',
      nr_session_ended: '已选串流结束，请重新选择',
      nr_no_session: '当前没有串流',
      nr_settings_pending: '正在应用设置，请稍后再试',
      nr_toggle_unsupported: '此串流不支持 NR 切换'
    },
    failed: '操作失败，请重试'
  },
  en: {
    stream: 'Stream',
    connected: 'Connected',
    maintenance: 'Installation & maintenance',
    replaceFile: 'Replace component file',
    installSetup: 'Install & set up',
    allowUse: 'Allow streams to use ',
    nrPermissionHint:
      'Makes the feature available; it does not indicate an active effect. Set app defaults in application settings, or toggle the current stream live.',
    hdrPermissionHint:
      'Allows apps to use SDR-to-HDR conversion. Per-app settings take effect on the next connection.',
    appDefaults: 'Application defaults',
    setupHint:
      'Install the enhancement component, then choose which streams use it.',
    setupHelp: 'Installation requirements',
    usageNotes: 'Usage notes',
    permissionSaved:
      'Feature setting saved. App defaults and live-stream effects are managed separately.',
    needsInstall: 'Not installed',
    needsSetup: 'Setup required',
    ready: 'Ready',
    installedDisabled: 'Installed · disabled',
    liveTitle: 'DLSS NR enhancement',
    liveHint: 'Apply now to this stream only',
    scaleHint: 'A lower scale reduces GPU cost, but may reduce detail.',

    title: 'Image enhancement',
    intro: 'Components, live effects and overlay controls',
    components: 'Enhancements',
    session: 'Current streams',
    settings: 'Overlay & shortcuts',
    componentHint:
      'Default effects are configured per app. Manage components and live controls here.',
    hdrIntro: 'Convert SDR content to HDR output.',
    nrIntro: 'Enhance details in SDR and native HDR content.',
    show: 'Show enhancement overlay',
    showHint: 'Adjust DLSS NR during a stream',
    opacity: 'Default background opacity',
    keys: 'Global shortcuts',
    reset: 'Restore defaults',
    visibilityKey: 'Show / hide overlay',
    nrKey: 'Toggle DLSS NR',
    unset: 'Not set',
    clear: 'Clear',
    record: 'Press a shortcut…',
    recordHint:
      'Use Ctrl, Alt or Shift. Esc cancels; capture expires after 30 seconds.',
    keyHint: 'Works with the overlay hidden. Only affects the selected stream.',
    saved: 'Saved',
    local: 'Local settings · Saved automatically',
    preview: 'Overlay preview',
    hidden: 'Overlay hidden',
    previewHint:
      'Shortcuts work independently of visibility. Appearance preview only.',
    previewState: 'Example NR state',
    choose: 'Select a target stream',
    expired: 'The selected stream ended. Select another stream.',
    noSession: 'Waiting for a stream',
    disconnected: 'Host disconnected',
    refresh: 'Retry',
    sessionHint: 'Changes affect the selected session only, not app defaults.',
    output: 'Output format',
    scale: 'Processing scale',
    enhancement: 'Enhancement',
    open: 'Open live overlay',
    statusUnavailable:
      'Shortcut unavailable. Choose another combination or save to retry.',
    disabledKey: 'Shortcut not set',
    action: 'Toggle enhancement',
    errors: {
      nr_shortcut_invalid: 'Use a valid Ctrl, Alt or Shift shortcut',
      nr_shortcut_duplicate: 'The two actions need different shortcuts',
      nr_shortcut_conflict:
        'Shortcut is already in use. Previous settings are retained.',
      nr_shortcut_restore_failed:
        'Could not restore shortcuts. Reconfigure them; actual registration status is shown below.',
      nr_shortcut_unregister:
        'Could not unregister the old shortcut. Settings were not saved.',
      nr_select_session: 'Select the target stream first',
      nr_session_ended: 'The selected stream ended. Select another stream.',
      nr_no_session: 'No active stream',
      nr_settings_pending: 'Settings are being applied. Try again shortly.',
      nr_toggle_unsupported: 'NR switching is unavailable for this stream'
    },
    failed: 'Operation failed. Try again.'
  }
}
export function enhancementControlsText(locale) {
  return messages[locale.startsWith('zh') ? 'zh' : 'en']
}
export function enhancementError(text, error) {
  return text.errors[String(error)] || text.failed
}
