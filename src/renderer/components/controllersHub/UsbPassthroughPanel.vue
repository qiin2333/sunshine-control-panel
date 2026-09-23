<template>
  <section class="chub-panel">
    <UsbForwardingSettings :transport-ready="status.ready && !statusProbeFailed" />
    <UsbTransportPanel :transport="transport" />
    <details class="chub-section chub-disclosure chub-disclosure--section">
      <summary>
        <span class="chub-section-label">◈ {{ t.deviceHub.usb.manualTitle }}</span>
        <span class="chub-section-rule"></span>
        <span class="chub-disclosure-mark" aria-hidden="true">▸</span>
      </summary>
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
      <p class="chub-hint">{{ t.deviceHub.usb.securityHint }}</p>
    </details>

    <div class="chub-section">
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
      <strong>{{ t.deviceHub.usb.boundaryTitle }}</strong>
      <span>{{ t.deviceHub.usb.boundaryHint }}</span>
    </div>
  </section>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { usbip } from '../../tauri-adapter.js'
import { useI18n } from '../../desktop/i18n/index.js'
import { useUsbTransport } from '../../composables/useUsbTransport.js'
import UsbForwardingSettings from './UsbForwardingSettings.vue'
import UsbTransportPanel from './UsbTransportPanel.vue'

const { t } = useI18n()
const transport = useUsbTransport()
const { status, statusLoaded, statusProbeFailed, attachedDevices, refreshStatus, friendlyError, transportBusy } = transport
const discovering = ref(false)
const discoveryDone = ref(false)
const attachingBusId = ref('')
const detachingPort = ref(0)
const remote = ref('')
const tcpPort = ref(3240)
const remoteDevices = ref([])
const operationBusy = computed(() => transportBusy.value || Boolean(attachingBusId.value || detachingPort.value))
const validTcpPort = computed(() => Number.isInteger(tcpPort.value) && tcpPort.value >= 1024 && tcpPort.value <= 65535)
const canDiscover = computed(() => Boolean(
  status.ready && remote.value.trim() && validTcpPort.value && !discovering.value && !operationBusy.value,
))

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
</script>
