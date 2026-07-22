/**
 * 多语言翻译配置
 * Internationalization (i18n) translations
 */

export const translations = {
  zh: {
    title: 'Sunshine 基地版',
    tagline: '让游戏串流更优雅',
    subtitle: 'Sunshine, a self-hosted game-stream host for Moonlight, now features an HDR-friendly fork that integrates virtual displays and control panels.',
    nav: {
      features: '特性',
      download: '下载',
      clients: '客户端',
      stats: 'Star History',
      docs: '文档'
    },
    hero: {
      download: '🚀 立即下载',
      github: '📦 GitHub Releases',
      mirror: '🌐 镜像下载'
    },
    features: {
      title: '🌟 核心特性',
      items: [
        {
          icon: '🎮',
          title: 'HDR友好支持',
          description: '经过优化的HDR处理管线，提供真正的HDR游戏流媒体体验'
        },
        {
          icon: '🖥️',
          title: '基地显示器（ZakoVDD）',
          description: '内置基地显示器管理，无需额外软件即可创建和管理虚拟显示器'
        },
        {
          icon: '🎤',
          title: '远程麦克风',
          description: '支持接收客户端麦克风，提供高音质的语音直通功能'
        },
        {
          icon: '⚡',
          title: '低延迟传输',
          description: '结合最新硬件能力优化的编码处理，提供流畅的游戏体验'
        },
        {
          icon: '🎛️',
          title: '高级控制面板',
          description: '直观的Web控制界面，提供实时监控和配置管理'
        },
        {
          icon: '🔗',
          title: '智能配对',
          description: '智能管理配对设备的对应配置文件'
        }
      ]
    },
    download: {
      title: '📥 下载 Sunshine 基地版',
      latestVersion: '最新版本',
      checkUpdate: '检查更新',
      checking: '正在检查最新版本...',
      error: '⚠️ 无法检查版本信息，使用默认下载地址',
      retry: '重试',
      requirements: '系统要求',
      requirementsList: [
        '系统: Windows10 22H2+',
        'CPU: Intel Core i3 / AMD Ryzen 3 以上',
        'GPU: 支持硬件编码的显卡, 支持VCE 1.0或更高版本, Intel VAAPI / AMD VCE / <a target="_blank" href="https://developer.nvidia.com/video-encode-and-decode-gpu-support-matrix-new">Nvidia NVENC</a>',
        'RAM: 4GB 或更多',
        '网络: 5GHz, 802.11ac'
      ],
      windowsLatest: 'Windows 最新版',
      recommended: '推荐使用',
      allVersions: '所有版本',
      githubReleases: 'GitHub Releases',
      mirrorDownload: '镜像下载',
      domesticSpeed: '国内加速',
      prerelease: '🚀 预发布版本可用',
      prereleaseFound: '发现新的预发布版本',
      viewPrerelease: '查看预发布版本'
    },
    clients: {
      title: '📱 推荐的 Moonlight Client',
      subtitle: '建议使用以下经过优化的客户端获得最佳的串流体验',
      downloadBtn: '下载 →'
    },
    stats: {
      title: '⭐ Star History',
      subtitle: '查看项目的 GitHub Star 增长趋势',
      loading: '正在加载 Star History...',
      error: '⚠️ 无法加载 Star History 图表',
      viewManually: '手动查看',
      giveStar: '⭐ 给个 Star',
      viewStats: '📊 查看详细统计'
    },
    docs: {
      title: '📚 文档与支持',
      userGuide: '📖 使用文档',
      userGuideDesc: '详细的使用指南和配置说明',
      officialDocs: '📋 官方文档',
      officialDocsDesc: 'LizardByte 官方文档参考',
      qqGroup: '💬 QQ 交流群',
      qqGroupDesc: '加入社区获取帮助'
    },
    footer: {
      title: 'Sunshine 基地版',
      subtitle: '让游戏串流更优雅',
      links: '相关链接',
      copyright: '© 2025 Foundation Sunshine. 基于 LizardByte/Sunshine 修改. 发布者: qiin2333'
    }
  },
  en: {
    title: 'Sunshine Foundation',
    tagline: 'Make Game Streaming Greater',
    subtitle: 'Sunshine, a self-hosted game-stream host for Moonlight, now features an HDR-friendly fork that integrates virtual displays and control panels.',
    nav: {
      features: 'Features',
      download: 'Download',
      clients: 'Clients',
      stats: 'Star History',
      docs: 'Docs'
    },
    hero: {
      download: '🚀 Download Now',
      github: '📦 GitHub Releases',
      mirror: '🌐 Mirror Download'
    },
    features: {
      title: '🌟 Core Features',
      items: [
        {
          icon: '🎮',
          title: 'HDR-Friendly Support',
          description: 'Optimized HDR processing pipeline for true HDR game streaming experience'
        },
        {
          icon: '🖥️',
          title: 'Virtual Display',
          description: 'Built-in virtual display management without additional software'
        },
        {
          icon: '🎤',
          title: 'Remote Microphone',
          description: 'Support client microphone with high-quality voice passthrough'
        },
        {
          icon: '⚡',
          title: 'Low Latency',
          description: 'Optimized encoding with latest hardware capabilities for smooth gaming'
        },
        {
          icon: '🎛️',
          title: 'Advanced Control Panel',
          description: 'Intuitive web interface with real-time monitoring and configuration'
        },
        {
          icon: '🔗',
          title: 'Smart Pairing',
          description: 'Intelligently manage configuration files for paired devices'
        }
      ]
    },
    download: {
      title: '📥 Download Sunshine Foundation',
      latestVersion: 'Latest Version',
      checkUpdate: 'Check Update',
      checking: 'Checking latest version...',
      error: '⚠️ Unable to check version info, using default download link',
      retry: 'Retry',
      requirements: 'System Requirements',
      requirementsList: [
        'OS: Windows 10 22H2+',
        'CPU: Intel Core i3 / AMD Ryzen 3 or higher',
        'GPU: Hardware encoding support, VCE 1.0+, Intel VAAPI / AMD VCE / <a target="_blank" href="https://developer.nvidia.com/video-encode-and-decode-gpu-support-matrix-new">Nvidia NVENC</a>',
        'RAM: 4GB or more',
        'Network: 5GHz, 802.11ac'
      ],
      windowsLatest: 'Windows Latest',
      recommended: 'Recommended',
      allVersions: 'All Versions',
      githubReleases: 'GitHub Releases',
      mirrorDownload: 'Mirror Download',
      domesticSpeed: 'China mirror',
      prerelease: '🚀 Pre-release Available',
      prereleaseFound: 'New pre-release version found',
      viewPrerelease: 'View Pre-release'
    },
    clients: {
      title: '📱 Recommended Moonlight Clients',
      subtitle: 'Use these optimized clients for the best streaming experience',
      downloadBtn: 'Download →'
    },
    stats: {
      title: '⭐ Star History',
      subtitle: 'View GitHub Star growth trends',
      loading: 'Loading Star History...',
      error: '⚠️ Unable to load Star History chart',
      viewManually: 'View Manually',
      giveStar: '⭐ Give a Star',
      viewStats: '📊 View Detailed Stats'
    },
    docs: {
      title: '📚 Documentation & Support',
      userGuide: '📖 User Guide',
      userGuideDesc: 'Detailed usage guide and configuration instructions',
      officialDocs: '📋 Official Docs',
      officialDocsDesc: 'LizardByte official documentation reference',
      qqGroup: '💬 QQ Community',
      qqGroupDesc: 'Join the QQ community for help'
    },
    footer: {
      title: 'Sunshine Foundation',
      subtitle: 'Make Game Streaming Greater',
      links: 'Links',
      copyright: '© 2025 Foundation Sunshine. Modified from LizardByte/Sunshine. Publisher: qiin2333'
    }
  }
}

