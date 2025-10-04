import { Menu, dialog } from 'electron'
import { t, getSupportedLanguages, setLanguage, getCurrentLanguage } from './index.js'

/**
 * 创建语言切换菜单项
 * @param {BrowserWindow} mainWindow 主窗口
 * @returns {Array} 菜单项数组
 */
export function createLanguageMenuItems(mainWindow) {
  const supportedLanguages = getSupportedLanguages()
  const currentLanguage = getCurrentLanguage()
  
  return supportedLanguages.map(lang => ({
    label: lang.nativeName,
    type: 'radio',
    checked: lang.code === currentLanguage,
    click: () => {
      setLanguage(lang.code)
      // 显示重启提示
      dialog.showMessageBox(mainWindow, {
        type: 'info',
        title: t('dialog.languageChanged'),
        message: t('dialog.restartRequired'),
        buttons: [t('dialog.ok')]
      })
    }
  }))
}

/**
 * 创建语言切换子菜单
 * @param {BrowserWindow} mainWindow 主窗口
 * @returns {Object} 菜单项对象
 */
export function createLanguageSubmenu(mainWindow) {
  return {
    label: t('menu.language'),
    submenu: createLanguageMenuItems(mainWindow)
  }
}

/**
 * 创建语言菜单作为一级菜单
 * @param {BrowserWindow} mainWindow 主窗口
 * @returns {Object} 语言菜单对象
 */
export function createLanguageMenu(mainWindow) {
  return {
    label: t('menu.language'),
    submenu: createLanguageMenuItems(mainWindow)
  }
}

/**
 * 添加语言切换功能到现有菜单
 * @param {Array} menuTemplate 现有菜单模板
 * @param {BrowserWindow} mainWindow 主窗口
 * @returns {Array} 更新后的菜单模板
 */
export function addLanguageSwitcherToMenu(menuTemplate, mainWindow) {
  // 创建语言菜单作为一级菜单
  const languageMenu = createLanguageMenu(mainWindow)
  
  // 将语言菜单添加到菜单模板中，放在关于菜单之前
  const aboutIndex = menuTemplate.findIndex(menu => menu.label === t('menu.about'))
  if (aboutIndex !== -1) {
    menuTemplate.splice(aboutIndex, 0, languageMenu)
  } else {
    // 如果找不到关于菜单，添加到末尾
    menuTemplate.push(languageMenu)
  }
  
  return menuTemplate
}
