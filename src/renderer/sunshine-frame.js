import { createApp } from 'vue'
import SunshineFrame from './components/SunshineFrame.vue'
import 'element-plus/theme-chalk/el-loading.css'
import 'element-plus/theme-chalk/el-message.css'
import 'element-plus/theme-chalk/el-overlay.css'
import 'element-plus/theme-chalk/el-message-box.css'
import 'element-plus/theme-chalk/el-notification.css'
import './styles/element-theme.less'  // Element Plus 深色 token，需早于各页面覆盖
import './styles/dialog.less'  // 导入对话框样式
// 导入 Tauri polyfill
import './tauri-polyfill.js'
import { i18n, getDefaultLocale, setLocale } from '../i18n/index.js'

// 初始化语言
setLocale(getDefaultLocale())

const app = createApp(SunshineFrame)
app.use(i18n)
app.mount('#app')
