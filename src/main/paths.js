import { app } from 'electron'
import { join, dirname } from 'node:path'

const appDir = dirname(app.getPath('exe'))

export const SUNSHINE_PATH = dirname(dirname(appDir))
export const SUNSHINE_TOOLS_PATH = join(SUNSHINE_PATH, 'tools')
export const VIRTUAL_DRIVER_PATH = join(SUNSHINE_TOOLS_PATH, 'vdd')
export const VDD_SETTINGS_PATH = join(VIRTUAL_DRIVER_PATH, 'vdd_settings.xml')
export const SUNSHINE_CONF_PATH = join(SUNSHINE_PATH, 'config', 'sunshine.conf')
