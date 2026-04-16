<template>
  <div class="devices-view">
    <div class="page-header fade-in">
      <h1 class="page-title">{{ t.devices.pageTitle }}</h1>
      <p class="page-subtitle">{{ t.devices.pageSubtitle }}</p>
    </div>

    <!-- 配对区域 - 双列布局 -->
    <div class="pairing-section fade-in">
      <div class="desktop-grid cols-2">
        <!-- 二维码配对 -->
        <div class="desktop-card pairing-card">
          <div class="card-header">
            <div class="card-title">
              <span class="title-icon"><Iphone /></span>
              {{ t.devices.qrCodePairing }}
            </div>
          </div>
          <div class="card-content">
            <div v-if="qrActive" class="qr-display">
              <div class="qr-image-wrapper">
                <img :src="qrDataUrl" alt="QR Code" class="qr-image" />
              </div>
              <div class="qr-meta">
                <div class="qr-pin">
                  <span class="qr-pin-label">PIN</span>
                  <span class="qr-pin-value">{{ qrPin }}</span>
                </div>
                <div class="qr-timer" :class="{ warning: qrRemaining <= 30 }">
                  ⏱ {{ qrRemaining }}s
                </div>
                <div class="qr-actions">
                  <button class="desktop-btn" @click="generateQrCode">{{ t.devices.qrRefresh }}</button>
                  <button class="desktop-btn" @click="cancelQrCode">{{ t.devices.qrCancel }}</button>
                </div>
              </div>
            </div>

            <div v-else-if="qrError" class="qr-error">{{ qrError }}</div>

            <div v-else class="qr-idle">
              <p>{{ t.devices.qrDesc }}</p>
              <button
                class="desktop-btn primary"
                :disabled="qrLoading"
                @click="generateQrCode"
              >
                {{ qrLoading ? t.devices.qrGenerating : t.devices.generateQrCode }}
              </button>
            </div>
          </div>
        </div>

        <!-- 手动 PIN 配对 -->
        <div class="desktop-card pairing-card">
          <div class="card-header">
            <div class="card-title">
              <span class="title-icon"><Link /></span>
              {{ t.devices.pinPairing }}
            </div>
          </div>
          <div class="card-content">
            <div class="pin-form">
              <div class="form-row">
                <label class="form-label">{{ t.devices.pinInput }}</label>
                <input
                  v-model="pinInput"
                  class="form-input"
                  type="text"
                  pattern="\d*"
                  maxlength="4"
                  :placeholder="t.devices.pinPlaceholder"
                />
              </div>
              <div class="form-row">
                <label class="form-label">{{ t.devices.deviceName }}</label>
                <input
                  v-model="deviceNameInput"
                  class="form-input"
                  type="text"
                  :placeholder="t.devices.deviceNamePlaceholder"
                />
              </div>
              <button
                class="desktop-btn primary full-width"
                :disabled="pinSubmitting || !pinInput || !deviceNameInput"
                @click="submitPin"
              >
                {{ pinSubmitting ? t.devices.pairing : t.devices.pairBtn }}
              </button>
              <div v-if="pinStatus" class="pin-status" :class="pinStatus.type">
                {{ pinStatus.message }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 已配对设备 -->
    <div class="section-header fade-in">
      <h2 class="section-title">{{ t.devices.pairedDevicesTitle }}</h2>
      <span class="section-count">{{ devices.length }}</span>
    </div>

    <div class="devices-list">
      <div
        v-for="(device, index) in devices"
        :key="device.uuid"
        class="desktop-card device-card fade-in"
        :style="{ animationDelay: `${index * 0.1}s` }"
      >
        <div class="device-icon"><Iphone /></div>
        <div class="device-info">
          <template v-if="editingUuid === device.uuid">
            <input
              v-model="editName"
              class="name-input"
              @keyup.enter="saveRename(device)"
              @keyup.escape="cancelRename"
              ref="nameInputRef"
            />
          </template>
          <template v-else>
            <div class="device-name">{{ device.name || t.devices.unnamedDevice }}</div>
          </template>
          <div class="device-meta">
            <span class="device-uuid">{{ device.uuid }}</span>
          </div>
        </div>
        <div class="device-actions">
          <template v-if="editingUuid === device.uuid">
            <button class="desktop-btn" @click="saveRename(device)">{{ t.devices.saveBtn }}</button>
            <button class="desktop-btn" @click="cancelRename">{{ t.devices.cancelBtn }}</button>
          </template>
          <template v-else>
            <button class="desktop-btn" @click="startRename(device)">{{ t.devices.renameBtn }}</button>
            <button class="desktop-btn danger" @click="unpairDevice(device)">{{ t.devices.unpairBtn }}</button>
          </template>
        </div>
      </div>

      <div v-if="!loading && devices.length === 0" class="empty-state fade-in">
        <div class="empty-icon"><Iphone /></div>
        <p>{{ t.devices.emptyState }}</p>
      </div>

      <div v-if="loading" class="empty-state fade-in">
        <p>{{ t.devices.loading }}</p>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { Iphone, Link } from '@element-plus/icons-vue'
import QRCode from 'qrcode'
import { useI18n } from '../i18n/index.js'

const { t } = useI18n()

// === Tauri invoke ===
const invoke = ref(null)
const proxyUrl = ref('http://localhost:48081')

async function initTauri() {
  try {
    const tauri = await import('@tauri-apps/api/core')
    invoke.value = tauri.invoke
    const url = await invoke.value('get_proxy_url_command')
    if (url) proxyUrl.value = url
  } catch (e) {
    console.log('Tauri invoke not available:', e)
  }
}

async function apiFetch(path, options = {}) {
  const response = await fetch(`${proxyUrl.value}${path}`, options)
  return await response.json()
}

// === 设备列表 ===
const devices = ref([])
const loading = ref(false)

async function loadDevices() {
  loading.value = true
  try {
    const data = await apiFetch('/api/clients/list')
    if (data.status?.toString() === 'true' && data.named_certs) {
      devices.value = data.named_certs
    }
  } catch (e) {
    console.error('Failed to load devices:', e)
  } finally {
    loading.value = false
  }
}

// === 重命名 ===
const editingUuid = ref(null)
const editName = ref('')
const nameInputRef = ref(null)

function startRename(device) {
  editingUuid.value = device.uuid
  editName.value = device.name || ''
  nextTick(() => {
    const input = document.querySelector('.name-input')
    if (input) input.focus()
  })
}

function cancelRename() {
  editingUuid.value = null
  editName.value = ''
}

async function saveRename(device) {
  const newName = editName.value.trim()
  if (!newName) return
  try {
    const data = await apiFetch('/api/clients/rename', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ uuid: device.uuid, name: newName }),
    })
    if (data.status?.toString() === 'true') {
      device.name = newName
    }
  } catch (e) {
    console.error('Failed to rename:', e)
  }
  cancelRename()
}

