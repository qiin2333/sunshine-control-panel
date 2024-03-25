const { contextBridge, ipcRenderer, webFrame } = require('electron')

contextBridge.exposeInMainWorld('electron', {
  googleTranslateElementInit: () => {
    webFrame.executeJavaScript(
      `new google.translate.TranslateElement(
        { 
          pageLanguage: 'en',
          layout: google.translate.TranslateElement.InlineLayout.VERTICAL,
          includedLanguages: 'zh-CN,zh-TW'
        },
        'google_translate_element'
      )
      console.log('------------ google_translate loaded --------------')`
    )
  },
})

/**
 * The preload script runs before `index.html` is loaded
 * in the renderer. It has access to web APIs as well as
 * Electron's renderer process modules and some polyfilled
 * Node.js functions.
 *
 * https://www.electronjs.org/docs/latest/tutorial/sandbox
 */
window.addEventListener('DOMContentLoaded', () => {
  const body = document.querySelector('body')
  ipcRenderer.on('theme', (e, shouldUseDarkColors = false) => {
    body.setAttribute('data-bs-theme', shouldUseDarkColors ? 'dark' : 'light')
  })
  if (!document.querySelector('#google_translate_element')) {
    const script = document.createElement('script')
    script.setAttribute('type', 'text/javascript')
    script.setAttribute(
      'src',
      'https://gtranslate.cdn.haah.net/translate_a/element.js?cb=electron.googleTranslateElementInit'
    )
    document.getElementsByTagName('head')[0].appendChild(script)
    const btn = document.createElement('div')
    btn.setAttribute('id', 'google_translate_element')
    btn.setAttribute('style', 'position: absolute; top: 12px; right: 10px;')
    body.appendChild(btn)
  }
})
