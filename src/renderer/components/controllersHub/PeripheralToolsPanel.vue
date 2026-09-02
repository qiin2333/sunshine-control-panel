<template>
  <section class="chub-panel">
    <div class="chub-section">
      <div class="chub-section-head">
        <span class="chub-section-label">◈ {{ t.controllersHub.peripherals.title }}</span>
        <span class="chub-section-rule"></span>
        <el-button size="small" :loading="refreshing" @click="refreshAll">{{ t.deviceHub.refresh }}</el-button>
      </div>

      <div v-if="!initialized" class="chub-cards" aria-live="polite">
        <article v-for="index in 4" :key="index" class="chub-card chub-card-placeholder">
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
              :type="vigemStatus.installed ? 'success' : 'info'"
              effect="plain"
            >{{ vigemStatus.installed
              ? (vigemStatus.version ? `v${vigemStatus.version}` : t.controllersHub.peripherals.installed)
              : t.controllersHub.peripherals.notInstalled }}</el-tag>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.vigem.hint }}</p>
          <p v-if="vigemStatus.installed && vigemStatus.status_text" class="chub-status-text">
            {{ vigemStatus.status_text }}
          </p>
          <div class="chub-card-actions">
            <el-button
              size="small"
              :type="vigemStatus.installed ? 'default' : 'primary'"
              :loading="ops.vigem"
              :disabled="refreshing"
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
              :type="vmouseStatus.installed ? 'success' : 'info'"
              effect="plain"
            >{{ vmouseStatus.installed
              ? t.controllersHub.peripherals.installed
              : t.controllersHub.peripherals.notInstalled }}</el-tag>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.vmouse.hint }}</p>
          <p v-if="vmouseStatus.installed && vmouseStatus.status_text" class="chub-status-text">
            {{ vmouseStatus.status_text }}
          </p>
          <div class="chub-card-actions">
            <el-checkbox
              :model-value="vmouseStatus.config_enabled"
              :disabled="refreshing || !vmouseStatus.installed || ops.vmouseConfig"
              @change="handleVmouseToggle"
            >{{ t.controllersHub.peripherals.vmouse.enableShort }}</el-checkbox>
            <el-button
              size="small"
              :type="vmouseStatus.installed ? 'default' : 'primary'"
              :loading="ops.vmouse"
              :disabled="refreshing"
              @click="confirmToggle('vmouse')"
            >{{ vmouseStatus.installed
              ? t.controllersHub.peripherals.uninstall
              : t.controllersHub.peripherals.install }}</el-button>
          </div>
        </article>

        <!-- ControllerMeta -->
        <article class="chub-card">
          <div class="chub-card-head">
            <strong>{{ t.controllersHub.peripherals.meta.title }}</strong>
            <el-tag
              size="small"
              :type="metaStatus.installed ? 'success' : 'info'"
              effect="plain"
            >{{ metaStatus.installed
              ? (metaStatus.version ? `v${metaStatus.version}` : t.controllersHub.peripherals.installed)
              : t.controllersHub.peripherals.notInstalled }}</el-tag>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.meta.hint }}</p>
          <div class="chub-card-actions">
            <el-button
              size="small"
              type="primary"
              :disabled="refreshing"
              @click="emit('open-controller-meta')"
            >{{ t.controllersHub.peripherals.meta.launch }}</el-button>
          </div>
        </article>

        <!-- 手写笔输入检测 -->
        <article class="chub-card">
          <div class="chub-card-head">
            <strong>{{ t.controllersHub.peripherals.stylus.title }}</strong>
          </div>
          <p class="chub-hint">{{ t.controllersHub.peripherals.stylus.hint }}</p>
          <div class="chub-card-actions">
            <el-button
              size="small"
              type="primary"
              :disabled="refreshing"
              @click="emit('open-stylus-input-probe')"
            >{{ t.controllersHub.peripherals.stylus.launch }}</el-button>
          </div>
        </article>
      </div>
    </div>
  </section>
</template>

<script setup>
import { onMounted } from 'vue'
import { ElMessageBox } from 'element-plus'
import { usePeripheralTools } from '../../composables/usePeripheralTools.js'
import { useI18n } from '../../desktop/i18n/index.js'

const emit = defineEmits(['open-controller-meta', 'open-stylus-input-probe'])
const { t } = useI18n()

const {
  vigemStatus, vmouseStatus, metaStatus, ops, initialized, refreshing,
  refreshAll, installVigem, uninstallVigem,
  installVmouse, uninstallVmouse, setVmouseEnabled,
} = usePeripheralTools()

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
