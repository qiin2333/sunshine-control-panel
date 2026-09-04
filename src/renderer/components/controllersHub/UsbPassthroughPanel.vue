<template>
  <section class="chub-panel">
    <div class="chub-window" :class="statusClass">
      <span class="chub-window-tab">USB/IP TRANSPORT</span>
      <div class="chub-hud-row">
        <div class="chub-hud-state">
          <span class="chub-status-dot"></span>
          <strong>{{ statusTitle }}</strong>
        </div>
        <div class="chub-hud-actions">
          <span v-if="status.version" class="chub-hud-version">v{{ status.version }}</span>
          <el-button size="small" :loading="statusLoading" @click="refreshStatus()">{{ t.deviceHub.refresh }}</el-button>
        </div>
      </div>

      <p v-if="statusProbeFailed || (status.detail && status.installed)" class="chub-status-error">{{ friendlyError(status.detail) }}</p>
      <div v-if="statusLoaded && !statusProbeFailed && status.supported && !status.ready" class="chub-usb-setup">
        <p>{{ t.deviceHub.usb.setupHint }}</p>
        <el-button type="primary" :loading="installing" @click="installTransport">
          {{ t.deviceHub.usb.installTransport }}
        </el-button>
      </div>
      <div v-if="statusLoaded && !statusProbeFailed && status.supported && status.vhci_residual" class="chub-usb-setup chub-usb-residual">
        <p>{{ t.deviceHub.usb.residualHint }}</p>
        <el-button type="danger" :loading="cleaning" @click="cleanupResidual">
          {{ t.deviceHub.usb.cleanupResidual }}
        </el-button>
      </div>
      <el-alert
        v-if="status.reboot_recommended"
        class="chub-notice"
        type="warning"
        :closable="false"
        :title="t.deviceHub.usb.rebootRequired"
      />
    </div>

    <div class="chub-section">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.deviceHub.usb.exporterTitle }}</span>
        <span class="chub-section-rule"></span>
      </div>
      <p class="chub-hint">{{ t.deviceHub.usb.exporterHint }}</p>
      <form class="chub-usb-discovery" @submit.prevent="discover">
        <el-input
          v-model="remote"
          :placeholder="t.deviceHub.usb.hostPlaceholder"
          clearable
          autocomplete="off"
          :disabled="!status.ready || discovering"
        />
        <el-input-number
          v-model="tcpPort"
          :min="1024"
          :max="65535"
          :controls="false"
          :disabled="!status.ready || discovering"
          :aria-label="t.deviceHub.usb.tcpPortLabel"
        />
        <el-button native-type="submit" type="primary" :loading="discovering" :disabled="!canDiscover">
          {{ t.deviceHub.usb.discover }}
        </el-button>
      </form>

      <div v-if="discoveryDone" class="chub-usb-list" aria-live="polite">
        <el-empty v-if="remoteDevices.length === 0" :description="t.deviceHub.usb.noRemoteDevices" :image-size="64" />
        <article v-for="device in remoteDevices" :key="device.bus_id" class="chub-usb-device">
          <div class="chub-usb-device-body">
            <strong>{{ device.description || t.deviceHub.usb.unknownDevice }}</strong>
            <span>{{ t.deviceHub.usb.busId }} · {{ device.bus_id }}</span>
            <small v-if="device.details?.length">{{ device.details[0] }}</small>
          </div>
          <el-button
            size="small"
            type="primary"
            plain
            :loading="attachingBusId === device.bus_id"
            :disabled="operationBusy || !status.ready"
            @click="attachDevice(device)"
          >{{ t.deviceHub.usb.attach }}</el-button>
        </article>
      </div>
    </div>

    <div class="chub-section chub-usb-attached-section">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.deviceHub.usb.attachedTitle }}</span>
        <span class="chub-section-rule"></span>
        <el-tag size="small" effect="plain" :type="attachedDevices.length ? 'success' : 'info'">
          {{ attachedDevices.length }}
        </el-tag>
      </div>
      <p class="chub-hint">{{ t.deviceHub.usb.attachedHint }}</p>
      <el-empty v-if="statusLoaded && attachedDevices.length === 0" :description="t.deviceHub.usb.noAttachedDevices" :image-size="64" />
      <div v-else class="chub-usb-list">
        <article v-for="device in attachedDevices" :key="device.port" class="chub-usb-device is-attached">
          <div class="chub-usb-port">{{ String(device.port).padStart(2, '0') }}</div>
          <div class="chub-usb-device-body">
            <strong>{{ device.description || t.deviceHub.usb.unknownDevice }}</strong>
            <span>{{ device.remote_host }}:{{ device.remote_port }} · {{ device.remote_bus_id }}</span>
            <small>{{ device.speed }}<template v-if="device.serial"> · {{ device.serial }}</template></small>
          </div>
          <el-button
            size="small"
            type="danger"
            plain
            :loading="detachingPort === device.port"
            :disabled="operationBusy"
            @click="detachDevice(device)"
          >{{ t.deviceHub.usb.detach }}</el-button>
        </article>
      </div>
    </div>

    <div class="chub-context-note">
      <strong>{{ t.deviceHub.usb.securityTitle }}</strong>
      <span>{{ t.deviceHub.usb.securityHint }}</span>
    </div>
    <div class="chub-context-note">
      <strong>{{ t.deviceHub.usb.boundaryTitle }}</strong>
      <span>{{ t.deviceHub.usb.boundaryHint }}</span>
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { usbip } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'

