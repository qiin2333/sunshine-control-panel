import { createApp } from 'vue'
import SunshineFrame from './components/SunshineFrame.vue'
import { ElIcon, ElScrollbar, ElSwitch, ElTooltip } from 'element-plus'
import 'element-plus/theme-chalk/base.css'
import 'element-plus/theme-chalk/el-icon.css'
import 'element-plus/theme-chalk/el-scrollbar.css'
import 'element-plus/theme-chalk/el-switch.css'
import 'element-plus/theme-chalk/el-popper.css'
import 'element-plus/theme-chalk/el-tooltip.css'
import 'element-plus/theme-chalk/el-message.css'
import './styles/dialog.less'  // 导入对话框样式
// 导入 Tauri polyfill
import './tauri-polyfill.js'
import { i18n, getDefaultLocale, setLocale } from '../i18n/index.js'

// 初始化语言
setLocale(getDefaultLocale())

const app = createApp(SunshineFrame)
app.component(ElIcon.name, ElIcon)
app.component(ElScrollbar.name, ElScrollbar)
app.component(ElSwitch.name, ElSwitch)
app.component(ElTooltip.name, ElTooltip)
app.use(i18n)
app.mount('#app')
