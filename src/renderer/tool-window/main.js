import { createApp } from 'vue';
import ToolWindow from './ToolWindow.vue';
import ElementPlus from 'element-plus';
import 'element-plus/dist/index.css';

const app = createApp(ToolWindow);
app.use(ElementPlus);
app.mount('#app');

