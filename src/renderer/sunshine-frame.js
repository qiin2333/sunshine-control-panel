import { createApp } from 'vue'
import SunshineFrame from './components/SunshineFrame.vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import './styles/dialog.less'  // 导入对话框样式
// 导入 Tauri polyfill
import './tauri-polyfill.js'

const app = createApp(SunshineFrame)
app.use(ElementPlus)
app.mount('#app')