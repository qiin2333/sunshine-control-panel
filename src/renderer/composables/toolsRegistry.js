import {
  Monitor,
  Setting,
  Operation,
  Timer,
  DataLine,
  Cpu,
  Connection,
  MagicStick,
  Brush,
  House,
  Grid,
  QuestionFilled,
} from '@element-plus/icons-vue'
import IconGamepad from '../desktop/icons/IconGamepad.vue'
import IconTools from '../desktop/icons/IconTools.vue'
import { ROUTES } from './useRouter.js'
import { WEBUI_PAGES, getWebuiPage } from './webuiNavigation.js'

const WEBUI_ICONS = {
  overview: House,
  apps: Grid,
  pairing: Connection,
  'stream-settings': Operation,
}

/** Navigation represents destinations; actions and feature status live on their pages. */
export function createNavigationGroups(ctx) {
  const labels = ctx.t.value.sidebar
  const routeItem = (id, icon, label, route) => ({
    id,
    icon,
    label,
    action: () => ctx.router.navigate(route),
    isActive: () => ctx.router.isRoute(route),
  })
  return [
    {
      id: 'stream',
      label: labels.sectionStream,
      items: WEBUI_PAGES.map((page) => ({
        id: page.id,
        icon: WEBUI_ICONS[page.id],
        label: labels[page.label],
        action: () => ctx.openWebUi(page.path),
        isActive: () => ctx.router.isRoute(ROUTES.HOME) && getWebuiPage(ctx.webuiPath.value)?.id === page.id,
      })),
    },
    {
      id: 'devices',
      label: labels.sectionDevices,
      items: [
        routeItem('vdd', Monitor, labels.virtualDisplay, ROUTES.VDD_SETTINGS),
        {
          id: 'device-hub',
          icon: IconGamepad,
          label: labels.deviceHub,
          action: ctx.openControllers,
          isActive: () =>
            [ROUTES.CONTROLLERS, ROUTES.DUALSENSE, ROUTES.CONTROLLERS_HUB].some((route) => ctx.router.isRoute(route)),
        },
      ],
    },
    {
      id: 'assist',
      label: labels.sectionAssist,
      items: [
        routeItem('toolbox', IconTools, labels.toolbox, ROUTES.TOOLBOX),
        routeItem('ai-assistant', MagicStick, labels.aiAssistant, ROUTES.AI_ASSISTANT),
        ...(import.meta.env.DEV ? [routeItem('web-stream', Connection, labels.webStream, ROUTES.WEB_STREAM)] : []),
      ],
    },
  ]
}

export function createUtilityTools(ctx) {
  const labels = ctx.t.value.sidebar
  return [
    {
      id: 'stream-timer',
      icon: Timer,
      label: labels.streamTimer,
      description: labels.timerDescription,
      action: ctx.openTimer,
    },
    {
      id: 'latency-test',
      icon: DataLine,
      label: labels.latencyTest,
      description: labels.latencyDescription,
      external: true,
      action: () => ctx.openUrl('https://yangkile.github.io/D-lay/'),
    },
    {
      id: 'gamepad-test',
      icon: Cpu,
      label: labels.gamepadTest,
      description: labels.gamepadDescription,
      action: ctx.openGamepadTest,
    },
    {
      id: 'stylus-input-probe',
      icon: Brush,
      label: ctx.t.value.tools.stylusInputProbe,
      description: labels.stylusDescription,
      action: ctx.openStylusInputProbe,
    },
  ]
}

export function createFooterTools(ctx) {
  return [
    {
      id: 'panel-settings',
      icon: Setting,
      label: ctx.t.value.sidebar.panelSettings,
      action: () => ctx.router.navigate(ROUTES.PANEL_SETTINGS),
      isActive: () =>
        ctx.router.isRoute(ROUTES.PANEL_SETTINGS) ||
        (ctx.router.isRoute(ROUTES.HOME) && /^\/password(?:\.html)?(?:[?#]|$)/.test(ctx.webuiPath.value)),
    },
    {
      id: 'help',
      icon: QuestionFilled,
      label: ctx.t.value.sidebar.helpAbout,
      action: () => ctx.router.navigate(ROUTES.HELP),
      isActive: () =>
        ctx.router.isRoute(ROUTES.HELP) ||
        (ctx.router.isRoute(ROUTES.HOME) && /^\/troubleshooting(?:\.html)?(?:[?#]|$)/.test(ctx.webuiPath.value)),
    },
  ]
}
