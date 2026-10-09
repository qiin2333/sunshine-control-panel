import { createApp, watchEffect } from 'vue';
import { useI18n } from '../desktop/i18n/index.js';

const { t } = useI18n();
watchEffect(() => { document.title = t.value.windowTitles.toolbar; });
import ToolbarApp from './ToolbarApp.vue';

const app = createApp(ToolbarApp);
app.mount('#app');

