<template>
  <div class="welcome-page" :data-theme="currentTheme">
    <div class="language-selector">
      <select v-model="currentLocale" @change="handleLocaleChange">
        <option v-for="loc in supportedLocales" :key="loc.code" :value="loc.code">
          {{ loc.name }}
        </option>
      </select>
    </div>

    <div class="container">
      <div class="welcome-header">
        <img src="../public/gura-pix.png" class="welcome-image" alt="Sunshine" />
        <h1 class="welcome-title">{{ t('greeting') }}</h1>
      </div>

      <div class="welcome-card">
        <p class="welcome-subtitle">{{ t('create_creds') }}</p>

        <div class="alert-box">
          <span class="alert-icon">⚠</span>
          <span>{{ t('create_creds_alert') }}</span>
        </div>

        <form @submit.prevent="handleSubmit" class="welcome-form">
          <div class="form-group">
            <label for="usernameInput">{{ t('username') }}</label>
            <input
              id="usernameInput"
              v-model="formData.username"
              type="text"
              autocomplete="username"
              :placeholder="t('username')"
            />
          </div>

          <div class="form-group">
            <label for="passwordInput">{{ t('password') }}</label>
            <input
              id="passwordInput"
              v-model="formData.password"
              type="password"
              autocomplete="new-password"
              :placeholder="t('password')"
              required
            />
          </div>

          <div class="form-group">
            <label for="confirmPasswordInput">{{ t('confirm_password') }}</label>
            <input
              id="confirmPasswordInput"
              v-model="formData.confirmPassword"
              type="password"
              :class="{ error: showPasswordError }"
              autocomplete="new-password"
              :placeholder="t('confirm_password')"
              required
            />
            <transition name="fade">
              <div v-if="showPasswordError" class="error-message">✗ {{ t('password_mismatch') }}</div>
            </transition>
            <transition name="fade">
              <div v-if="showPasswordSuccess" class="success-message">✓ {{ t('password_match_success') }}</div>
            </transition>
          </div>

          <button type="submit" class="btn" :class="{ 'btn-loading': loading }" :disabled="loading || !isFormValid">
            <span v-if="!loading">{{ t('login') }}</span>
          </button>

          <transition name="fade">
            <div v-if="errorMessage" class="message error">{{ errorMessage }}</div>
          </transition>
          <transition name="fade">
            <div v-if="successMessage" class="message success">{{ successMessage }}</div>
          </transition>
        </form>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { supportedLocales, setLocale as setI18nLocale, getDefaultLocale } from '../../i18n/index.js'
import { sunshine } from '../tauri-adapter.js'

const REDIRECT_DELAY = 2000

const emit = defineEmits(['close'])
const { t, locale } = useI18n()

const currentLocale = ref(locale.value)
const formData = ref({ username: 'sunshine', password: '', confirmPassword: '' })
const loading = ref(false)
const errorMessage = ref('')
const successMessage = ref('')
const currentTheme = ref('light')

const passwordsMatch = computed(() => {
  const { password, confirmPassword } = formData.value
  return !password || !confirmPassword || password === confirmPassword
})

const showPasswordError = computed(() => !passwordsMatch.value && formData.value.confirmPassword)
const showPasswordSuccess = computed(
  () => passwordsMatch.value && formData.value.confirmPassword && formData.value.password
)
const isFormValid = computed(() => {
  const { username, password, confirmPassword } = formData.value
  return username && password && confirmPassword && passwordsMatch.value
})

watch(locale, (val) => (currentLocale.value = val))
watch(currentLocale, (val) => {
  if (locale.value !== val) {
    setI18nLocale(val)
    locale.value = val
  }
})

const handleLocaleChange = () => setI18nLocale(currentLocale.value)