// === 取消配对 ===
async function unpairDevice(device) {
  const name = device.name || device.uuid
  if (!confirm(t.value.devices.unpairConfirm.replace('{name}', name))) return
  try {
    const data = await apiFetch('/api/clients/unpair', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ uuid: device.uuid }),
    })
    if (data.status?.toString() === 'true') {
      devices.value = devices.value.filter(d => d.uuid !== device.uuid)
    }
  } catch (e) {
    console.error('Failed to unpair:', e)
  }
}

// === QR 配对 ===
const qrDataUrl = ref('')
const qrPin = ref('')
const qrExpiresAt = ref(0)
const qrRemaining = ref(0)
const qrLoading = ref(false)
const qrError = ref('')
let countdownTimer = null

const qrActive = computed(() => qrRemaining.value > 0 && qrDataUrl.value !== '')

function stopCountdown() {
  if (countdownTimer) {
    clearInterval(countdownTimer)
    countdownTimer = null
  }
}

function startCountdown() {
  stopCountdown()
  countdownTimer = setInterval(() => {
    const remaining = Math.max(0, Math.floor((qrExpiresAt.value - Date.now()) / 1000))
    qrRemaining.value = remaining
    if (remaining <= 0) {
      stopCountdown()
      qrDataUrl.value = ''
      qrPin.value = ''
    }
  }, 1000)
}

async function generateQrCode() {
  qrLoading.value = true
  qrError.value = ''
  try {
    console.log('QR pair: using proxy URL:', proxyUrl.value)
    const data = await apiFetch('/api/qr-pair', { method: 'POST' })

    if (data.status?.toString() !== 'true') {
      qrError.value = data.error || t.value.devices.qrGenerateFailed
      return
    }

    qrPin.value = data.pin
    qrExpiresAt.value = Date.now() + data.expires_in * 1000
    qrRemaining.value = data.expires_in

    qrDataUrl.value = await QRCode.toDataURL(data.url, {
      width: 280,
      margin: 2,
      color: { dark: '#000000', light: '#ffffff' },
    })

    startCountdown()
  } catch (e) {
    console.error('Failed to generate QR:', e)
    qrError.value = `${t.value.devices.qrNetworkError}: ${e.message}`
  } finally {
    qrLoading.value = false
  }
}

function cancelQrCode() {
  stopCountdown()
  qrDataUrl.value = ''
  qrPin.value = ''
  qrRemaining.value = 0
}

// === 手动 PIN 配对 ===
const pinInput = ref('')
const deviceNameInput = ref('')
const pinSubmitting = ref(false)
const pinStatus = ref(null)

async function submitPin() {
  pinSubmitting.value = true
  pinStatus.value = null
  try {
    const data = await apiFetch('/api/pin', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ pin: pinInput.value, name: deviceNameInput.value }),
    })
    if (data.status?.toString() === 'true') {
      pinStatus.value = { type: 'success', message: t.value.devices.pinStatus.success }
      pinInput.value = ''
      deviceNameInput.value = ''
      loadDevices()
    } else {
      pinStatus.value = { type: 'error', message: t.value.devices.pinStatus.error }
    }
  } catch (e) {
    pinStatus.value = { type: 'error', message: t.value.devices.pinStatus.networkError }
  } finally {
    pinSubmitting.value = false
  }
}

