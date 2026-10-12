import { ref, computed } from 'vue'
import { useI18n } from '../desktop/i18n/index.js'

/**
 * 路由定义
 */
export const ROUTES = {
  HOME: 'home',           // 默认内容 (slot)
  VDD_SETTINGS: 'vdd-settings',
  WELCOME: 'welcome',
  WEB_STREAM: 'web-stream',
  AI_ASSISTANT: 'ai-assistant',
  CONTROLLERS: 'controllers',
  // Compatibility alias for callers that still navigate to the original page id.
  DUALSENSE: 'dualsense',
  // Compatibility alias for the device hub's original page id.
  CONTROLLERS_HUB: 'controllers-hub',
  HDR_ENHANCED: 'hdr-enhanced',
  TOOLBOX: 'toolbox',
  PANEL_SETTINGS: 'panel-settings',
  HELP: 'help',
}

/**
 * 路由配置
 */
const routeConfig = {
  [ROUTES.HOME]: {
    name: ROUTES.HOME,
    component: null, // 使用 slot
    titleKey: 'home',
  },
  [ROUTES.VDD_SETTINGS]: {
    name: ROUTES.VDD_SETTINGS,
    component: 'VddSettings',
    titleKey: 'virtualDisplay',
  },
  [ROUTES.WELCOME]: {
    name: ROUTES.WELCOME,
    component: 'Welcome',
    titleKey: 'welcome',
  },
  [ROUTES.WEB_STREAM]: {
    name: ROUTES.WEB_STREAM,
    component: 'WebStreamSettings',
    titleKey: 'webStream',
  },
  [ROUTES.AI_ASSISTANT]: {
    name: ROUTES.AI_ASSISTANT,
    component: 'AiAssistant',
    titleKey: 'aiAssistant',
  },
  [ROUTES.CONTROLLERS]: {
    name: ROUTES.CONTROLLERS,
    component: 'ControllersHub',
    titleKey: 'deviceHub',
  },
  [ROUTES.DUALSENSE]: {
    name: ROUTES.DUALSENSE,
    component: 'ControllersHub',
    titleKey: 'deviceHub',
  },
  [ROUTES.CONTROLLERS_HUB]: {
    name: ROUTES.CONTROLLERS_HUB,
    component: 'ControllersHub',
    titleKey: 'deviceHub',
  },
  [ROUTES.HDR_ENHANCED]: {
    name: ROUTES.HDR_ENHANCED,
    component: 'HdrEnhancedManager',
    titleKey: 'hdrEnhanced',
  },
  [ROUTES.TOOLBOX]: { name: ROUTES.TOOLBOX, component: 'ControlPanelPage', titleKey: 'toolbox' },
  [ROUTES.PANEL_SETTINGS]: { name: ROUTES.PANEL_SETTINGS, component: 'ControlPanelPage', titleKey: 'panelSettings' },
  [ROUTES.HELP]: { name: ROUTES.HELP, component: 'ControlPanelPage', titleKey: 'helpAbout' },
}

/**
 * 路由管理 Composable
 */
export function useRouter() {
  const { t } = useI18n()
  const currentRoute = ref(ROUTES.HOME)
  const routeHistory = ref([ROUTES.HOME])

  /**
   * 导航到指定路由
   * @param {string} routeName - 路由名称
   * @param {object} options - 导航选项
   */
  const navigate = (routeName, options = {}) => {
    if (!routeConfig[routeName]) {
      console.warn(`路由 ${routeName} 不存在`)
      return
    }

    // 如果需要替换当前历史记录而不是添加
    if (options.replace) {
      routeHistory.value[routeHistory.value.length - 1] = routeName
    } else {
      routeHistory.value.push(routeName)
      // 限制历史记录长度
      if (routeHistory.value.length > 10) {
        routeHistory.value.shift()
      }
    }

    currentRoute.value = routeName
  }

  /**
   * 返回上一页
   */
  const goBack = () => {
    if (routeHistory.value.length > 1) {
      routeHistory.value.pop() // 移除当前路由
      currentRoute.value = routeHistory.value[routeHistory.value.length - 1]
    }
  }

  /**
   * 返回首页
   */
  const goHome = () => {
    navigate(ROUTES.HOME, { replace: true })
  }

  /**
   * 获取当前路由配置
   */
  const getCurrentRouteConfig = computed(() => {
    const config = routeConfig[currentRoute.value] || routeConfig[ROUTES.HOME]
    return { ...config, title: t.value.routeTitles[config.titleKey] || t.value.sidebar[config.titleKey] }
  })

  /**
   * 检查是否是某个路由
   */
  const isRoute = (routeName) => {
    return currentRoute.value === routeName
  }

  return {
    currentRoute,
    routeHistory,
    navigate,
    goBack,
    goHome,
    getCurrentRouteConfig,
    isRoute,
    ROUTES,
  }
}
