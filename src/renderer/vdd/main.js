import { createApp } from 'vue'
import '../style.css'
import App from './index.vue'
import 'element-plus/theme-chalk/el-message.css'
// 导入 Tauri polyfill
import '../tauri-polyfill.js'

const app = createApp(App)
app.mount('#app')