const handleSubmit = async () => {
  errorMessage.value = ''
  successMessage.value = ''

  if (!passwordsMatch.value) {
    errorMessage.value = t('password_mismatch')
    return
  }

  const { username, password, confirmPassword } = formData.value
  if (!username || !password || !confirmPassword) {
    errorMessage.value = `${t('error')} ${t('create_creds')}`
    return
  }

  loading.value = true

  try {
    const proxyUrl = await sunshine.getProxyUrl()
    const response = await fetch(`${proxyUrl}/api/password`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ newUsername: username, newPassword: password, confirmNewPassword: confirmPassword }),
    })

    const result = await response.json()

    if (result.status?.toString() === 'true') {
      successMessage.value = `${t('success')} ${t('welcome_success')}`
      setTimeout(() => {
        window.location.href = `${proxyUrl}/`
      }, REDIRECT_DELAY)
    } else {
      errorMessage.value = `${t('error')} ${result.error || t('server_error')}`
    }
  } catch (err) {
    console.error('Failed to save password:', err)
    errorMessage.value = `${t('error')} ${err.message || t('network_error')}`
  } finally {
    loading.value = false
  }
}

const getCurrentTheme = () => {
  const savedTheme = localStorage.getItem('sunshine-theme')
  return savedTheme || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
}

const updateTheme = () => {
  currentTheme.value = getCurrentTheme()
}

let themeObserver = null
let themeMediaQuery = null
let themeChangeHandler = null

onMounted(() => {
  const defaultLocale = getDefaultLocale()
  currentLocale.value = defaultLocale
  setI18nLocale(defaultLocale)

  updateTheme()

  themeObserver = new MutationObserver(updateTheme)
  if (document.body) {
    themeObserver.observe(document.body, {
      attributes: true,
      attributeFilter: ['data-bs-theme'],
    })
  }

  themeMediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  themeChangeHandler = () => {
    if (!localStorage.getItem('sunshine-theme')) {
      updateTheme()
    }
  }
  themeMediaQuery.addEventListener('change', themeChangeHandler)
})

onUnmounted(() => {
  themeObserver?.disconnect()
  if (themeMediaQuery && themeChangeHandler) {
    themeMediaQuery.removeEventListener('change', themeChangeHandler)
  }
})
</script>

<style lang="less" scoped>
@font-face {
  font-family: 'PixelMplus12';
  src: url('../public/fonts/PixelMplus12-Bold.woff2') format('woff2');
  font-weight: 700;
}

// Color Palette
@gura-blue: #4a9eff;
@gura-light-blue: #7ab8ff;
@gura-pale-blue: #a2d2ff;
@gura-bg-light: #f0f8ff;
@gura-bg-mid: #e6f2ff;
@gura-text: #3a7ed5;
@error: #ff4444;
@success: #44ff44;
@shadow: rgba(74, 158, 255, 0.3);

// Dark theme colors
@dark-bg: #2d2628;
@dark-bg-mid: #3d3235;
@dark-card: rgba(45, 38, 40, 0.95);
@dark-border: #8b6f5e;
@dark-border-hover: #a68b7a;
@dark-text: #e6d5b8;
@dark-text-secondary: #d4c4b0;

// Mixins
.pixel-rendering() {
  image-rendering: pixelated;
  image-rendering: -moz-crisp-edges;
  image-rendering: crisp-edges;
}

.pixel-font() {
  font-family: 'PixelMplus12', 'YouYuan', monospace;
}

