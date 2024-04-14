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

contextBridge.exposeInMainWorld('darkMode', {
  toggle: () => ipcRenderer.invoke('dark-mode:toggle'),
  system: () => ipcRenderer.invoke('dark-mode:system'),
})

const resetStyles = `
  p { 
    margin -bottom: 1rem;
  }
  ::-webkit-scrollbar-track-piece {
    background-color:#f8f8f8;
  }
  ::-webkit-scrollbar {
    width: 8px;
  }
  ::-webkit-scrollbar-thumb {
    background-color:#dddddd;
    background-clip:padding-box;
    min-height:28px;
  }
  ::-webkit-scrollbar-thumb:hover {
    background-color:#bbb;
  }
`

webFrame.insertCSS(resetStyles)

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
  ipcRenderer.on('theme', (e, { shouldUseDarkColors = false }) => {
    body.setAttribute('data-bs-theme', shouldUseDarkColors ? 'dark' : 'light')
  })

  if (!document.querySelector('#theme_ctrl')) {
    const btn = document.createElement('button')
    btn.setAttribute('id', 'theme_ctrl')
    btn.setAttribute(
      'style',
      'position: fixed; width: 56px; height: 56px; border-radius: 48px; right: 18px; bottom: 18px;'
    )
    btn.innerHTML =
      '<svg t="1711372397603" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg" p-id="11874" width="100%" height="100%"><path d="M561.15624 705.345669c-13.779647-17.489552-19.013263-35.376593-25.439348-55.118587-31.732937-97.318755-51.143689-197.949926-74.330595-297.454875L367.247461 378.079058c25.439348 98.511225 84.532833 230.014104 122.09562 319.051821 9.274762 21.994436 17.688297 41.140195 20.404477 45.181342 11.725949 17.290807 78.371741 66.513295 99.968687 66.513295 11.725949 0.331242 25.306851-8.413534 21.729443-21.729443 0-12.785922-59.424727-65.718315-70.289448-81.750404z" p-id="11875"></path><path d="M652.313903 557.678204c-17.224558-114.145824-42.13392-233.194022-67.904509-342.50372l-95.132561 25.704341c33.521641 100.564922 71.018179 201.063596 96.32503 304.079705 3.974898 16.363331 8.877272 29.877984 6.028596 46.638805l-16.230834 108.249725c-0.066248 6.889823 9.407259 11.129715 16.297082 11.129714h3.577408c21.530698 0 61.544672-83.605357 61.544673-104.93731v-3.577408c0-10.665977-1.722456-26.499321-4.504885-44.783852zM578.31455 279.369088c-4.041146 1.391214-9.009769 0-14.309633-2.252442l13.64715 20.338229-3.444912-0.132497-1.324966 3.246167-13.845895-20.603222 1.788704 24.511872-3.047421-1.788704-2.583684 2.252442-1.854953-25.240603c-2.981174 5.564857-6.22734 10.202238-10.599728 11.725949-3.246167 1.126221-6.823575-0.662483-7.949796-3.908649-0.993725-2.914925 0-4.902374 2.451187-7.41981-3.444912-0.463738-5.432361-1.457463-6.426085-4.438636-1.126221-3.246167 0.662483-6.823575 3.908649-7.949797 5.299864-1.788704 12.255936 1.126221 19.344505 4.571133 0.132497 0 0.198745-0.066248 0.331241-0.132496 1.258718-0.861228 2.583684-2.186194 3.90865-2.119946 3.378663-6.62483 6.956072-12.653426 11.990943-14.375882 3.246167-1.126221 6.823575 0.662483 7.949796 3.90865 0.993725 2.981174 0 4.902374-2.451187 7.41981 3.444912 0.463738 5.432361 1.457463 6.426085 4.438636 1.126221 3.246167-0.662483 6.823575-3.90865 7.949796z" p-id="11876"></path></svg>'
    btn.setAttribute('onclick', 'window.darkMode.toggle()')
    body.appendChild(btn)
  }

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
    btn.setAttribute('style', 'position: absolute; top: 18px; right: 20px;')
    body.appendChild(btn)
  }
})
