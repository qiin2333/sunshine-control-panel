import { createApp } from 'vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import '../styles/global.less'
import './desktop.less'
import App from './DesktopApp.vue'

const app = createApp(App)

// 注册 Element Plus
app.use(ElementPlus)

app.mount('#app')

