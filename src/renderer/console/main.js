import { createApp } from 'vue';
import LogConsoleApp from './LogConsoleApp.vue';
import 'element-plus/theme-chalk/el-message.css';

const app = createApp(LogConsoleApp);
app.mount('#app');
