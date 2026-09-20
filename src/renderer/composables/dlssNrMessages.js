import { enhancementMessages } from './enhancementMessages.js'

export const dlssNrMessages = {
  en: {
    ...enhancementMessages.en,
    "acquisitionDescription": "Choose a locally obtained NVIDIA nvngx_dlssnr.dll. This is separate from RTX HDR; do not select nvngx_truehdr.dll. Import records the file digest. Use Choose again to update it.",
    "adapterMissingNotice": "The Sunshine DLSS NR adapter is missing. Reinstall or update Sunshine, then refresh the component status.",
    "bridge": "Sunshine DLSS NR adapter",
    "bridgeMissingNotice": "This Sunshine installation does not support DLSS NR. Update Sunshine and try again.",
    "componentTitle": "NVIDIA DLSS NR",
    "enable": "Enable DLSS NR",
    "enableHint": "After enabling the component, enable DLSS NR in application settings and reconnect. Native HDR retains HDR output; RTX HDR takes priority when enabled.",
    "installConfirm": "Install the selected nvngx_dlssnr.dll? Administrator permission may be required.",
    "installFailed": "Unable to install the NVIDIA DLSS NR component.",
    "installSuccess": "The component is installed. You can now enable DLSS NR.",
    "installTitle": "Install NVIDIA DLSS NR component",
    "intro": "Experimental enhancement before encoding; compatible Sunshine builds support SDR and native HDR. Adds GPU work and may change faces and textures. Disabled by default; compatible Sunshine builds offer optional optical-flow motion estimation in application settings.",
    "repairTitle": "Replace NVIDIA DLSS NR component",
    "runtime": "NVIDIA DLSS NR component",
    "runtimeMissingNotice": "The NVIDIA DLSS NR runtime is missing. Choose component to import a local nvngx_dlssnr.dll.",
    "selectRuntime": "Choose nvngx_dlssnr.dll",
    "title": "NVIDIA DLSS NR",
    "uninstallConfirm": "Disable DLSS NR and remove the installed NVIDIA component? Your original file will not be changed.",
    "uninstallSuccess": "The component was removed and DLSS NR was disabled.",
    "uninstallTitle": "Remove NVIDIA DLSS NR component",
    "appSettingsFailed": "Could not open application settings. Check that Sunshine is running and try again.",
    "appSettings": "Open application settings"
  },
  zh: {
    ...enhancementMessages.zh,
    "acquisitionDescription": "选择你已取得的 NVIDIA nvngx_dlssnr.dll。此组件与 RTX HDR 不同，请勿选择 nvngx_truehdr.dll。导入后会记录文件摘要；更新运行库请使用“重新选择”。",
    "adapterMissingNotice": "缺少 Sunshine DLSS NR 适配组件。请重新安装或更新 Sunshine，然后刷新组件状态。",
    "bridge": "Sunshine DLSS NR 适配组件",
    "bridgeMissingNotice": "当前 Sunshine 不支持 DLSS NR，请更新主程序后重试。",
    "componentTitle": "NVIDIA DLSS NR",
    "enable": "启用 DLSS NR",
    "enableHint": "启用组件后，还需在应用设置中开启 DLSS NR 并重新连接。原生 HDR 保留 HDR 输出；开启 RTX HDR 时优先使用 RTX HDR。",
    "installConfirm": "是否安装所选的 nvngx_dlssnr.dll？过程中可能需要管理员权限。",
    "installFailed": "NVIDIA DLSS NR 组件安装失败。",
    "installSuccess": "组件已安装，现在可以启用 DLSS NR。",
    "installTitle": "安装 NVIDIA DLSS NR 组件",
    "intro": "实验性画面增强，兼容的主程序支持 SDR 与原生 HDR。在编码前处理游戏画面，会增加 GPU 开销，也可能改变人物与纹理。默认关闭；支持光流的主程序可在应用设置中单独开启运动估计。",
    "repairTitle": "替换 NVIDIA DLSS NR 组件",
    "runtime": "NVIDIA DLSS NR 组件",
    "runtimeMissingNotice": "尚未安装 NVIDIA DLSS NR 运行库。点击“选择组件”，导入本地 nvngx_dlssnr.dll。",
    "selectRuntime": "选择 nvngx_dlssnr.dll",
    "title": "NVIDIA DLSS NR",
    "uninstallConfirm": "是否关闭 DLSS NR 并删除已安装的 NVIDIA 组件？你原来的文件不会被修改。",
    "uninstallSuccess": "组件已删除，DLSS NR 已关闭。",
    "uninstallTitle": "删除 NVIDIA DLSS NR 组件",
    "appSettingsFailed": "无法打开应用设置，请检查 Sunshine 是否正在运行后重试。",
    "appSettings": "打开应用设置"
  }
}

export function dlssNrText(locale) {
  return dlssNrMessages[locale === 'zh' || locale === 'zh_TW' ? 'zh' : 'en']
}
