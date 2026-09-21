const messages = {
  "zh": {
    "states": {
      "disabled": "NR 已关闭",
      "active": "NR 已开启",
      "warming_up": "NR 启动中",
      "stopping": "NR 关闭中",
      "scaling": "切换处理比例",
      "degraded": "NR 已降级",
      "idle": "等待串流",
      "blocked": "NR 不可用",
      "unavailable": "主机未连接"
    },
    "scale": "NR 处理比例",
    "processing": "处理尺寸",
    "targetSize": "预设尺寸",
    "scaleHint": "降低比例可减少开销；原画尺寸保持不变",
    "scaleFailed": "该比例未能生效，已请求恢复上一档",
    "choose": "请选择会话",
    "drag": "拖动浮层",
    "session": "当前串流",
    "close": "关闭浮层",
    "enhancement": "画面增强",
    "preserved": "保持输出",
    "onlySession": "仅影响当前会话，不修改应用默认设置",
    "opacity": "背景不透明度",
    "shortcut": "切换画面增强",
    "shortcutUnavailable": "快捷键不可用，可点击开关",
    "hdrBusy": "RTX HDR 正在占用增强位置",
    "unsupported": "当前主机或捕获路径不支持实时切换",
    "fallback": "增强未生效，正在使用原画；关闭后可重新尝试",
    "wait": "等待主机处理下一帧；首次开启需要初始化",
    "retry": "重试",
    "disconnected": "无法读取主机状态，请确认 Sunshine 正在运行",
    "ended": "该串流已结束",
    "failed": "切换失败，请重试"
  },
  "en": {
    "states": {
      "disabled": "NR off",
      "active": "NR on",
      "warming_up": "NR starting",
      "stopping": "NR stopping",
      "scaling": "Changing NR scale",
      "degraded": "NR bypassed",
      "idle": "Waiting for stream",
      "blocked": "NR unavailable",
      "unavailable": "Host disconnected"
    },
    "scale": "NR processing scale",
    "processing": "Processing size",
    "targetSize": "Planned size",
    "scaleHint": "Lower scales reduce cost. Original dimensions stay unchanged.",
    "scaleFailed": "Scale failed; restoration of the previous scale was requested.",
    "choose": "Select a stream",
    "drag": "Drag overlay",
    "session": "Current stream",
    "close": "Close overlay",
    "enhancement": "Enhancement",
    "preserved": "Output preserved",
    "onlySession": "This session only. App defaults stay unchanged.",
    "opacity": "Background opacity",
    "shortcut": "Toggle enhancement",
    "shortcutUnavailable": "Shortcut unavailable; use the switch",
    "hdrBusy": "RTX HDR is using the enhancement slot",
    "unsupported": "This host or capture path does not support live switching",
    "fallback": "Using the original image. Switch off and on to retry.",
    "wait": "Waiting for the next frame. First use needs initialization.",
    "retry": "Retry",
    "disconnected": "Cannot read host status. Check that Sunshine is running.",
    "ended": "This stream has ended",
    "failed": "Could not switch. Try again."
  }
}

export function nrOverlayText(locale) {
  return messages[locale.startsWith("zh") ? "zh" : "en"]
}
