const { contextBridge, ipcRenderer, webFrame } = require('electron')

contextBridge.exposeInMainWorld('electron', {
  googleTranslateElementInit: () => {
    webFrame.executeJavaScript(
      `new google.translate.TranslateElement(
        { pageLanguage: 'en', layout: google.translate.TranslateElement.InlineLayout.VERTICAL },
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
  if (!document.querySelector('#google_translate_element')) {
    var script = document.createElement('script')
    script.setAttribute('type', 'text/javascript')
    script.setAttribute(
      'src',
      'https://gtranslate.cdn.haah.net/translate_a/element.js?cb=electron.googleTranslateElementInit'
    )
    document.getElementsByTagName('head')[0].appendChild(script)

    var btn = document.createElement('div')
    btn.setAttribute('id', 'google_translate_element')
    btn.setAttribute('style', 'position: absolute; top: 5px; right: 5px;')
    document.querySelector('body').appendChild(btn)
  }
})
