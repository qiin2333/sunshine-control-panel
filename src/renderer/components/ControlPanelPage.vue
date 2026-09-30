<template>
  <div class="panel-page">
    <header class="panel-heading">
      <h1>{{ heading.title }}</h1>
      <p>{{ heading.description }}</p>
    </header>
    <div v-if="page === ROUTES.TOOLBOX" class="tool-grid">
      <button v-for="tool in tools" :key="tool.id" type="button" class="tool-card" @click="tool.action">
        <el-icon :size="23" aria-hidden="true"><component :is="tool.icon" /></el-icon>
        <span>
          <strong>{{ tool.label }}</strong>
          <small>{{ tool.description }}</small>
        </span>
        <el-icon v-if="tool.external" class="external-icon" aria-hidden="true"><TopRight /></el-icon>
      </button>
    </div>
    <template v-else-if="page === ROUTES.PANEL_SETTINGS">
      <section class="panel-section">
        <h2>{{ t.sidebar.appearanceLanguage }}</h2>
        <div class="settings-card">
          <div class="settings-row">
            <label for="panel-theme">{{ t.sidebar.theme }}</label>
            <el-select id="panel-theme" :model-value="themeMode" @update:model-value="emit('theme-change', $event)">
              <el-option :label="t.sidebar.systemTheme" value="system" />
              <el-option :label="t.sidebar.lightMode" value="light" />
              <el-option :label="t.sidebar.darkMode" value="dark" />
            </el-select>
          </div>
          <div class="settings-row">
            <label for="panel-language">{{ t.sidebar.language }}</label>
            <el-select id="panel-language" :model-value="locale" @update:model-value="emit('locale-change', $event)">
              <el-option label="简体中文" value="zh" />
              <el-option label="English" value="en" />
            </el-select>
          </div>
        </div>
      </section>
      <section class="panel-section">
        <h2>{{ t.sidebar.softwareUpdate }}</h2>
        <div class="settings-card">
          <div class="settings-row">
            <span id="panel-update-channel">{{ t.sidebar.updateChannel }}</span>
            <el-radio-group
              :model-value="includePrerelease"
              aria-labelledby="panel-update-channel"
              @update:model-value="emit('channel-change', $event)"
            >
              <el-radio :value="false">{{ t.sidebar.stableChannel }}</el-radio>
              <el-radio :value="true">{{ t.sidebar.betaChannel }}</el-radio>
            </el-radio-group>
          </div>
          <div class="settings-row">
            <div>
              <strong>{{ t.sidebar.currentVersion }}</strong>
              <p>{{ currentVersion }}</p>
            </div>
            <el-button :loading="checkingUpdate" @click="checkUpdate">{{ t.sidebar.checkUpdate }}</el-button>
          </div>
        </div>
      </section>
      <section class="panel-section">
        <h2>{{ t.sidebar.syncFeatures }}</h2>
        <div class="settings-card">
          <div class="settings-row">
            <div>
              <strong>{{ t.sidebar.clipboardSync }}</strong>
              <p>{{ t.sidebar.clipboardDescription }}</p>
            </div>
            <div class="row-actions">
              <el-tag :type="clipboardSyncEnabled ? 'success' : 'info'">
                {{ clipboardSyncEnabled ? t.sidebar.connected : t.sidebar.notConnected }}
              </el-tag>
              <el-button @click="actions.showClipboardSyncStatus">{{ t.sidebar.viewStatus }}</el-button>
            </div>
          </div>
        </div>
      </section>
      <section class="panel-section">
        <h2>{{ t.sidebar.accountSettings }}</h2>
        <div class="settings-card">
          <div class="settings-row">
            <div>
              <strong>{{ t.sidebar.accountCredentials }}</strong>
              <p>{{ t.sidebar.accountDescription }}</p>
            </div>
            <el-button @click="emit('webui-navigate', '/password')">{{ t.sidebar.configure }}</el-button>
          </div>
          <div class="settings-row">
            <div>
              <strong>{{ t.sidebar.logout }}</strong>
              <p>{{ t.sidebar.logoutDescription }}</p>
            </div>
            <el-button type="danger" plain @click="emit('webui-navigate', '/password#logout')">
              {{ t.sidebar.logout }}
            </el-button>
          </div>
        </div>
      </section>
      <section class="panel-section">
        <h2>{{ t.sidebar.maintenance }}</h2>
        <div class="settings-card">
          <div class="settings-row">
            <div>
              <strong>{{ t.sidebar.coverCache }}</strong>
              <p>{{ t.sidebar.coverCacheDescription }}</p>
            </div>
            <el-button @click="actions.cleanupCovers">{{ t.sidebar.clean }}</el-button>
          </div>
          <div v-if="!isAdmin" class="settings-row">
            <el-button @click="actions.restartAsAdmin">{{ t.sidebar.restartAsAdmin }}</el-button>
          </div>
        </div>
      </section>
    </template>
    <template v-else>
      <div class="settings-card">
        <div class="settings-row">
          <div>
            <strong>{{ t.sidebar.officialWebsite }}</strong>
            <p>AlkaidLab</p>
          </div>
          <el-button @click="actions.openUrl('https://www.alkaidlab.com/')">
            {{ t.sidebar.visit }}
            <el-icon aria-hidden="true"><TopRight /></el-icon>
          </el-button>
        </div>
        <div class="settings-row">
          <div>
            <strong>{{ t.sidebar.clientDownloads }}</strong>
            <p>Moonlight / VoidLink</p>
          </div>
          <el-button @click="emit('webui-navigate', '/')">{{ t.sidebar.view }}</el-button>
        </div>
        <div class="settings-row">
          <div>
            <strong>{{ t.sidebar.diagnostics }}</strong>
            <p>{{ t.sidebar.diagnosticsDescription }}</p>
          </div>
          <el-button @click="emit('webui-navigate', '/troubleshooting')">{{ t.sidebar.open }}</el-button>
        </div>
      </div>
      <p class="version-note">Foundation Sunshine · {{ currentVersion }}</p>
    </template>
  </div>
