<template>
  <div class="desktop-card about-card fade-in">
    <div class="about-content">
      <div class="about-logo"><Sunny /></div>
      <div class="about-info">
        <div class="about-name">Foundation Desktop</div>
        <div class="about-version">{{ t.settings.version }} {{ appVersion }}</div>
        <div class="about-links">
          <a href="#" @click.prevent="emit('open-link', 'github')">GitHub</a>
          <span>&middot;</span>
          <a href="#" @click.prevent="emit('open-link', 'docs')">{{ t.settings.docs }}</a>
          <span>&middot;</span>
          <a href="#" @click.prevent="emit('open-link', 'discord')">Discord</a>
        </div>
      </div>
    </div>
    <button class="desktop-btn" :disabled="checking" @click="emit('check-update')">
      {{ checking ? t.settings.checking : t.settings.checkUpdate }}
    </button>
  </div>
</template>

<script setup>
import { Sunny } from '@element-plus/icons-vue'
import { useI18n } from '../../i18n/index.js'

defineProps({
  appVersion: {
    type: String,
    required: true,
  },
  checking: {
    type: Boolean,
    required: true,
  },
})

const emit = defineEmits(['open-link', 'check-update'])
const { t } = useI18n()
</script>

<style lang="less" scoped>
.about-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.about-content {
  display: flex;
  align-items: center;
  gap: 16px;
}

.about-logo {
  width: 42px;
  height: 42px;
  color: var(--fd-accent, #00fff5);
  display: flex;
  align-items: center;
  justify-content: center;

  svg {
    width: 100%;
    height: 100%;
  }
}

.about-name {
  font-size: 18px;
  font-weight: 600;
  color: var(--fd-text-primary, #fff);
}

.about-version {
  font-size: 14px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
  margin-bottom: 4px;
}

.about-links {
  font-size: 13px;
  display: flex;
  gap: 8px;

  a {
    color: var(--fd-accent, #00fff5);
    text-decoration: none;

    &:hover {
      text-decoration: underline;
    }
  }

  span {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
  }
}
</style>
