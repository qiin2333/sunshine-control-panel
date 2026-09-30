export const WEBUI_PAGES = Object.freeze([
  { id: 'overview', path: '/', label: 'overview' },
  { id: 'apps', path: '/apps', label: 'applications' },
  { id: 'pairing', path: '/pin', label: 'clientPairing' },
  { id: 'stream-settings', path: '/config', label: 'streamSettings' },
])

export function getWebuiPage(path) {
  if (typeof path !== 'string' || !path.startsWith('/') || path.startsWith('//')) return null
  const pathname =
    path
      .split(/[?#]/, 1)[0]
      .replace(/\.html$/, '')
      .replace(/\/$/, '') || '/'
  if (pathname === '/index') return WEBUI_PAGES[0]
  return (
    WEBUI_PAGES.find((page) => pathname === page.path || (page.path !== '/' && pathname.startsWith(`${page.path}/`))) ||
    null
  )
}

export function isWebuiNavigationPath(path) {
  return (
    WEBUI_PAGES.some((page) => page.path === path) ||
    path === '/password' ||
    path === '/password#logout' ||
    path === '/troubleshooting'
  )
}
