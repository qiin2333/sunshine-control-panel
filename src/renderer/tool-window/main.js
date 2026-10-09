import { createApp, watchEffect } from 'vue';
import { useI18n } from '../desktop/i18n/index.js';

const { t } = useI18n();
watchEffect(() => { document.title = t.value.windowTitles.toolWindow; });
import ToolWindow from './ToolWindow.vue';

const app = createApp(ToolWindow);
app.mount('#app');

