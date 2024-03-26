# Sunshine 基地版串流食用指南
https://docs.gtimg.com/docs-design-resources/individuation/desktop/tmoji/qq_emoji/basic/78_meow_qqemoji-f5b2f212f7.png sunshine-foundation 是基于官方nightly分支修改得来，旨在提高各种串流终端设备与windows主机的接入体验。


## 下载地址
[Release Sunshine-Foundation v0.1 · qiin2333/Sunshine](https://github.com/qiin2333/Sunshine/releases/foundation)

[https://mirror.ghproxy.com/https://github.com/qiin2333/Sunshine/releases/download/foundation/sunshine-windows-installer.exe](https://mirror.ghproxy.com/https://github.com/qiin2333/Sunshine/releases/download/foundation/sunshine-windows-installer.exe)



## 安装
##### 非首次安装弹出是否卸载旧版
![图片](https://docimg5.docs.qq.com/image/AgAABTSpRGfkdaBX-ptAGr4FgNS9SywQ.png?w=885&h=373)
- 选择是将会清空原有配置信息，以及虚拟显示器的分辨率信息
- 选择否会保留原有配置信息
https://docs.gtimg.com/docs-design-resources/individuation/desktop/tmoji/qq_emoji/basic/77_dog_head_qqemoji-91208afa3a.png 21推荐： 否
##### 安装选项
![图片](https://docimg1.docs.qq.com/image/AgAABTSpRGePUFj6Gi1Aa6AojFb2-Nkb.png?w=997&h=716)
- 首次安全推荐组件全部勾上，非首次安装建议如图勾选。
- IddSampleDriver （虚拟HDR显示器）不支持win10，[win10点击下载非HDR版本虚拟显示器自行安装](https://github.com/itsmikethetech/Virtual-Display-Driver/releases/tag/23.10.20.2)
https://docs.gtimg.com/docs-design-resources/individuation/desktop/tmoji/qq_emoji/basic/77_dog_head_qqemoji-91208afa3a.png 21推荐： 默认安装目录，不随意更改目录，不安装在中文路径下。


## 设置
##### 配对(pin)
![图片](https://docimg7.docs.qq.com/image/AgAABTSpRGcDAUpR_JZBQZa5tu3lXzwl.png?w=2097&h=1054)
TODO: 为不同的串流接入设备自动启用指定的配置



##### 游戏&应用
![图片](https://docimg1.docs.qq.com/image/AgAABTSpRGc0qyQx1m5Ne5h3RdTEf9T5.png?w=2138&h=1589)
按图所示添加/编辑游戏的执行路径

https://docs.gtimg.com/docs-design-resources/individuation/desktop/tmoji/qq_emoji/basic/77_dog_head_qqemoji-91208afa3a.png 21推荐： 使用游戏管理工具 [r](g)来统一管理主机上的所有游戏，这样串流程序只需要指定Playnite。


##### 串流显示器行为
Sunshine 设置 → 视频/音频

![图片](https://docimg2.docs.qq.com/image/AgAABTSpRGelhbXzmztIy6kFNVX13Qb9.png?w=2590&h=1893)
如图是最佳推荐设置，显示设备指定(Display Device Specify)为可用的虚拟显示器，同时串流准备设置为“停用其他激活指定”，即可实现串流自动息屏物理显示器，退出串流后自动恢复物理显示器。

基地版默认安装的虚拟显示器支持多种分辨率与刷新率，如需要添加修改更多的分辨率可以从菜单上进行修改。

若使用物理欺骗器需要修改其内置的显示参数可以参考：[你奶奶都能学会的显示器超频指北](https://meowbot.page/2021/09/02/monitor-overclocking/)

##### HDR
- 一般 HDR 支持信息和要求：
    - HDR 必须在主机操作系统中激活，需要连接到主机 PC 的支持 HDR 的显示器（虚拟）或 EDID 显卡欺骗器。
    - 您还必须在 Moonlight 客户端设置中启用 HDR 选项，否则数据流将是 SDR 格式（如果主机是 HDR 格式，则可能曝光过度）。
    - 良好的 HDR 体验有赖于操作系统和游戏中正确的 HDR 显示校准。客户端和主机显示器的 HDR 校准可能会有很大不同。
支持编码 HEVC Main 10 或 AV1 10 位配置文件的英特尔、AMD 和英伟达™（NVIDIA®）图形处理器均支持 HDR 流媒体。

https://docs.gtimg.com/docs-design-resources/individuation/desktop/tmoji/qq_emoji/basic/77_dog_head_qqemoji-91208afa3a.png 21推荐： 通过将 [r](g)串流到客户端设备来校准显示屏，并保存 HDR 校准配置文件，以便在串流时使用。
## 高级用法
##### 超采样串流
方法一：使用 威力加强版 Moonlight-Android 调整主机缩放比例。

方法二：Sunshine 设置 → 视频/音频 → Display device options → Remap display modes。

## Q&A
Q: sunshine串流不能显示XBOXGAMEBAR

A: 设置 → 高级 → Force a Specific Capture Method → WGC，您可能需要停止服务并手动运行可执行文件才能测试新的捕获路径，像下面这样的简单批处理脚本就可以工作。

```powershell
cd /d "c:\program files\sunshine"
net stop sunshineservice
sunshine.exe
```


## 相关资源
[https://www.bilibili.com/video/BV1xu4y1M7yq/](https://www.bilibili.com/video/BV1xu4y1M7yq/)
[Sunshine官方文档](https://docs.lizardbyte.dev/projects/sunshine/en/latest/index.html)
[串流设备解码性能 - Moonlight Game Streaming Project](https://docs.qq.com/sheet/DSGxMdUl0UVZCeFRQ?tab=BB08J2)
[解锁杜比全景声串流](https://docs.qq.com/pdf/DSEFKbExvRXRzVktF)
[moonlight-android 威力加强版](https://github.com/qiin2333/moonlight-android/releases/shortcut)
