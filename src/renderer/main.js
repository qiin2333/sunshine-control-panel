import { createApp } from 'vue'
import './styles/global.less'
import './styles/dialog.less'  // 导入对话框样式
import App from './App.vue'
// 导入 Tauri polyfill 以支持全局 API
import './tauri-polyfill.js'
import { getInitialSimpleLocale, setDocumentLocale, syncSimpleLocaleFromSunshine } from './shared/simple-i18n.js'

setDocumentLocale(getInitialSimpleLocale())
syncSimpleLocaleFromSunshine({ zh: {}, en: {} }, setDocumentLocale)

const app = createApp(App);

app.mount('#app');