</template>
<script setup>
import { computed, ref } from 'vue'
import { TopRight } from '@element-plus/icons-vue'
import { useI18n } from '../desktop/i18n/index.js'
import { ROUTES } from '../composables/useRouter.js'
const props = defineProps({
  page: { type: String, required: true },
  tools: { type: Array, default: () => [] },
  themeMode: { type: String, default: 'system' },
  locale: { type: String, default: 'en' },
  includePrerelease: Boolean,
  currentVersion: { type: String, default: '' },
  clipboardSyncEnabled: Boolean,
  isAdmin: Boolean,
  actions: { type: Object, required: true },
})
const emit = defineEmits(['theme-change', 'locale-change', 'channel-change', 'webui-navigate'])
const { t } = useI18n()
const heading = computed(() => {
  const s = t.value.sidebar
  if (props.page === ROUTES.TOOLBOX) return { title: s.toolbox, description: s.toolboxDescription }
  if (props.page === ROUTES.PANEL_SETTINGS) return { title: s.panelSettings, description: s.panelSettingsDescription }
  return { title: s.helpAbout, description: 'Foundation Sunshine' }
})
const checkingUpdate = ref(false)
const checkUpdate = async () => {
  if (checkingUpdate.value) return
  checkingUpdate.value = true
  try {
    await props.actions.handleCheckForUpdates()
  } finally {
    checkingUpdate.value = false
  }
}
</script>
<style scoped lang="less">
.panel-page {
  --panel-surface: #3d3235;
  --panel-text: #eee5db;
  --panel-muted: #bfb2b5;
  --panel-border: rgba(230, 213, 184, 0.16);
  --panel-accent: #e6d5b8;
  max-width: 1100px;
  margin: 0 auto;
  padding: 30px;
  color: var(--panel-text);
  font-family: 'Segoe UI', 'Microsoft YaHei', sans-serif;
}
:global([data-bs-theme='light'] .panel-page) {
  --panel-surface: #fff;
  --panel-text: #26354a;
  --panel-muted: #62758c;
  --panel-border: #dbe6f4;
  --panel-accent: #3484ef;
}
.panel-heading {
  margin-bottom: 27px;
  h1 {
    font-size: 28px;
    font-weight: 600;
    margin: 0 0 10px;
  }
  p {
    color: var(--panel-muted);
    font-size: 14px;
    line-height: 1.6;
    margin: 0;
  }
}
.panel-section {
  margin-bottom: 25px;
  h2 {
    font-size: 16px;
    font-weight: 500;
    margin: 0 0 12px;
  }
}
.settings-card,
.tool-card {
  background: var(--panel-surface);
  border: 1px solid var(--panel-border);
  border-radius: 16px;
}
.settings-card {
  padding: 4px 22px;
}
.settings-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 19px 0;
  border-bottom: 1px solid var(--panel-border);
  font-size: 14px;
  &:last-child {
    border-bottom: 0;
  }
  strong {
    font-weight: 500;
  }
  p {
    color: var(--panel-muted);
    font-size: 12px;
    line-height: 1.6;
    margin: 5px 0 0;
  }
  :deep(.el-select) {
    width: 175px;
  }
  :deep(.el-radio-group) {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  :deep(.el-radio) {
    margin-right: 10px;
  }
}
.row-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}
.tool-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}
.tool-card {
  display: flex;
  align-items: center;
  gap: 15px;
  padding: 23px;
  text-align: left;
  cursor: pointer;
  color: var(--panel-text);
  font: inherit;
  min-width: 0;
  > .el-icon {
    color: var(--panel-accent);
    flex-shrink: 0;
  }
  span {
    min-width: 0;
    flex: 1;
  }
  strong {
    display: block;
    font-weight: 500;
    font-size: 15px;
  }
  small {
    display: block;
    font-size: 12px;
    line-height: 1.6;
    color: var(--panel-muted);
    margin-top: 6px;
  }
  &:hover {
    border-color: var(--panel-accent);
  }
  .external-icon {
    margin-left: auto;
  }
}
.version-note {
  color: var(--panel-muted);
  margin-top: 20px;
  font-size: 12px;
}
@media (max-width: 850px) {
  .panel-page {
    padding: 24px 18px;
  }
  .tool-grid {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 480px) {
  .settings-card {
    padding: 4px 16px;
  }
  .settings-row :deep(.el-select) {
    width: 100%;
  }
  .panel-heading h1 {
    font-size: 23px;
  }
}
</style>