const { t } = useI18n()
const status = reactive({
  supported: true,
  installed: false,
  ready: false,
  version: '',
  version_valid: false,
  reboot_recommended: false,
  vhci_residual: false,
  attached_devices: [],
  detail: '',
})
const statusLoading = ref(false)
const statusLoaded = ref(false)
const statusProbeFailed = ref(false)
const installing = ref(false)
const cleaning = ref(false)
const discovering = ref(false)
const discoveryDone = ref(false)
const attachingBusId = ref('')
const detachingPort = ref(0)
const remote = ref('')
const tcpPort = ref(3240)
const remoteDevices = ref([])
let statusRefreshPromise = null

const attachedDevices = computed(() => status.attached_devices || [])
const operationBusy = computed(() => Boolean(attachingBusId.value || detachingPort.value))
const validTcpPort = computed(() => Number.isInteger(tcpPort.value) && tcpPort.value >= 1024 && tcpPort.value <= 65535)
const canDiscover = computed(() => Boolean(
  status.ready
  && remote.value.trim()
  && validTcpPort.value
  && !discovering.value
  && !operationBusy.value,
))
const statusClass = computed(() => {
  if (status.ready) return 'state-ready'
  if (!statusLoaded.value) return ''
  if (statusProbeFailed.value) return 'state-error'
  if (!status.supported) return ''
  return status.installed ? 'state-error' : 'state-update_available'
})
const statusTitle = computed(() => {
  if (!statusLoaded.value) return t.value.deviceHub.usb.checkingTransport
  if (statusProbeFailed.value) return t.value.deviceHub.usb.statusUnavailable
  if (!status.supported) return t.value.deviceHub.usb.transportUnsupported
  if (status.ready) return t.value.deviceHub.usb.transportReady
  if (!status.installed) return t.value.deviceHub.usb.transportMissing
  return t.value.deviceHub.usb.transportNeedsRepair
})

function applyStatus(next) {
  if (next) Object.assign(status, next)
}

function friendlyError(error) {
  const message = String(error || '')
  const code = message.match(/^(USBIP-[A-Z]+-\d{3})/)?.[1]
  const translated = code && t.value.deviceHub.usb.errors?.[code]
  return translated || t.value.deviceHub.usb.unknownError
}

async function performStatusRefresh() {
  statusLoading.value = true
  try {
    const result = await usbip.getStatus()
    statusProbeFailed.value = !result?.success
    if (result?.success) applyStatus(result.data)
    else {
      status.ready = false
      status.attached_devices = []
      status.vhci_residual = false
      status.detail = result?.message || ''
    }
  } catch (error) {
    statusProbeFailed.value = true
    status.ready = false
    status.attached_devices = []
    status.vhci_residual = false
    status.detail = String(error || '')
  } finally {
    statusLoaded.value = true
    statusLoading.value = false
  }
}

