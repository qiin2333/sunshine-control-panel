import { rtxHdrMessages } from './rtxHdrMessages.js'

export function dlssNrText(locale) {
  const chinese = locale === 'zh' || locale === 'zh_TW'
  const base = rtxHdrMessages[chinese ? 'zh' : 'en']
  const text = Object.fromEntries(Object.entries(base).map(([key, value]) => [
    key,
    typeof value === 'string'
      ? value.replaceAll('RTX HDR', 'DLSS NR').replaceAll('nvngx_truehdr.dll', 'nvngx_dlssnr.dll').replaceAll('Sunshine HDR', 'Sunshine DLSS NR')
      : { ...value },
  ]))
  return {
    ...text,
    title: 'NVIDIA DLSS NR',
    appSettingsFailed: chinese ? '无法打开应用设置，请检查 Sunshine 是否正在运行后重试。' : 'Could not open application settings. Check that Sunshine is running and try again.',
    appSettings: chinese ? '打开应用设置' : 'Open application settings',
    bridgeMissingNotice: chinese ? '当前 Sunshine 不支持 DLSS NR，请更新主程序后重试。' : 'This Sunshine installation does not support DLSS NR. Update Sunshine and try again.',
    intro: chinese
      ? '实验性 SDR 画面增强。在编码前处理游戏画面，会增加 GPU 开销，也可能改变人物与纹理。默认关闭；支持光流的主程序可在应用设置中单独开启运动估计。'
      : 'Experimental SDR enhancement before encoding. Adds GPU work and may change faces and textures. Disabled by default; compatible Sunshine builds offer optional optical-flow motion estimation in application settings.',
    enableHint: chinese
      ? '启用组件后，还需在应用设置中开启 DLSS NR，并以 SDR 模式重新连接。HDR 串流会跳过此处理。'
      : 'After enabling the component, enable DLSS NR in the application settings and reconnect in SDR mode. HDR streams bypass this processing.',
    acquisitionDescription: chinese
      ? '选择你已取得的 NVIDIA nvngx_dlssnr.dll。此组件与 RTX HDR 不同，请勿选择 nvngx_truehdr.dll。导入后会记录文件摘要；更新运行库请使用“重新选择”。'
      : 'Choose a locally obtained NVIDIA nvngx_dlssnr.dll. This is separate from RTX HDR; do not select nvngx_truehdr.dll. Import records the file digest. Use Choose again to update it.',
    runtimeMissingNotice: chinese
      ? '尚未安装 NVIDIA DLSS NR 运行库。点击“选择组件”，导入本地 nvngx_dlssnr.dll。'
      : 'The NVIDIA DLSS NR runtime is missing. Choose component to import a local nvngx_dlssnr.dll.',
  }
}
