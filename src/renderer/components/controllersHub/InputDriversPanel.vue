<template>
  <section class="chub-panel">
    <div class="chub-section">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.deviceHub.components.inputDrivers }}</span>
        <span class="chub-section-rule"></span>
        <el-button size="small" :loading="refreshing" @click="refreshAll">{{ t.deviceHub.refresh }}</el-button>
      </div>

      <div v-if="!initialized" class="chub-cards" aria-live="polite">
        <article v-for="index in 2" :key="index" class="chub-card chub-card-placeholder">
          <el-skeleton :rows="3" animated />
        </article>
      </div>
      <div v-else class="chub-cards">
        <!-- ViGEm -->
        <article class="chub-card" v-loading="ops.vigem">
          <div class="chub-card-head">
            <strong>{{ t.controllersHub.peripherals.vigem.title }}</strong>
            <el-tag
              size="small"
              :type="!probeFailed.vigem && vigemStatus.installed ? 'success' : 'info'"
              effect="plain"
            >{{ probeFailed.vigem ? t.deviceHub.probeUnavailable : (vigemStatus.installed
              ? (vigemStatus.version ? `v${vigemStatus.version}` : t.controllersHub.peripherals.installed)
              : t.controllersHub.peripherals.notInstalled) }}</el-tag>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.vigem.hint }}</p>
          <p v-if="vigemStatus.installed && vigemStatus.status_text">
            {{ vigemStatus.status_text }}
          </p>
          <div class="chub-card-actions">
            <el-button
              size="small"
              :type="vigemStatus.installed ? 'default' : 'primary'"
              :loading="ops.vigem"
              :disabled="refreshing || probeFailed.vigem"
              @click="confirmToggle('vigem')"
            >{{ vigemStatus.installed
              ? t.controllersHub.peripherals.uninstall
              : t.controllersHub.peripherals.install }}</el-button>
          </div>
        </article>

        <!-- 虚拟鼠标 -->
        <article class="chub-card" v-loading="ops.vmouse || ops.vmouseConfig">
          <div class="chub-card-head">
            <strong>{{ t.controllersHub.peripherals.vmouse.title }}</strong>
            <el-tag
              size="small"
              :type="!probeFailed.vmouse && vmouseStatus.installed ? 'success' : 'info'"
              effect="plain"
            >{{ probeFailed.vmouse ? t.deviceHub.probeUnavailable : (vmouseStatus.installed
              ? t.controllersHub.peripherals.installed
              : t.controllersHub.peripherals.notInstalled) }}</el-tag>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.vmouse.hint }}</p>
          <p v-if="vmouseStatus.installed && vmouseStatus.status_text">
            {{ vmouseStatus.status_text }}
          </p>
          <div class="chub-card-actions">
            <el-checkbox
              :model-value="vmouseStatus.config_enabled"
              :disabled="refreshing || !vmouseStatus.installed || ops.vmouseConfig || probeFailed.vmouse"
              @change="handleVmouseToggle"
            >{{ t.controllersHub.peripherals.vmouse.enableShort }}</el-checkbox>
            <el-button
              size="small"
              :type="vmouseStatus.installed ? 'default' : 'primary'"
              :loading="ops.vmouse"
              :disabled="refreshing || probeFailed.vmouse"
              @click="confirmToggle('vmouse')"
            >{{ vmouseStatus.installed
              ? t.controllersHub.peripherals.uninstall
              : t.controllersHub.peripherals.install }}</el-button>
          </div>
        </article>

      </div>
    </div>
  </section>
</template>

<script setup>
import { onMounted } from 'vue'
import { ElMessageBox } from 'element-plus'
import { useInputDrivers } from '../../composables/useInputDrivers.js'
import { useI18n } from '../../desktop/i18n/index.js'

const { t } = useI18n()

const {
  vigemStatus, vmouseStatus, probeFailed, ops, initialized, refreshing,
  refreshAll, installVigem, uninstallVigem,
  installVmouse, uninstallVmouse, setVmouseEnabled,
} = useInputDrivers()

async function confirmToggle(tool) {
  const strings = t.value.controllersHub.peripherals[tool]
  const installed = tool === 'vigem' ? vigemStatus.installed : vmouseStatus.installed
  const action = installed ? 'uninstall' : 'install'
  try {
    await ElMessageBox.confirm(strings[action === 'install' ? 'confirmInstall' : 'confirmUninstall'], t.value.controllersHub.peripherals.confirmTitle, {
      type: action === 'uninstall' ? 'warning' : 'info',
    })
  } catch {
    return
  }
  if (tool === 'vigem') {
    await (installed ? uninstallVigem() : installVigem())
  } else {
    await (installed ? uninstallVmouse() : installVmouse())
  }
}

async function handleVmouseToggle(enabled) {
  const settled = await setVmouseEnabled(enabled)
  vmouseStatus.config_enabled = settled
}

onMounted(refreshAll)
</script>