async function refreshStatus(options = {}) {
  const afterCurrent = options?.afterCurrent === true
  const activeRefresh = statusRefreshPromise
  if (activeRefresh) {
    await activeRefresh
    if (!afterCurrent) return
  }

  const nextRefresh = performStatusRefresh()
  statusRefreshPromise = nextRefresh
  try {
    await nextRefresh
  } finally {
    if (statusRefreshPromise === nextRefresh) statusRefreshPromise = null
  }
}

async function installTransport() {
  try {
    await ElMessageBox.confirm(
      t.value.deviceHub.usb.installConfirm,
      t.value.deviceHub.usb.installTransport,
      { confirmButtonText: t.value.deviceHub.usb.continue, cancelButtonText: t.value.deviceHub.usb.cancel, type: 'warning' },
    )
  } catch { return }
  installing.value = true
  const result = await usbip.installTransport()
  installing.value = false
  if (!result?.success) return ElMessage.error(friendlyError(result?.message))
  applyStatus(result.data)
  if (result.data?.ready) ElMessage.success(t.value.deviceHub.usb.installSuccess)
  else ElMessage.warning(t.value.deviceHub.usb.rebootRequired)
}

async function cleanupResidual() {
  try {
    await ElMessageBox.confirm(
      t.value.deviceHub.usb.cleanupConfirm,
      t.value.deviceHub.usb.cleanupResidual,
      { confirmButtonText: t.value.deviceHub.usb.continue, cancelButtonText: t.value.deviceHub.usb.cancel, type: 'warning' },
    )
  } catch { return }
  cleaning.value = true
  const result = await usbip.cleanupTransport()
  cleaning.value = false
  if (!result?.success) return ElMessage.error(friendlyError(result?.message))
  applyStatus(result.data)
  if (result.data?.vhci_residual) ElMessage.error(friendlyError('USBIP-CLEAN-002'))
  else ElMessage.success(t.value.deviceHub.usb.cleanupSuccess)
}

async function discover() {
  if (!canDiscover.value) return
  discovering.value = true
  discoveryDone.value = false
  const result = await usbip.listRemote(remote.value.trim(), tcpPort.value)
  discovering.value = false
  discoveryDone.value = true
  if (!result?.success) {
    remoteDevices.value = []
    return ElMessage.error(friendlyError(result?.message))
  }
  remoteDevices.value = result.data || []
}

async function attachDevice(device) {
  if (!status.ready || operationBusy.value) return
  try {
    await ElMessageBox.confirm(
      t.value.deviceHub.usb.attachConfirm
        .replace('{device}', device.description || device.bus_id)
        .replace('{host}', remote.value.trim()),
      t.value.deviceHub.usb.attach,
      { confirmButtonText: t.value.deviceHub.usb.attach, cancelButtonText: t.value.deviceHub.usb.cancel, type: 'warning' },
    )
  } catch { return }
  if (!status.ready || operationBusy.value) return
  attachingBusId.value = device.bus_id
  const result = await usbip.attach(remote.value.trim(), device.bus_id, tcpPort.value)
  attachingBusId.value = ''
  if (!result?.success) return ElMessage.error(friendlyError(result?.message))
  await refreshStatus({ afterCurrent: true })
  ElMessage.success(t.value.deviceHub.usb.attachSuccess)
}

async function detachDevice(device) {
  try {
    await ElMessageBox.confirm(
      t.value.deviceHub.usb.detachConfirm.replace('{device}', device.description || device.remote_bus_id),
      t.value.deviceHub.usb.detach,
      { confirmButtonText: t.value.deviceHub.usb.detach, cancelButtonText: t.value.deviceHub.usb.cancel, type: 'warning' },
    )
  } catch { return }
  detachingPort.value = device.port
  const result = await usbip.detach(device.port)
  detachingPort.value = 0
  if (!result?.success) return ElMessage.error(friendlyError(result?.message))
  await refreshStatus({ afterCurrent: true })
  ElMessage.success(t.value.deviceHub.usb.detachSuccess)
}

watch([remote, tcpPort], () => {
  discoveryDone.value = false
  remoteDevices.value = []
})

onMounted(() => refreshStatus())
</script>
