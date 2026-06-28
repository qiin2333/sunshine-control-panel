import { createApp } from 'vue';
import LogConsoleApp from './LogConsoleApp.vue';
import { ElIcon } from 'element-plus';
import 'element-plus/theme-chalk/base.css';
import 'element-plus/theme-chalk/el-icon.css';
import 'element-plus/theme-chalk/el-message.css';

const app = createApp(LogConsoleApp);
app.component(ElIcon.name, ElIcon);
app.mount('#app');
