import { createApp } from 'vue';
import LogConsoleApp from './LogConsoleApp.vue';
import ElementPlus from 'element-plus';
import 'element-plus/dist/index.css';

const app = createApp(LogConsoleApp);
app.use(ElementPlus);
app.mount('#app');