// === 生命周期 ===
onMounted(async () => {
  await initTauri()
  loadDevices()
})

onUnmounted(() => {
  stopCountdown()
})
</script>

<style lang="less" scoped>
.devices-view {
  max-width: 1000px;
  margin: 0 auto;
}

.page-header {
  margin-bottom: 32px;

  .page-title {
    font-size: 32px;
    font-weight: 700;
    color: var(--fd-text-primary, #fff);
    margin: 0 0 8px 0;
  }

  .page-subtitle {
    font-size: 16px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    margin: 0;
  }
}

// 配对区域
.pairing-section {
  margin-bottom: 40px;
}

.pairing-card {
  display: flex;
  flex-direction: column;
}

.qr-idle {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 24px 0;
  text-align: center;

  p {
    margin: 0;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);
    font-size: 14px;
  }
}

.qr-display {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;

  .qr-image-wrapper {
    padding: 10px;
    background: #fff;
    border-radius: 12px;
  }

  .qr-image {
    width: 180px;
    height: 180px;
    border-radius: 4px;
    display: block;
  }

  .qr-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    justify-content: center;
  }

  .qr-pin {
    display: flex;
    align-items: center;
    gap: 8px;

    .qr-pin-label {
      font-size: 13px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.5);
    }

    .qr-pin-value {
      font-size: 24px;
      font-weight: 700;
      font-family: monospace;
      letter-spacing: 6px;
      background: linear-gradient(135deg, var(--fd-accent, #00fff5), var(--fd-accent-secondary, #ff00ff));
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
    }
  }

  .qr-timer {
    font-size: 13px;
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);

    &.warning {
      background: rgba(var(--fd-status-warning-rgb, 255, 215, 0), 0.15);
      color: var(--fd-status-warning, #ffd700);
    }
  }

  .qr-actions {
    display: flex;
    gap: 8px;
  }
}

.qr-error {
  color: var(--fd-status-danger, #ff6b35);
  padding: 12px;
  background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.1);
  border-radius: 8px;
}

// PIN 表单
.pin-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.form-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 13px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.6);
  font-weight: 500;
}

.form-input {
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  border-radius: 8px;
  padding: 10px 14px;
  color: var(--fd-text-primary, #fff);
  font-size: 14px;
  outline: none;
  transition: border-color 0.2s;

  &:focus {
    border-color: var(--fd-accent, #00fff5);
    box-shadow: 0 0 8px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.2);
  }

  &::placeholder {
    color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.25);
  }
}

.full-width {
  width: 100%;
}

.pin-status {
  padding: 10px 14px;
  border-radius: 8px;
  font-size: 14px;

  &.success {
    background: rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.1);
    color: var(--fd-status-success, #00ff88);
    border: 1px solid rgba(var(--fd-status-success-rgb, 0, 255, 136), 0.3);
  }

  &.error {
    background: rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.1);
    color: var(--fd-status-danger, #ff6b35);
    border: 1px solid rgba(var(--fd-status-danger-rgb, 255, 107, 53), 0.3);
  }
}

// 段落标题
.section-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
}

.section-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--fd-text-primary, #fff);
  margin: 0;
}

.section-count {
  font-size: 13px;
  padding: 2px 10px;
  border-radius: 10px;
  background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.12);
  color: var(--fd-accent, #00fff5);
}

// 设备列表
.devices-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.device-card {
  display: flex;
  align-items: center;
  gap: 20px;

  .device-icon {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 24px;
    background: rgba(var(--fd-accent-rgb, 0, 255, 245), 0.1);
    flex-shrink: 0;
  }

  .device-info {
    flex: 1;
    min-width: 0;

    .device-name {
      font-size: 15px;
      font-weight: 600;
      color: var(--fd-text-primary, #fff);
      margin-bottom: 2px;
    }

    .name-input {
      font-size: 15px;
      font-weight: 600;
      color: var(--fd-text-primary, #fff);
      background: rgba(0, 0, 0, 0.3);
      border: 1px solid rgba(var(--fd-accent-rgb, 0, 255, 245), 0.5);
      border-radius: 6px;
      padding: 4px 10px;
      outline: none;
      margin-bottom: 2px;

      &:focus {
        border-color: var(--fd-accent, #00fff5);
        box-shadow: 0 0 8px rgba(var(--fd-accent-rgb, 0, 255, 245), 0.3);
      }
    }

    .device-meta {
      font-size: 12px;
      color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.3);
      font-family: monospace;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  }

  .device-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }
}

.empty-state {
  text-align: center;
  padding: 40px 20px;
  color: rgba(var(--fd-text-primary-rgb, 255, 255, 255), 0.4);

  .empty-icon {
    font-size: 48px;
    margin-bottom: 12px;
  }

  p {
    margin: 0;
    font-size: 14px;
  }
}
</style>

