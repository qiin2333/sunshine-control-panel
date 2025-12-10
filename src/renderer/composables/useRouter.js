import { ref, computed } from 'vue'

/**
 * 路由定义
 */
export const ROUTES = {
  HOME: 'home',           // 默认内容 (slot)
  VDD_SETTINGS: 'vdd-settings',
  WELCOME: 'welcome',
}

/**
 * 路由配置
 */
const routeConfig = {
  [ROUTES.HOME]: {
    name: ROUTES.HOME,
    component: null, // 使用 slot
    title: '高级设置',
  },
  [ROUTES.VDD_SETTINGS]: {
    name: ROUTES.VDD_SETTINGS,
    component: 'VddSettings',
    title: '虚拟显示器',
  },
  [ROUTES.WELCOME]: {
    name: ROUTES.WELCOME,
    component: 'Welcome',
    title: '欢迎页面',
  },
}

/**
 * 路由管理 Composable
 */
export function useRouter() {
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
    return routeConfig[currentRoute.value] || routeConfig[ROUTES.HOME]
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