.welcome-page {
  .pixel-font();
  background: linear-gradient(135deg, @gura-bg-light 0%, @gura-bg-mid 50%, #d4e8ff 100%);
  height: 100vh;
  max-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.5rem 1rem;
  position: relative;
  overflow: hidden;
}

.container {
  max-width: 480px;
  width: 100%;
  text-align: center;
  position: relative;
  z-index: 1;
}

.welcome-header {
  margin-bottom: 0.75rem;
}

.welcome-image {
  width: 50%;
  max-width: 120px;
  opacity: 0.85;
  margin-bottom: 0.5rem;
  animation: gura-float 2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  filter: drop-shadow(0 4px 12px @shadow);
  .pixel-rendering();
}

@keyframes gura-float {
  0%,
  100% {
    transform: translateY(0) rotate(0deg);
  }
  25% {
    transform: translateY(-10px) rotate(2deg);
  }
  50% {
    transform: translateY(-5px) rotate(-2deg);
  }
  75% {
    transform: translateY(-8px) rotate(1deg);
  }
}

.welcome-title {
  .pixel-font();
  font-size: 1.1rem;
  color: @gura-blue;
  margin: 0;
  text-shadow: 1px 1px 3px fade(@gura-blue, 30%), 2px 2px 0 fade(@gura-blue, 20%);
  letter-spacing: 1px;
  transform: skew(-2deg);
}

.welcome-card {
  background: rgba(255, 255, 255, 0.9);
  border: 3px solid @gura-blue;
  padding: 1rem 1.25rem;
  box-shadow: 0 8px 16px fade(@gura-blue, 20%), inset 0 1px 0 rgba(255, 255, 255, 0.5);
  .pixel-rendering();
}

.welcome-subtitle {
  .pixel-font();
  font-size: 0.85rem;
  color: @gura-text;
  margin: 0 0 0.75rem;
  line-height: 1.4;
  text-shadow: 1px 1px 2px fade(@gura-blue, 20%);
  letter-spacing: 0.5px;
}

.alert-box {
  background: fade(@gura-pale-blue, 30%);
  border: 2px solid @gura-light-blue;
  border-left: 4px solid @gura-blue;
  padding: 0.5rem 0.75rem;
  margin-bottom: 0.75rem;
  .pixel-font();
  font-size: 0.75rem;
  color: @gura-text;
  text-align: left;
  display: flex;
  align-items: flex-start;
  gap: 0.4rem;
  line-height: 1.3;
  text-shadow: 1px 1px 2px fade(@gura-blue, 15%);

  .alert-icon {
    font-size: 1rem;
    line-height: 1;
    flex-shrink: 0;
  }
}

.welcome-form {
  text-align: left;
}

.form-group {
  margin-bottom: 0.75rem;
}

label {
  display: block;
  margin-bottom: 0.3rem;
  .pixel-font();
  font-size: 0.75rem;
  font-weight: 700;
  color: @gura-text;
  letter-spacing: 0.5px;
  text-shadow: 1px 1px 2px fade(@gura-blue, 20%);
}

input {
  width: 100%;
  padding: 0.5rem 0.75rem;
  background: rgba(255, 255, 255, 0.95);
  border: 3px solid @gura-light-blue;
  color: @gura-text;
  font-size: 0.8rem;
  .pixel-font();
  transition: border-color 0.2s, box-shadow 0.2s;
  .pixel-rendering();

  &::placeholder {
    color: fade(@gura-text, 50%);
  }

  &:focus {
    outline: none;
    border-color: @gura-blue;
    box-shadow: 0 0 0 4px fade(@gura-blue, 20%), inset 0 2px 4px rgba(0, 0, 0, 0.05);
  }

  &.error {
    border-color: @error;
    background: rgba(255, 240, 240, 0.95);
    animation: shake 0.3s;
  }
}

@keyframes shake {
  0%,
  100% {
    transform: translateX(0);
  }
  25% {
    transform: translateX(-4px);
  }
  75% {
    transform: translateX(4px);
  }
}

.error-message,
.success-message {
  margin-top: 0.3rem;
  .pixel-font();
  font-size: 0.7rem;
  letter-spacing: 0.5px;
}

.error-message {
  color: @error;
  text-shadow: 1px 1px 2px fade(@error, 30%);
}

.success-message {
  color: @success;
  text-shadow: 1px 1px 2px fade(@success, 30%);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.btn {
  width: 100%;
  padding: 0.65rem 1rem;
  background: @gura-blue;
  color: #fff;
  border: 3px solid darken(@gura-blue, 10%);
  font-size: 0.85rem;
  font-weight: 700;
  .pixel-font();
  letter-spacing: 1px;
  cursor: pointer;
  transition: all 0.2s;
  position: relative;
  box-shadow: 0 4px 0 darken(@gura-blue, 20%), 0 6px 12px @shadow;
  text-shadow: 1px 1px 2px fade(#000, 30%);
  .pixel-rendering();
  margin-top: 0.5rem;

  &:hover:not(:disabled) {
    background: @gura-light-blue;
    transform: translateY(-2px);
    box-shadow: 0 6px 0 darken(@gura-blue, 20%), 0 8px 16px fade(@gura-blue, 40%);
  }

  &:active:not(:disabled) {
    transform: translateY(2px);
    box-shadow: 0 2px 0 darken(@gura-blue, 20%), 0 4px 8px fade(@gura-blue, 30%);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  &-loading {
    color: transparent;

    &::after {
      content: '';
      position: absolute;
      top: 50%;
      left: 50%;
      width: 20px;
      height: 20px;
      margin: -10px 0 0 -10px;
      border: 3px solid fade(#fff, 30%);
      border-top-color: #fff;
      border-radius: 50%;
      animation: spin 0.8s linear infinite;
    }
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.message {
  margin-top: 0.75rem;
  padding: 0.6rem 0.75rem;
  text-align: center;
  .pixel-font();
  font-size: 0.75rem;
  border: 2px solid;
  letter-spacing: 0.5px;
  text-shadow: 1px 1px 2px fade(#000, 20%);

  &.error {
    background: rgba(255, 240, 240, 0.9);
    color: @error;
    border-color: @error;
  }

  &.success {
    background: rgba(240, 255, 240, 0.9);
    color: @success;
    border-color: @success;
  }
}

.language-selector {
  position: absolute;
  top: 2rem;
  right: 1rem;
  z-index: 10;

  select {
    padding: 0.5rem 0.875rem;
    background: rgba(255, 255, 255, 0.95);
    border: 3px solid @gura-light-blue;
    color: @gura-text;
    font-size: 0.875rem;
    .pixel-font();
    font-weight: 700;
    cursor: pointer;
    transition: border-color 0.2s, box-shadow 0.2s;
    box-shadow: 0 2px 4px fade(@gura-blue, 20%);
    .pixel-rendering();

    &:hover {
      border-color: @gura-blue;
      box-shadow: 0 4px 8px fade(@gura-blue, 30%);
    }

    &:focus {
      outline: none;
      border-color: @gura-blue;
      box-shadow: 0 0 0 4px fade(@gura-blue, 20%);
    }
  }
}

// Dark theme
.welcome-page[data-theme='dark'] {
  background: linear-gradient(135deg, @dark-bg 0%, @dark-bg-mid 50%, #4a3f42 100%);

  .welcome-image {
    filter: drop-shadow(0 4px 12px rgba(212, 165, 165, 0.3));
  }

  .welcome-title {
    color: @dark-text;
    text-shadow: 1px 1px 3px rgba(0, 0, 0, 0.4), 2px 2px 0 rgba(0, 0, 0, 0.3);
  }

  .welcome-card {
    background: @dark-card;
    border-color: @dark-border;
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.1);
  }

  .welcome-subtitle {
    color: @dark-text-secondary;
    text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.5);
  }

  .alert-box {
    background: rgba(61, 50, 53, 0.6);
    border-color: @dark-border;
    border-left-color: @dark-border-hover;
    color: @dark-text;
    text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.5);
  }

  label {
    color: @dark-text-secondary;
    text-shadow: 1px 1px 2px rgba(0, 0, 0, 0.5);
  }

  input {
    background: @dark-card;
    border-color: @dark-border;
    color: @dark-text;

    &::placeholder {
      color: fade(@dark-text-secondary, 50%);
    }

    &:focus {
      border-color: @dark-border-hover;
      box-shadow: 0 0 0 4px fade(@dark-border-hover, 20%), inset 0 2px 4px rgba(0, 0, 0, 0.2);
    }

    &.error {
      background: rgba(61, 38, 40, 0.95);
      border-color: #ff6666;
    }
  }

  .btn {
    background: @dark-border;
    border-color: #6b5548;
    box-shadow: 0 4px 0 #5a463a, 0 6px 12px rgba(0, 0, 0, 0.4);

    &:hover:not(:disabled) {
      background: @dark-border-hover;
      box-shadow: 0 6px 0 #6b5548, 0 8px 16px rgba(0, 0, 0, 0.5);
    }
  }

  .message {
    &.error {
      background: rgba(61, 38, 40, 0.9);
      color: #ff6666;
      border-color: #ff6666;
    }

    &.success {
      background: rgba(40, 61, 40, 0.9);
      color: #66ff66;
      border-color: #66ff66;
    }
  }

  .language-selector select {
    background: @dark-card;
    border-color: @dark-border;
    color: @dark-text;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);

    &:hover {
      border-color: @dark-border-hover;
      box-shadow: 0 4px 8px rgba(0, 0, 0, 0.4);
    }

    &:focus {
      border-color: @dark-border-hover;
      box-shadow: 0 0 0 4px fade(@dark-border-hover, 20%);
    }
  }
}
</style>
